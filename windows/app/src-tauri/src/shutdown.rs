//! 끌 때 확인과 탭 목록 저장 — 트레이 "종료"·업데이트 직전에 도는 세션을 보여 주고 확인을 받는다(mac `confirmShutdown`),
//! 다시 뜰 때 이어 열 탭 목록을 설정에 남긴다(mac `saveTabs`·`restoreTabs`).
//!
//! 확인 창은 보드 창 안의 Svelte 모달이다. 백엔드가 `board://confirm-shutdown` 을 보내면 프론트가 목록을 만들어(규칙은 프론트
//! `terminal/shutdownList.ts`) 줄이 없으면 바로 `answer_shutdown` 으로 답하고, 줄이 있으면 `shutdown_dialog_shown` 으로 창을 띄웠다고 알린 뒤
//! 사용자가 고르면 `answer_shutdown` 으로 답한다. 백엔드는 답을 받아 `.paused` 표지를 쓰고(`terminal.rs paused_mark` 경로) 끝낸다.
//!
//! 기다림은 두 단계다. 이벤트를 보낸 뒤 프론트가 반응(답 또는 "창을 띄움")하기를 `REACTION_TIMEOUT` 동안 기다리고, 반응이 없으면
//! 프론트가 없거나 멎은 것이라 로그를 남기고 그대로 진행한다. "창을 띄움" 뒤에는 사용자가 고를 때까지 기다린다 — 사람이 읽는 시간을 끊지 않는다.
//! 프론트가 수명 주기를 아직 켜지 않았으면(탭 목록을 한 번도 저장하지 않음 = 탭이 없다) 묻지 않고 지나간다.

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use claude_board_core::clock;
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::board::Board;
use crate::util::board_log;

/// 끄기 전에 프론트에 확인 창을 띄우라고 보내는 이벤트 — 내용은 `types.ts ConfirmShutdownPayload`(목록은 프론트가 채운다).
pub const CONFIRM_SHUTDOWN_EVENT: &str = "board://confirm-shutdown";

/// 이벤트를 보낸 뒤 프론트의 첫 반응을 기다리는 시간 — 넘으면 프론트가 없는 것으로 보고 진행한다(판단값).
const REACTION_TIMEOUT: Duration = Duration::from_secs(10);

/// 끄는 까닭 — 이벤트의 `reason` 이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Quit,
    Update,
}

impl Reason {
    fn as_str(self) -> &'static str {
        match self {
            Reason::Quit => "quit",
            Reason::Update => "update",
        }
    }
}

/// 프론트가 확인 창에서 고른 것.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub ok: bool,
    /// `.paused` 를 남길 세션 id
    pub pause: Vec<String>,
}

/// 확인 요청의 끝.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Answered(Answer),
    /// 프론트가 `REACTION_TIMEOUT` 안에 반응하지 않았다
    TimedOut,
    /// 이미 다른 확인이 기다리는 중이다
    Busy,
}

enum Message {
    Shown,
    Answer(Answer),
}

/// 확인 요청과 프론트의 답을 이어 주는 한 칸짜리 우편함. 한 번에 요청 하나만 받는다.
pub struct Handshake {
    waiting: Mutex<Option<Sender<Message>>>,
}

impl Handshake {
    pub const fn new() -> Self {
        Handshake { waiting: Mutex::new(None) }
    }

    /// `emit` 으로 요청을 보내고 답을 기다린다. 우편함은 `emit` 전에 열어 두므로 답이 먼저 와도 놓치지 않는다.
    pub fn ask(&self, emit: impl FnOnce(), reaction_timeout: Duration) -> Outcome {
        let (sender, receiver) = channel();
        {
            let mut waiting = self.waiting.lock().unwrap();
            if waiting.is_some() {
                return Outcome::Busy;
            }
            *waiting = Some(sender);
        }
        emit();
        let outcome = match receiver.recv_timeout(reaction_timeout) {
            Ok(Message::Answer(answer)) => Outcome::Answered(answer),
            Ok(Message::Shown) => match receiver.recv() {
                Ok(Message::Answer(answer)) => Outcome::Answered(answer),
                // 창을 띄운 쪽이 사라졌다(보낸 쪽이 닫힘) — 답이 없는 것과 같다
                _ => Outcome::TimedOut,
            },
            Err(_) => Outcome::TimedOut,
        };
        *self.waiting.lock().unwrap() = None;
        outcome
    }

