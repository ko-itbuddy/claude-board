//! (청크 Inbox 1/2 전반만 이식됨 — 이 첫 줄은 `tests/ported.rs` 가 「구현됐는데 PORTED.json 에 없는 규칙」으로 잡지 않게 하는 표시다. R-Inbox-b 가 후반을 옮긴 뒤 이 줄을 지우고 PORTED.json 항목을 넣는다)
//! 방향을 바꿀 말(`inbox/<시각>.md`)의 글 꼴·어느 단위에 붙일지·언제 넘길지를 정하는 규칙. 원본: `Sources/AutopilotRules/Inbox.swift`.
//! 순수 함수만 있다(파일·시각·환경 없음).
//!
//! 이식된 선언(원본 순서, 원본 1~142 줄): `InboxTo`(APInboxTo) · `InboxNote`(APInboxNote) · `INBOX_MAX_CHARS`·`INBOX_MAX_BYTES`(apInboxMaxChars·Bytes) ·
//! `compose`(apInboxCompose) · `name_ok`(apInboxNameOK) · `InboxParse`(APInboxParse) · `parse`(apInboxParse) · `InboxEntry`(APInboxEntry) ·
//! `INBOX_STATUSES`(apInboxStatuses) · `InboxBind`(APInboxBind) · `bind`(apInboxBind) · 비공개 `unit_word`(apInboxUnitWord) ·
//! `now_for`(apInboxNowFor) · `next_for`(apInboxNextFor).
//! 남은 선언(R-Inbox-b, 원본 「// 모델에게 넘기는 글」 주석(144 줄)부터 끝까지): `apInboxQuote`(비공개) · `apInboxScope`(비공개) · `apInboxNowText` ·
//! `apInboxNextText` · `apInboxPassedText` · `APAfterTurn`·`apAfterTurn` · `APAfterCheck`·`apAfterCheck` · `apInboxCloseUnit` · `apInboxCloseRun` ·
//! 포트·유스케이스(`APInboxRead`·`APInboxFiles`·`apInboxReceive`·`APInboxOut`·`APInboxSend`·`apInboxSend`) · `apInboxLabel` · `apInboxReceivedText`.
//! 원본 시험 파일 끝의 `RestartTests`(멈추고 다시 출발, `Control.swift` 규칙)도 R-Inbox-b 몫이다 — 규칙은 `rules::control` 에 이미 있다.
//!
//! Foundation 과 다른 점(원본 Swift 는 `Character`=자소 묶음 단위, 여기는 유니코드 글자(`char`) 단위):
//! - 본문 길이 한도(`count` ≤ 2000)와 머리 오류 글의 `prefix(40)`·`prefix(20)` 은 `chars()` 로 센다. 결합 글자·ZWJ 그림 글자는 원본이 하나로 세는 것을 여기서는 여럿으로 센다.
//! - 머리 줄의 `firstIndex(of: ":")` 는 바이트로 찾는다(쌍점 바로 뒤에 결합 글자가 붙은 줄에서만 갈린다).
//! - `trimmingCharacters` 의 `.whitespaces`(Zs+탭)는 `is_ws`, `.whitespacesAndNewlines`(Zs·Zl·Zp·U+0009~000D·U+0085)는 `char::is_whitespace` 와 같은 집합이다.
//! - `apInboxNameOK` 의 정규식 `$` 는 ICU 에서 끝 줄바꿈 앞에서도 맞지만, 여기는 끝 줄바꿈이 붙은 이름을 받지 않는다(파일 이름에 줄바꿈은 들어갈 수 없다).
//! - `Double("…")` 은 16진 실수(`0x1p3`)도 읽지만 `f64::from_str` 은 읽지 않는다(그런 `at` 머리는 여기서 `bad`). 무한·NaN 은 둘 다 `bad`.
//! - `compose` 의 `at` 은 `{:.3}` 으로 쓴다. 유한한 값은 `%.3f` 와 같고, NaN 은 `NaN`(Swift 는 `nan`)으로 나온다.
//! - `apInboxBind` 에 `unit <= 0` 이 직접 들어오면 Swift 는 배열 범위 밖으로 죽는다. 여기는 「그 단위가 이 실행에 없음」으로 `late` 를 낸다(`parse` 는 1 이상만 내므로 닿지 않는다).
//! - `InboxEntry` 의 Codable 은 serde 로 옮겼다: 옵셔널은 비면 키를 쓰지 않고, 정수인 시각은 정수로 쓴다(Swift JSONEncoder 와 같게). 키 차례는 선언 차례다.

use serde::{Deserialize, Serialize, Serializer};

/// 말을 어디에 붙이는가. 원본: `APInboxTo`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InboxTo {
    /// 지금 도는 단위에
    Now,
    /// 다음 단위부터
    Next,
}

