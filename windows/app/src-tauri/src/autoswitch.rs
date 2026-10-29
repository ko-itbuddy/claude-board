//! 사용 한도 자동 전환 — `AutoSwitcher` (`ClaudeBoard.swift AutoSwitcher`, SPEC §5.6).
//!
//! 지금 계정의 5시간·주간 한도가 차거나 세션이 한도에 걸려 멈추면 저장한 다른 계정으로 바꾸고, 한도로 멈췄던 세션의
//! 콘솔에 이어서 진행하라는 요청을 넣는다. 다른 계정의 남은 한도는 미리 알 수 없어 그 계정이 지금 계정이었을 때
//! 한도가 찼던 기록으로 고른다. 결정마다 `notifier/auto-switch.log` 에 한 줄씩 남긴다.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use claude_board_core::console::{self, Input};
use claude_board_core::{clock, paths};

use crate::accounts::{self, Account, SwitchSide};
use crate::board::Board;
use crate::model::{Limit, Session};
use crate::util::append_log;

/// 실행 중인 세션은 로그인 정보를 최대 30초 기억해 두고 쓴다 — 전환한 뒤 이만큼은 이전 계정으로 요청이 나갈 수 있다
pub const SETTLE: f64 = 40.0;
/// 이보다 오래된 한도 기록은 보지 않는다 — 오래전에 멈춘 세션을 계정을 바꿀 때마다 깨우지 않게
pub const RECENT: f64 = 6.0 * 3600.0;
pub const PROMPT: &str = "사용 한도로 멈췄던 세션을 다른 계정으로 전환해 재개합니다. 중단된 지점부터 직전 작업을 이어서 진행해 주세요.";

#[derive(Default)]
pub struct AutoSwitch {
    /// 자동 전환을 요청하고 결과를 기다리는 중
    pending: bool,
    /// 이어 가기를 넣은 "세션 id@한도 시각" — 한 번만 넣는다
    resumed: HashSet<String>,
    /// 전환할 계정이 없다고 이미 알렸다 — 계정이 바뀌기 전에는 다시 알리지 않는다
    no_candidate: bool,
    /// 전환 실패로 잠시 쉬는 계정·시각 — S2 가 채운다(K1 에서는 자리만, decide 로 넘겨만 본다)
    pause: Option<Pause>,
    /// 지금 계정을 알 수 없다고 봤다 — S2 가 한 번만 알리는 데 쓴다(K1 에서는 자리만)
    #[allow(dead_code)]
    unknown_current: bool,
}

impl AutoSwitch {
    /// 계정이 바뀌었다(손으로 바꾼 것 포함) — 다시 알릴 수 있게 한다.
    pub fn switched(&mut self) {
        self.no_candidate = false;
    }
}

fn log(text: &str) {
    append_log(&paths::notifier_dir().join("auto-switch.log"), text);
}

/// 저장한 계정 가운데 지금 계정 다음부터 돌며 쓸 수 있는(다시 로그인 필요 아님, 한도 기록 없음) 첫 계정.
pub fn pick_next<'a>(saved: &'a [Account], cur: &str, marks: &BTreeMap<String, f64>, now: f64) -> Option<&'a Account> {
    let start = saved.iter().position(|a| a.id == cur).map_or(0, |i| i + 1);
    saved[start..].iter().chain(&saved[..start]).find(|a| a.id != cur && !a.expired(now) && !marks.contains_key(&a.id))
}

/// 한도에 걸려 멈춘 세션 — 종료·일시중지·승인 대기(Esc 가 승인을 거절한다)는 건드리지 않는다.
pub fn limited(sessions: &[Session], now: f64) -> Vec<Session> {
    sessions
        .iter()
        .filter(|s| !matches!(s.state.as_str(), "ended" | "paused" | "waiting") && s.limit_s.is_some_and(|at| now - at < RECENT))
        .cloned()
        .collect()
}

/// 지금 계정의 찬 5시간·주간 한도(풀리기 전).
pub fn full_limit(limits: &[Limit], now: f64) -> Option<&Limit> {
    limits.iter().find(|l| (l.id == "5h" || l.id == "7d") && l.pct >= 100.0 && l.resets.is_none_or(|r| r as f64 / 1000.0 > now))
}