    fn send(&self, message: Message) -> bool {
        self.waiting.lock().unwrap().as_ref().is_some_and(|sender| sender.send(message).is_ok())
    }

    /// 프론트의 답을 기다리는 요청에 전한다. 기다리는 요청이 없으면 false.
    pub fn answer(&self, answer: Answer) -> bool {
        self.send(Message::Answer(answer))
    }

    /// 프론트가 확인 창을 띄웠다고 알린다 — 이 뒤로는 사용자가 고를 때까지 기다린다.
    pub fn shown(&self) -> bool {
        self.send(Message::Shown)
    }
}

static HANDSHAKE: Handshake = Handshake::new();

/// 프론트가 탭 목록을 한 번이라도 저장했다 = 프론트의 수명 주기(듣기·저장)가 켜졌다.
static LIFECYCLE_ACTIVE: AtomicBool = AtomicBool::new(false);

/// 끄거나 업데이트를 진행해도 되는지와 `.paused` 를 남길 세션 — 순수한 판단이다.
#[derive(Debug, PartialEq, Eq)]
pub struct Decision {
    pub proceed: bool,
    pub pause: Vec<String>,
    /// 사용자에게 보이지 않는 사정이면 board.log 에 남길 한 줄
    pub log: Option<&'static str>,
}

pub fn decide(outcome: Outcome) -> Decision {
    match outcome {
        Outcome::Answered(Answer { ok: true, pause }) => Decision { proceed: true, pause, log: None },
        Outcome::Answered(Answer { ok: false, .. }) => Decision { proceed: false, pause: Vec::new(), log: None },
        Outcome::TimedOut => Decision { proceed: true, pause: Vec::new(), log: Some("끌 때 확인에 프론트가 답하지 않아 그대로 진행합니다") },
        Outcome::Busy => Decision { proceed: false, pause: Vec::new(), log: None },
    }
}

/// 끄기 전에 확인을 받는다. 계속해도 되면 true(`.paused` 표지는 여기서 쓴다), 취소하거나 이미 다른 확인이 열려 있으면 false.
fn confirm(app: &AppHandle, reason: Reason) -> bool {
    if !LIFECYCLE_ACTIVE.load(Ordering::SeqCst) {
        return true;
    }
    let payload = json!({"reason": reason.as_str(), "items": []});
    let outcome = HANDSHAKE.ask(
        || {
            if let Err(e) = app.emit(CONFIRM_SHUTDOWN_EVENT, payload) {
                board_log(&format!("끌 때 확인 이벤트를 보내지 못했습니다: {e}"));
            }
        },
        REACTION_TIMEOUT,
    );
    let decision = decide(outcome);
    if let Some(line) = decision.log {
        board_log(line);
    }
    if decision.proceed && !decision.pause.is_empty() {
        if let Some(board) = app.try_state::<Arc<Board>>() {
            write_marks(&board, &decision.pause);
        }
    }
    decision.proceed
}

/// 트레이 "종료". 도는 세션이 있으면 확인을 받은 뒤 끝낸다. 메인 스레드를 막지 않게 따로 돈다.
pub fn quit(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        if confirm(&app, Reason::Quit) {
            app.exit(0);
        }
    });
}

/// 업데이트를 시작해도 되는지. 도는 세션이 있으면 확인을 받아 취소하면 false. 호출한 스레드가 답이 올 때까지 막힌다.
pub fn confirm_update(board: &Board) -> bool {
    confirm(&board.app, Reason::Update)
}

/// 일시중지 표지를 남겨도 되는 세션인가 — 사람이 입력하는 대화형 주 세션만이다(끝난 세션·`-p`·`--bg` 는 입력 자리가 없다).
pub fn pausable(state: &str, headless: bool, mode: Option<&str>) -> bool {
    state != "ended" && !headless && mode.is_none()
}