impl InboxTo {
    /// 원본 `rawValue`
    pub fn as_str(self) -> &'static str {
        match self {
            InboxTo::Now => "now",
            InboxTo::Next => "next",
        }
    }

    /// 원본 `APInboxTo(rawValue:)` — 정확히 `now`·`next` 만
    pub fn from_raw(s: &str) -> Option<InboxTo> {
        match s {
            "now" => Some(InboxTo::Now),
            "next" => Some(InboxTo::Next),
            _ => None,
        }
    }
}

/// 파일 하나의 글 — 머리(빈 줄 앞까지 "이름: 값" 줄)와 본문. 원본: `APInboxNote`
#[derive(Debug, Clone, PartialEq)]
pub struct InboxNote {
    pub to: InboxTo,
    /// 보드가 보낼 때 본 지금 단위(그 단위가 돌고 있을 때만 적는다)
    pub unit: Option<i64>,
    /// 보낸 시각(유닉스 초)
    pub at: Option<f64>,
    pub text: String,
}

impl InboxNote {
    /// 원본 `APInboxNote(to:text:)` — `unit`·`at` 은 비운다
    pub fn new(to: InboxTo, text: &str) -> InboxNote {
        InboxNote { to, unit: None, at: None, text: text.to_string() }
    }
}

/// 본문 글자 수 상한(보드 입력 칸과 같음). 원본: `apInboxMaxChars`
pub const INBOX_MAX_CHARS: usize = 2000;
/// 파일 크기 상한(엔진이 이보다 크면 읽지 않는다). 원본: `apInboxMaxBytes`
pub const INBOX_MAX_BYTES: usize = 16_384;