fn key(s: &Session) -> String {
    format!("{}@{}", s.id, s.limit_s.unwrap_or(0.0))
}

/// 자동 전환 실패 뒤 잠시 쉬는 표시 — 그 계정을 `until` 유닉스 초까지 고르지 않는다(§plan 3.3).
/// K1 에서는 자리만 — S2 가 채워 읽는다.
#[allow(dead_code)]
pub struct Pause {
    pub account: String,
    pub until: f64,
}

/// 자동 전환의 판단 하나 — `decide` 가 내리는 결정 (§plan 3.3).
/// SwitchTo 가 Account 쌍을 통째로 옮겨 눈 하나에 256바이트를 넘긴다 — 계약이 정한 꼴을 그대로 두기 위해 허용한다.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Move {
    /// 지금 그대로 — 바꿀 까닭이 없다
    Stay,
    /// `from` 계정에서 `next` 계정으로 전환한다
    SwitchTo { next: Account, from: Account, reason: String, stopped: bool },
    /// 전환할 계정이 없다 — 이유는 이미 알렸다
    NoCandidate(String),
    /// 지금 계정을 알 수 없다 — K1 에서는 만들지 않는다(S2 가 채운다)
    #[allow(dead_code)]
    UnknownCurrent(String),
}

/// `decide` 의 결과 — 전환 결정과 한도 표시 갱신·이어 가기 목록.
pub struct Decision {
    /// 전환할 것인가
    pub mv: Move,
    /// 지금 계정의 찬 한도 표시를 갱신한 목록 — None 이면 건드리지 않는다
    pub full: Option<BTreeMap<String, f64>>,
    /// 이어 가기를 넣을 세션들
    pub resume: Vec<Session>,
}

/// 지금 상태에서 자동 전환 결정을 내린다 — 파일·프로세스를 만지지 않는 순수 판단(§plan 3.3).
/// 지금 계정을 모르면 전환하지 않는다 — 지금 동작처럼 조용히 건너뛰고 이어 가기만 내놓는다.
#[allow(clippy::too_many_arguments)]
pub fn decide(
    sessions: &[Session],
    limits: &[Limit],
    current: Option<&Account>,
    saved: &[Account],
    full: &BTreeMap<String, f64>,
    resumed: &HashSet<String>,
    last_switch: f64,
    pending: bool,
    switching: bool,
    _pause: Option<&Pause>,
    now: f64,
) -> Decision {
    let settled = last_switch + SETTLE;
    let limited = limited(sessions, now);
    // 이어 가기 — settle 이 지났으면 그 전(이전 계정)에 한도로 멈춘 세션, 아직 요청을 넣지 않은 것
    let resume = if now > settled {
        limited
            .iter()
            .filter(|s| s.limit_s.is_some_and(|at| at <= settled) && !resumed.contains(&key(s)))
            .cloned()
            .collect()
    } else {
        Vec::new()
    };
    let Some(cur) = current else {
        return Decision { mv: Move::Stay, full: None, resume };
    };
    // 대기·전환 중·전환 직후(settle 안)에는 보지 않는다
    let reason = if pending || switching || now <= settled {
        None
    } else if let Some(hit) = limited.iter().find(|s| s.limit_s.is_some_and(|at| at > settled)) {
        Some(format!("{} 세션이 한도에 걸림({})", hit.project(), hit.message))
    } else {
        full_limit(limits, now).map(|l| format!("{} 한도 {}%", l.label, l.pct as i64))
    };
    let Some(reason) = reason else {
        return Decision { mv: Move::Stay, full: None, resume };
    };
    // 표시 갱신 — 지금 계정의 찬 한도 풀림 시각(모르면 1시간 뒤)을 적고 만료된 표시는 지운다
    let resets = limits
        .iter()
        .filter(|l| l.pct >= 100.0)
        .filter_map(|l| l.resets.map(|r| r as f64 / 1000.0))
        .filter(|r| *r > now)
        .fold(None, |m: Option<f64>, r| Some(m.map_or(r, |m| m.max(r))));
    let mut marks = full.clone();
    marks.retain(|_, v| *v > now);
    marks.insert(cur.id.clone(), resets.unwrap_or(now + 3600.0));
    let stopped = sessions.iter().any(|s| s.limit_s.is_some() && s.state != "ended");
    let mv = match pick_next(saved, &cur.id, &marks, now).cloned() {
        Some(next) => Move::SwitchTo { next, from: cur.clone(), reason, stopped },
        None => Move::NoCandidate(reason),
    };
    Decision { mv, full: Some(marks), resume }
}