/// 표지 파일 하나를 쓴다. 이미 있으면(사용자가 일시중지해 둔 세션) 덮지 않고 false.
pub fn write_mark(path: &Path, paused_at: f64, topic: &str) -> bool {
    use std::io::Write;
    let body = serde_json::to_vec(&json!({"paused_at": paused_at, "topic": topic})).unwrap_or_default();
    match fs::OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => file.write_all(&body).is_ok(),
        Err(_) => false,
    }
}

/// 프론트가 고른 세션들에 `.paused` 를 남긴다. 알려진 세션이 아니거나 `pausable` 이 아니면 건너뛴다. 새로 쓴 표지 수를 돌려준다.
fn write_marks(board: &Arc<Board>, ids: &[String]) -> usize {
    let now = clock::now();
    let mut written = 0;
    for id in ids {
        let Some(session) = board.session(id) else { continue };
        if !pausable(&session.state, session.headless, session.place.mode) {
            continue;
        }
        if write_mark(&crate::terminal::paused_mark(board, &session), now, &session.topic) {
            written += 1;
        }
    }
    if written > 0 {
        board.publish();
    }
    written
}

/// 프론트의 `answerShutdown(ok, pause)` — 확인 창의 답. `pause` 는 `.paused` 를 남길 세션 id(`ok` 일 때만 쓰인다).
#[tauri::command(async)]
pub fn answer_shutdown(ok: bool, pause: Vec<String>) {
    if !HANDSHAKE.answer(Answer { ok, pause }) {
        board_log("기다리는 끌 때 확인이 없는데 답이 왔습니다");
    }
}

/// 프론트의 `shutdownDialogShown()` — 확인 창을 띄웠다. 이 뒤로는 사용자가 고를 때까지 기다린다.
#[tauri::command(async)]
pub fn shutdown_dialog_shown() {
    HANDSHAKE.shown();
}

/// 프론트의 `saveTabs(entries)` — 다시 뜰 때 이어 열 탭 목록을 설정(`terminalTabs`)에 통째로 바꿔 넣는다. 항목 모양은 프론트가 쥔다.
/// 첫 저장은 프론트의 수명 주기가 켜졌다는 뜻이라 이 뒤부터 끌 때 확인을 건다.
#[tauri::command(async)]
pub fn save_tabs(board: State<'_, Arc<Board>>, entries: Vec<Value>) {
    LIFECYCLE_ACTIVE.store(true, Ordering::SeqCst);
    board.update_settings(|s| s.terminal_tabs = entries);
}