// Foundation `CharacterSet.whitespaces`: 일반 범주 Zs 와 탭(U+0009)
fn is_ws(c: char) -> bool {
    matches!(c, '\t' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

fn trim_ws(s: &str) -> &str {
    s.trim_matches(is_ws)
}

// Foundation `CharacterSet.whitespacesAndNewlines` — 유니코드 White_Space 와 같은 집합
fn trim_all(s: &str) -> &str {
    s.trim_matches(char::is_whitespace)
}

fn prefix_chars(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// 보드가 쓰는 글. 본문은 앞뒤 빈칸을 다듬고 줄 끝은 `\n` 으로. 원본: `apInboxCompose`
pub fn compose(n: &InboxNote) -> String {
    let mut h = format!("to: {}\n", n.to.as_str());
    if let Some(u) = n.unit {
        h.push_str(&format!("unit: {u}\n"));
    }
    if let Some(a) = n.at {
        h.push_str(&format!("at: {a:.3}\n"));
    }
    let body = n.text.replace("\r\n", "\n");
    format!("{h}\n{}\n", trim_all(&body))
}

/// 파일 이름 꼴 — 보드는 `<yyyyMMdd-HHmmss-SSS>[-k].md` 로 쓴다. 엔진은 이 꼴만 읽는다
/// (임시 파일 `.tmp-…`·숨은 파일·다른 확장자는 넘김). 원본: `apInboxNameOK`, 정규식 `^[0-9A-Za-z][0-9A-Za-z._-]{0,80}\.md$`
pub fn name_ok(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".md") else {
        return false;
    };
    let b = stem.as_bytes();
    if b.is_empty() || b.len() > 81 || !b[0].is_ascii_alphanumeric() {
        return false;
    }
    b.iter().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
}

/// 엔진이 읽은 글. 원본: `APInboxParse`
#[derive(Debug, Clone, PartialEq)]
pub enum InboxParse {
    Note(InboxNote),
    /// 읽지 않는 까닭(쉬운 말)
    Bad(String),
}

/// 엔진이 읽는 글. 머리 `to` 는 꼭 있어야 하고(now·next), `unit` 은 1 이상의 정수, `at` 은 수.
/// 모르는 머리 칸은 넘긴다(뒤에 칸이 늘어도 읽게). 본문이 비었거나 상한보다 길면 읽지 않는다. 원본: `apInboxParse`
pub fn parse(raw: &str) -> InboxParse {
    let s = raw.replace("\r\n", "\n");
    let lines: Vec<&str> = s.split('\n').collect();
    let Some(blank) = lines.iter().position(|l| trim_ws(l).is_empty()) else {
        return InboxParse::Bad("머리와 본문 사이 빈 줄이 없습니다".to_string());
    };
    let mut to: Option<InboxTo> = None;
    let mut unit: Option<i64> = None;
    let mut at: Option<f64> = None;
    for l in &lines[..blank] {
        let Some(c) = l.find(':') else {
            return InboxParse::Bad(format!("머리 줄 꼴이 아닙니다: {}", prefix_chars(l, 40)));
        };
        let k = trim_ws(&l[..c]).to_lowercase();
        let v = trim_ws(&l[c + 1..]);
        match k.as_str() {
            "to" => match InboxTo::from_raw(v) {
                Some(t) => to = Some(t),
                None => return InboxParse::Bad(format!("머리 to 는 now 또는 next 입니다({})", prefix_chars(v, 20))),
            },
            "unit" => match v.parse::<i64>() {
                Ok(u) if u >= 1 => unit = Some(u),
                _ => return InboxParse::Bad(format!("머리 unit 은 1 이상의 정수입니다({})", prefix_chars(v, 20))),
            },
            "at" => match v.parse::<f64>() {
                Ok(a) if a.is_finite() => at = Some(a),
                _ => return InboxParse::Bad(format!("머리 at 은 수입니다({})", prefix_chars(v, 20))),
            },
            _ => continue,
        }
    }
    let Some(to) = to else {
        return InboxParse::Bad("머리에 to: now 또는 to: next 가 없습니다".to_string());
    };
    let text = trim_all(&lines[blank + 1..].join("\n")).to_string();
    if text.is_empty() {
        return InboxParse::Bad("본문이 비었습니다".to_string());
    }
    let n = text.chars().count();
    if n > INBOX_MAX_CHARS {
        return InboxParse::Bad(format!("본문이 {INBOX_MAX_CHARS}자보다 깁니다({n}자)"));
    }
    InboxParse::Note(InboxNote { to, unit, at, text })
}

// 시각(유닉스 초)이 정수면 정수로 쓴다 — Swift JSONEncoder 가 1793300000 으로 쓰는 것과 같게(소수가 있으면 그대로)
fn secs_value(at: f64) -> serde_json::Value {
    if at.is_finite() && at.fract() == 0.0 && at.abs() < 9e15 {
        serde_json::Value::from(at as i64)
    } else {
        serde_json::Value::from(at)
    }
}

fn ser_secs<S: Serializer>(at: &f64, s: S) -> Result<S::Ok, S::Error> {
    secs_value(*at).serialize(s)
}

fn ser_secs_opt<S: Serializer>(at: &Option<f64>, s: S) -> Result<S::Ok, S::Error> {
    match at {
        Some(a) => secs_value(*a).serialize(s),
        None => s.serialize_none(),
    }
}

/// `run.json` 의 inbox 한 줄 — 받은 말 하나와 그 말이 어떻게 됐는지(보드가 그린다). 원본: `APInboxEntry`
/// (Swift 의 긴 `init` 은 기본값 인자가 있다 — 여기는 필수 다섯을 `new` 로 받고 나머지는 필드로 채운다)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxEntry {
    /// `inbox/` 안 파일 이름(같은 파일을 다시 읽지 않는 열쇠)
    pub file: String,
    /// now·next(읽지 못한 파일은 빈 글자)
    pub to: String,
    pub text: String,
    /// 머리 at
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_secs_opt")]
    pub sent: Option<f64>,
    /// 머리 unit(보낼 때 본 단위)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seen: Option<i64>,
    /// 엔진이 받은 시각
    #[serde(serialize_with = "ser_secs")]
    pub received: f64,
    /// `INBOX_STATUSES` 가운데 하나
    pub status: String,
    /// now: 넘길 단위
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<i64>,
    /// next: 붙이기 시작할 단위
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<i64>,
    /// 넘긴(now)·붙인(next) 단위
    pub units: Vec<i64>,
    /// 처음 넘긴·붙인 시각
    #[serde(default, skip_serializing_if = "Option::is_none", serialize_with = "ser_secs_opt")]
    pub done: Option<f64>,
    /// late·dropped·unused·bad 의 까닭
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// 없으면 사람이 보드에서 보낸 말, takeover 면 돌려줄 때 엔진이 만든 이어받기 지시(⑫-8, `Takeover.swift`). 없는 칸은 옛 꼴 그대로 읽힌다
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

impl InboxEntry {
    /// 원본 `APInboxEntry(file:to:text:received:status:)` — 나머지 칸은 비워 둔다
    pub fn new(file: &str, to: &str, text: &str, received: f64, status: &str) -> InboxEntry {
        InboxEntry {
            file: file.to_string(),
            to: to.to_string(),
            text: text.to_string(),
            sent: None,
            seen: None,
            received,
            status: status.to_string(),
            unit: None,
            from: None,
            units: Vec::new(),
            done: None,
            why: None,
            source: None,
        }
    }
}

/// waiting: 넘길·붙일 차례를 기다림, delivered: now 를 세션에 넘김, applied: next 를 단위 첫 입력에 붙임(뒤 단위에도 이어 붙는다),
/// late: 받았을 때 이미 넘길 곳이 없음, dropped: now 를 넘기기 전에 그 단위가 끝남, unused: next 를 붙일 단위가 시작하지 않고 실행이 끝남,
/// bad: 읽지 못함. 원본: `apInboxStatuses`
pub const INBOX_STATUSES: [&str; 7] = ["waiting", "delivered", "applied", "late", "dropped", "unused", "bad"];

/// 받은 말이 어느 단위에 붙는지의 결과. 원본: `APInboxBind`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InboxBind {
    /// waiting 또는 late
    pub status: String,
    pub unit: Option<i64>,
    pub from: Option<i64>,
    pub why: Option<String>,
}

impl InboxBind {
    fn waiting_unit(unit: i64) -> InboxBind {
        InboxBind { status: "waiting".to_string(), unit: Some(unit), from: None, why: None }
    }

    fn waiting_from(from: i64) -> InboxBind {
        InboxBind { status: "waiting".to_string(), unit: None, from: Some(from), why: None }
    }

    fn late(why: String) -> InboxBind {
        InboxBind { status: "late".to_string(), unit: None, from: None, why: Some(why) }
    }
}

/// 받은 말을 어느 단위에 붙일지 — `statuses` 는 `queue.json` 차례의 단위 상태(running 은 일시정지·한도 대기 중인 단위도).
/// - now: 보낼 때 본 단위(`seen`)가 있으면 그 단위가 아직 돌거나 시작 전일 때만 그 단위, 끝났으면 넘기지 않는다(다른 단위의 일에 그 말이 섞이지 않게).
///   없으면 지금 도는 단위, 도는 단위가 없으면(멈출 선 대기·단위 사이) 다음에 시작할 단위의 첫 입력
/// - next: 지금 도는 단위 뒤(도는 단위가 없으면 처음부터)의 첫 대기 단위부터 남은 단위 모두
///
/// 원본: `apInboxBind`
pub fn bind<S: AsRef<str>>(n: &InboxNote, statuses: &[S]) -> InboxBind {
    let running = statuses.iter().position(|s| s.as_ref() == "running").map(|i| i as i64 + 1);
    let first_pending = |after: i64| -> Option<i64> {
        statuses.iter().enumerate().find(|(i, s)| *i as i64 + 1 > after && s.as_ref() == "pending").map(|(i, _)| i as i64 + 1)
    };
    match n.to {
        InboxTo::Now => {
            if let Some(s) = n.unit {
                if s < 1 || s > statuses.len() as i64 {
                    return InboxBind::late(format!("보낼 때 본 단위(단위 {s})가 이 실행에 없어 넘기지 않았습니다"));
                }
                let st = statuses[(s - 1) as usize].as_ref();
                if st == "running" || st == "pending" {
                    return InboxBind::waiting_unit(s);
                }
                return InboxBind::late(format!(
                    "보낼 때 본 단위(단위 {s})가 이미 끝나({}) 넘기지 않았습니다. 다음 단위부터로 다시 보내거나 멈추고 다시 출발합니다",
                    unit_word(st)
                ));
            }
            if let Some(r) = running {
                return InboxBind::waiting_unit(r);
            }
            if let Some(p) = first_pending(0) {
                return InboxBind::waiting_unit(p);
            }
            InboxBind::late("남은 단위가 없어 넘기지 않았습니다".to_string())
        }
        InboxTo::Next => {
            if let Some(p) = first_pending(running.unwrap_or(0)) {
                return InboxBind::waiting_from(p);
            }
            InboxBind::late("뒤에 남은 단위가 없어 붙일 곳이 없습니다".to_string())
        }
    }
}

// 끝난 단위 상태의 쉬운 말. 원본: `apInboxUnitWord`(비공개)
fn unit_word(s: &str) -> &str {
    match s {
        "done" => "통과",
        "failed" => "미통과",
        "capped" => "상한에서 멈춤",
        "stopped" => "중단",
        "skipped" => "건너뜀",
        other => other,
    }
}

/// 단위 n 의 세션에 넘길 말(now, 기다림, 그 단위) — 받은 차례대로 자리 번호. 원본: `apInboxNowFor`
pub fn now_for(entries: &[InboxEntry], unit: i64) -> Vec<usize> {
    (0..entries.len())
        .filter(|&i| entries[i].to == "now" && entries[i].status == "waiting" && entries[i].unit == Some(unit))
        .collect()
}

/// 단위 n 의 첫 입력에 붙일 방향(next, 기다림·붙임, 붙이기 시작할 단위가 n 이하) — 받은 차례대로. 원본: `apInboxNextFor`
pub fn next_for(entries: &[InboxEntry], unit: i64) -> Vec<usize> {
    (0..entries.len())
        .filter(|&i| {
            let e = &entries[i];
            e.to == "next" && (e.status == "waiting" || e.status == "applied") && e.from.unwrap_or(i64::MAX) <= unit && !e.units.contains(&unit)
        })
        .collect()
}