/// 전환이 실패했을 때의 표시·쉼 — 실패의 쪽을 본다(§plan 3.3). K1 지금 동작: 늘 전환하려던 대상 계정을 1시간 뒤까지 건너뛴다.
pub fn failed(_side: SwitchSide, target: &Account, _current: &Account, full: &BTreeMap<String, f64>, now: f64) -> (BTreeMap<String, f64>, Option<Pause>) {
    let mut marks = full.clone();
    marks.insert(target.id.clone(), now + 3600.0);
    (marks, None)
}

/// 3초마다.
pub fn tick(board: &Arc<Board>) {
    let (on, last) = {
        let st = board.settings.lock().unwrap();
        (st.auto_switch, st.auto_switch_last)
    };
    if !on {
        return;
    }
    let now = clock::now();
    let sessions: Vec<Session> = board.sessions();
    let limits = board.usage.lock().unwrap().limits.clone();
    let (switching, current, saved) = {
        let acc = board.accounts.lock().unwrap();
        (acc.switching, acc.current.clone(), acc.saved.clone())
    };
    let marks = board.settings.lock().unwrap().auto_switch_full.clone();
    let decision = {
        let auto = board.auto.lock().unwrap();
        decide(&sessions, &limits, current.as_ref(), &saved, &marks, &auto.resumed, last, auto.pending, switching, auto.pause.as_ref(), now)
    };

    // 1) 전환 — 결정을 옮긴다
    if let Some(full) = decision.full {
        board.update_settings(|st| st.auto_switch_full = full);
    }
    match decision.mv {
        Move::Stay => {}
        Move::UnknownCurrent(_) => {
            // K1 에서는 만들지 않는다 — S2 가 지금 계정을 모를 때의 알림을 채운다
        }
        Move::NoCandidate(reason) => {
            let mut notified = false;
            {
                let mut auto = board.auto.lock().unwrap();
                if !auto.no_candidate {
                    auto.no_candidate = true;
                    notified = true;
                }
            }
            if notified {
                log(&format!("{reason} — 전환할 계정 없음"));
                board.flash("사용 한도에 도달했지만 전환할 다른 계정이 없습니다 — 저장한 계정이 모두 한도에 도달했거나 다시 로그인이 필요합니다", true);
            }
        }
        Move::SwitchTo { next, from, reason, stopped } => {
            board.auto.lock().unwrap().pending = true;
            log(&format!("{reason} — {} 에서 {} 로 전환", from.email(), next.email()));
            let mut note = format!("사용 한도에 도달해 {} 계정으로 자동 전환했습니다", next.email());
            if stopped {
                note += &format!(" — 한도로 멈춘 세션은 {}초 뒤 이어서 진행합니다", SETTLE as i64);
            }
            let b = board.clone();
            let failed_next = next.clone();
            accounts::switch_to(
                board,
                next,
                Some(note),
                Some(Box::new(move |r| {
                    b.auto.lock().unwrap().pending = false;
                    let Err(e) = r else { return };
                    log(&format!("{} 로 전환하지 못함 — 1시간 동안 고르지 않음", failed_next.email()));
                    let (marks, pause) = {
                        let full = b.update_settings(|st| st.auto_switch_full.clone());
                        failed(e.side, &failed_next, &from, &full, now)
                    };
                    b.update_settings(|st| st.auto_switch_full = marks);
                    b.auto.lock().unwrap().pause = pause; // K1 에서는 늘 None — S2 가 채워 쓴다
                })),
            );
        }
    }

    // 2) 이어 가기 — 결정의 세션에 이어서 진행하라는 요청을 넣는다
    let todo = decision.resume;
    if todo.is_empty() {
        return;
    }
    {
        let mut auto = board.auto.lock().unwrap();
        auto.resumed.extend(todo.iter().map(key));
    }
    let mut manual = Vec::new();
    for s in &todo {
        let name = s.project();
        if !s.can_drive {
            log(&format!("{name} — 콘솔에 입력할 수 없는 세션이라 이어 가지 못함"));
            manual.push(name);
            continue;
        }
        let pid = s.pid;
        std::thread::spawn(move || {
            // Esc 를 먼저 — Claude Code 가 띄운 한도 선택 창을 닫고 "한도가 풀리면 저절로 이어 가기" 예약을 취소한다
            let inputs = [Input::Escape, Input::Pause(Duration::from_millis(500)), Input::Text(PROMPT), Input::Pause(Duration::from_millis(200)), Input::Enter];
            let ok = console::send_input(pid, &inputs);
            log(&format!("{name} — {}", if ok { "이어 가기 요청을 넣음" } else { "콘솔에 붙지 못해 이어 가지 못함" }));
        });
    }
    let n = todo.len() - manual.len();
    let mut text = if n > 0 { format!("한도로 멈췄던 세션 {n}개를 이어서 진행합니다") } else { String::new() };
    if !manual.is_empty() {
        text += &format!("{}{} 세션은 터미널에 입력할 수 없어 직접 이어 주세요", if n > 0 { " — " } else { "" }, manual.join(", "));
    }
    board.flash(text, !manual.is_empty());
}