/// 프론트의 `loadTabs()` — 저장해 둔 탭 목록. 없으면 빈 목록.
#[tauri::command(async)]
pub fn load_tabs(board: State<'_, Arc<Board>>) -> Vec<Value> {
    board.settings.lock().unwrap().terminal_tabs.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use std::time::Instant;

    const FAST: Duration = Duration::from_millis(50);

    fn answer(ok: bool, pause: &[&str]) -> Answer {
        Answer { ok, pause: pause.iter().map(|id| id.to_string()).collect() }
    }

    #[test]
    fn 답이_오면_그대로_돌려준다() {
        let handshake = Arc::new(Handshake::new());
        let sender = handshake.clone();
        let outcome = handshake.ask(
            move || {
                thread::spawn(move || {
                    assert!(sender.answer(answer(true, &["a"])));
                });
            },
            Duration::from_secs(5),
        );
        assert_eq!(outcome, Outcome::Answered(answer(true, &["a"])));
    }

    #[test]
    fn 이벤트를_보내는_중에_온_답도_놓치지_않는다() {
        let handshake = Arc::new(Handshake::new());
        let sender = handshake.clone();
        let outcome = handshake.ask(move || assert!(sender.answer(answer(false, &[]))), Duration::from_secs(5));
        assert_eq!(outcome, Outcome::Answered(answer(false, &[])));
    }

    #[test]
    fn 반응이_없으면_시간이_지난_뒤_시간초과() {
        let handshake = Handshake::new();
        let started = Instant::now();
        let outcome = handshake.ask(|| {}, FAST);
        assert_eq!(outcome, Outcome::TimedOut);
        assert!(started.elapsed() >= FAST);
    }

    #[test]
    fn 창을_띄운_뒤에는_반응_시간을_넘겨도_사용자의_답을_기다린다() {
        let handshake = Arc::new(Handshake::new());
        let sender = handshake.clone();
        let outcome = handshake.ask(
            move || {
                thread::spawn(move || {
                    assert!(sender.shown());
                    thread::sleep(FAST * 4);
                    assert!(sender.answer(answer(true, &["b"])));
                });
            },
            FAST,
        );
        assert_eq!(outcome, Outcome::Answered(answer(true, &["b"])));
    }

    #[test]
    fn 요청이_끝나면_우편함이_비어_다음_요청을_받는다() {
        let handshake = Handshake::new();
        assert_eq!(handshake.ask(|| {}, Duration::from_millis(10)), Outcome::TimedOut);
        assert!(!handshake.answer(answer(true, &[])), "기다리는 요청이 없으면 답을 받지 않는다");
        assert_eq!(handshake.ask(|| {}, Duration::from_millis(10)), Outcome::TimedOut);
    }

    #[test]
    fn 이미_기다리는_요청이_있으면_둘째는_바쁨() {
        let handshake = Arc::new(Handshake::new());
        let inner = handshake.clone();
        let outcome = handshake.ask(
            move || {
                assert_eq!(inner.ask(|| panic!("둘째는 이벤트를 보내지 않는다"), FAST), Outcome::Busy);
                assert!(inner.answer(answer(true, &[])));
            },
            Duration::from_secs(5),
        );
        assert_eq!(outcome, Outcome::Answered(answer(true, &[])));
    }

    #[test]
    fn 판단_계속을_고르면_진행하고_표지_대상을_넘긴다() {
        let d = decide(Outcome::Answered(answer(true, &["a", "b"])));
        assert_eq!(d, Decision { proceed: true, pause: vec!["a".into(), "b".into()], log: None });
    }

    #[test]
    fn 판단_취소하면_표지도_남기지_않는다() {
        let d = decide(Outcome::Answered(answer(false, &["a"])));
        assert_eq!(d, Decision { proceed: false, pause: vec![], log: None });
    }

    #[test]
    fn 판단_답이_없으면_로그를_남기고_진행한다() {
        let d = decide(Outcome::TimedOut);
        assert!(d.proceed);
        assert!(d.pause.is_empty());
        assert!(d.log.is_some());
    }

    #[test]
    fn 판단_이미_열린_확인이_있으면_진행하지_않는다() {
        let d = decide(Outcome::Busy);
        assert!(!d.proceed);
        assert!(d.log.is_none());
    }

    #[test]
    fn 사람이_입력하는_주_세션만_표지를_받는다() {
        assert!(pausable("idle", false, None));
        assert!(pausable("working", false, None));
        assert!(pausable("waiting", false, None));
        assert!(!pausable("ended", false, None));
        assert!(!pausable("working", true, Some("-p")));
        assert!(!pausable("working", false, Some("-p")));
        assert!(!pausable("idle", false, Some("--bg")));
    }

    #[test]
    fn 표지_파일은_모양을_지키고_이미_있으면_덮지_않는다() {
        let dir = std::env::temp_dir().join(format!("cb-shutdown-{}", crate::util::short_id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s1.paused");
        assert!(write_mark(&path, 12.5, "주제"));
        let mark: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(mark, json!({"paused_at": 12.5, "topic": "주제"}));
        assert!(!write_mark(&path, 99.0, "다른"), "있는 표지는 덮지 않는다");
        let again: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(again["paused_at"], 12.5);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn 이벤트_이름과_까닭_글자는_프론트와_같다() {
        assert_eq!(CONFIRM_SHUTDOWN_EVENT, "board://confirm-shutdown");
        assert_eq!(Reason::Quit.as_str(), "quit");
        assert_eq!(Reason::Update.as_str(), "update");
    }
}