// 시험 표 — T2 가 표 한 장으로 옮길 자리(§plan 3.3 C2). 지금은 비어 있다.
#[cfg(test)]
mod table;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn acc(uuid: &str, exp: Option<f64>) -> Account {
        Account::new(json!({"accountUuid": uuid, "emailAddress": format!("{uuid}@x")}).as_object().unwrap(), exp).unwrap()
    }

    #[test]
    fn picks_next_usable_account() {
        let saved = vec![acc("a", None), acc("b", Some(10.0)), acc("c", None), acc("d", None)];
        let none = BTreeMap::new();
        // 지금 계정 다음부터 돈다 — b 는 다시 로그인 필요라 건너뛴다
        assert_eq!(pick_next(&saved, "a|", &none, 100.0).unwrap().id, "c|");
        assert_eq!(pick_next(&saved, "c|", &none, 100.0).unwrap().id, "d|");
        // 끝에서 처음으로 돌아간다
        assert_eq!(pick_next(&saved, "d|", &none, 100.0).unwrap().id, "a|");
        // 한도 기록이 있는 계정은 고르지 않는다
        let marks = BTreeMap::from([("d|".to_string(), 200.0), ("a|".to_string(), 200.0)]);
        assert_eq!(pick_next(&saved, "c|", &marks, 100.0), None);
        // 저장 목록에 없는 지금 계정이면 처음부터
        assert_eq!(pick_next(&saved, "zzz|", &marks, 100.0).unwrap().id, "c|");
        // b 가 아직 유효하면 고른다
        assert_eq!(pick_next(&saved, "a|", &none, 5.0).unwrap().id, "b|");
        assert_eq!(pick_next(&[], "a|", &none, 5.0), None);
    }

    fn limit(id: &str, pct: f64, resets: Option<i64>) -> Limit {
        Limit { id: id.into(), label: id.into(), short: id.into(), tiny: id.into(), pct, resets }
    }

    #[test]
    fn full_limits_and_limited_sessions() {
        let now = 1000.0;
        assert!(full_limit(&[limit("5h", 99.0, None), limit("7d-Fable", 100.0, None)], now).is_none());
        assert_eq!(full_limit(&[limit("5h", 100.0, Some(2_000_000))], now).unwrap().id, "5h");
        // 이미 풀린 한도는 보지 않는다
        assert!(full_limit(&[limit("7d", 100.0, Some(500_000))], now).is_none());
        assert_eq!(full_limit(&[limit("7d", 120.0, None)], now).unwrap().id, "7d");
    }

    fn limited_session(id: &str, limit_s: f64) -> Session {
        use std::path::PathBuf;
        Session {
            id: id.into(),
            cwd: "C:/일".into(),
            state: "working".into(),
            since: 0,
            updated: 0,
            headless: false,
            title: String::new(),
            topic: String::new(),
            message: "작업 중".into(),
            name: None,
            ctx: None,
            brain: None,
            subagents: Vec::new(),
            shells: 0,
            paused: false,
            muted: false,
            limit_at: None,
            can_drive: true,
            place: crate::model::PidPlace::default(),
            pid: 7,
            pid_ctime: None,
            file: PathBuf::new(),
            project_dir: "일".into(),
            resume_args: Vec::new(),
            since_s: 0.0,
            updated_s: 0.0,
            limit_s: Some(limit_s),
        }
    }

    #[test]
    fn decide_ignores_unknown_current() {
        // 지금 계정을 모르면 전환 결정 없이 지금 동작 그대로 건너뛴다 — 이어 가기는 settle 뒤 내놓는다
        let s = limited_session("w1", 30.0);
        let d = decide(std::slice::from_ref(&s), &[], None, &[], &BTreeMap::new(), &HashSet::new(), 0.0, false, false, None, 50.0);
        assert!(matches!(d.mv, Move::Stay));
        assert!(d.full.is_none());
        assert_eq!(d.resume.len(), 1);
        assert_eq!(d.resume[0].id, "w1");

        // settle(40) 안이면 이어 가기도 없다
        let d = decide(&[limited_session("w1", 30.0)], &[], None, &[], &BTreeMap::new(), &HashSet::new(), 0.0, false, false, None, 10.0);
        assert!(d.resume.is_empty());
    }

    #[test]
    fn decide_switches_with_marks() {
        // 5h 한도가 찼다 — 지금 계정에서 다음 계정으로 전환을 내놓고 지금 계정의 한도 표시를 채운다
        let limits = vec![limit("5h", 100.0, None)];
        let cur = acc("cur", None);
        let saved = vec![cur.clone(), acc("next", None)];
        let d = decide(&[], &limits, Some(&cur), &saved, &BTreeMap::new(), &HashSet::new(), -100.0, false, false, None, 1000.0);
        let Move::SwitchTo { next, from, reason, stopped } = &d.mv else { panic!("전환 결정이 아니다: {:?}", d.mv) };
        assert_eq!(next.id, "next|");
        assert_eq!(from.id, "cur|");
        assert_eq!(reason, "5h 한도 100%");
        assert!(!stopped);
        let marks = d.full.clone().unwrap();
        assert_eq!(marks.get("cur|"), Some(&(1000.0 + 3600.0))); // 풀리는 시각을 모르면 1시간 뒤

        // 전환할 계정이 없으면 없다고 본다
        let only = vec![cur.clone()];
        let d = decide(&[], &limits, Some(&cur), &only, &BTreeMap::new(), &HashSet::new(), -100.0, false, false, None, 1000.0);
        let Move::NoCandidate(reason) = &d.mv else { panic!("없음 결정이 아니다: {:?}", d.mv) };
        assert_eq!(reason, "5h 한도 100%");
        assert!(d.full.is_some());
    }

    #[test]
    fn decide_switches_when_session_hits_limit() {
        // settle 뒤에 새로 걸린 세션이 있다 — 멈춘 세션이 있으니 전환 안내에 이어 진행 표시도 붙는다
        let cur = acc("cur", None);
        let saved = vec![cur.clone(), acc("next", None)];
        let hit = limited_session("w1", 50.0);
        let d = decide(&[hit], &[], Some(&cur), &saved, &BTreeMap::new(), &HashSet::new(), 0.0, false, false, None, 100.0);
        let Move::SwitchTo { reason, stopped, .. } = &d.mv else { panic!("전환 결정이 아니다: {:?}", d.mv) };
        assert!(reason.contains("세션이 한도에 걸림(작업 중)"), "{reason}");
        assert!(stopped);
    }

    #[test]
    fn failed_always_marks_target() {
        // K1 지금 동작 — 실패의 쪽과 관계없이 전환하려던 대상 계정을 1시간 뒤까지 건너뛰고 쉬지는 않는다
        let target = acc("t", None);
        let cur = acc("cur", None);
        let full = BTreeMap::from([("cur|".to_string(), 500.0)]);
        for side in [SwitchSide::Target, SwitchSide::Other, SwitchSide::Busy] {
            let (marks, pause) = failed(side, &target, &cur, &full, 1000.0);
            assert_eq!(marks.get("t|"), Some(&(1000.0 + 3600.0)), "쪽 {side:?}");
            assert_eq!(marks.get("cur|"), Some(&500.0), "쪽 {side:?}");
            assert!(pause.is_none(), "쪽 {side:?}");
        }
    }
}
