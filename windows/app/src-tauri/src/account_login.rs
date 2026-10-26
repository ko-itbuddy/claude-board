//! 계정 추가 로그인(설정 > 계정) — 보드가 ConPTY 로 `claude auth login --claudeai` 를 돌리고(`CLAUDE_CONFIG_DIR=<슬롯>`, 1000열),
//! 출력에서 주소를 찾아(`login_output.rs`) 프론트에 알리고, 사용자가 붙여 넣은 코드를 그 ConPTY 에 넣는다.
//! 끝났는지는 보드가 그 슬롯을 지켜보는 `accounts::tick`(15분 한도)이 가르고, 이 모듈은 그 결과(`Host::pending`)를 따라 프로세스를 거둔다.
//!
//! - `machine.rs` — 순수 규칙. 출력·종료·코드·끝남을 받아 할 일(`Effect`)만 돌려준다.
//! - `login_window.rs` — 로그인 주소를 여는 보드 소유 창(닫으면 취소).
//! - 여기 — 프로세스(ConPTY)·스레드·명령. 보드와는 `Host` 로만 닿는다(시험은 가짜 `Host`).
//!
//! 프론트와는 이벤트 `board://account-login`(`AccountLoginEvent`)과 명령 `begin_add_account`·`submit_login_code`·`open_login_url`·
//! `delete_account`, 그리고 board.rs 의 `cancel_add_account` 로 말한다.
//!
//! 어느 프로그램을 띄울지는 `test_claude::login_program` 이 가른다 — 시험 모드면 가짜(`windows/ci/fake-claude-login.cjs`), 설정이 어긋나면 거절,
//! 꺼져 있을 때만 진짜 claude. 시험은 가짜만 쓴다.
//!
//! unchecked: 진짜 Windows `claude auth login` 의 ConPTY 출력 모양, 진짜 claude 가 `BROWSER` 를 따르는지(이 모듈은 설정하지 않는다 —
//! 진짜 claude 가 기본 브라우저를 함께 열 수 있다), 로그인 창 자체(GUI).

mod machine;
mod login_window;

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use claude_board_core::paths;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::accounts::{self, Account};
use crate::board::Board;
use crate::pty::Pty;
use crate::test_claude::{self, Decision};
use crate::util::board_log;
use machine::{Effect, Machine};

/// 프론트로 보내는 로그인 진행 이벤트.
pub const ACCOUNT_LOGIN_EVENT: &str = "board://account-login";

/// 로그인 주소가 한 줄로 나오게 넓은 가짜 콘솔로 띄운다(mac 은 1000열 × 50줄).
const COLUMNS: u16 = 1000;
const ROWS: u16 = 50;

/// 보드가 이 로그인을 아직 기다리는지 보는 간격
const WATCH_INTERVAL: Duration = Duration::from_millis(300);
/// 프로세스가 끝난 뒤 남은 출력을 기다리는 한도 — 콘솔을 닫아도 파이프가 안 끊기는 경우에도 종료 처리는 한다
const OUTPUT_DRAIN_LIMIT: Duration = Duration::from_secs(3);

/// 이벤트 내용 — 없는 칸은 보내지 않는다. `url` 은 열 로그인 주소, `note` 는 안내 한 줄, `done` 은 끝남, `error` 는 사유.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 프론트에 로그인 진행을 알린다.
pub fn emit(app: &AppHandle, event: &AccountLoginEvent) {
    if let Err(e) = app.emit(ACCOUNT_LOGIN_EVENT, event) {
        board_log(&format!("계정 로그인 이벤트를 보내지 못했습니다: {e}"));
    }
}

// ── 띄우기 ───────────────────────────────────────────────────

/// 로그인 명령 하나를 띄우는 방법.
#[derive(Debug, Clone)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// 이 프로세스에만 더하는 환경변수
    pub vars: Vec<(String, String)>,
}

/// 로그인 프로세스에 더할 환경 — `CLAUDE_CONFIG_DIR`(슬롯)는 늘, 시험 모드(가짜 claude)에서만 보드 환경의 `FAKE_LOGIN_*` 을 그대로 넘긴다
/// (탭 셸처럼 이 프로세스의 환경도 보드 환경을 물려받지 않으므로 가짜의 시험 손잡이는 이름으로 골라 넘겨야 닿는다).
pub fn launch_vars(slot: &Path, test_mode: bool, env: impl Iterator<Item = (OsString, OsString)>) -> Vec<(String, String)> {
    let mut vars = vec![("CLAUDE_CONFIG_DIR".to_string(), slot.to_string_lossy().into_owned())];
    if test_mode {
        for (name, value) in env {
            if let (Some(name), Some(value)) = (name.to_str(), value.to_str()) {
                if name.starts_with("FAKE_LOGIN_") {
                    vars.push((name.to_string(), value.to_string()));
                }
            }
        }
    }
    vars
}

// ── 보드와의 경계 ────────────────────────────────────────────

/// 로그인 세션이 바깥(보드·프론트)과 닿는 곳.
pub trait Host: Send + Sync + 'static {
    /// 프론트에 알린다
    fn emit(&self, event: AccountLoginEvent);
    /// 보드가 이 로그인을 아직 기다린다(저장·취소·시간 초과로 끝나면 false)
    fn pending(&self) -> bool;
    /// 계정 추가를 접고 이 안내를 오류로 띄운다
    fn fail(&self, message: &str);
}

/// 실행 중인 로그인 하나 — ConPTY 와 상태 기계.
pub struct Session {
    pty: Arc<Pty>,
    machine: Mutex<Machine>,
    host: Arc<dyn Host>,
    /// 알려 준 로그인 주소 — "다른 브라우저에서 열기"가 읽는다
    url: Mutex<Option<String>>,
    /// 프로세스가 끝나고 출력도 읽었다
    finished: AtomicBool,
}

impl Session {
    /// 사용자가 붙여 넣은 코드를 명령에 써 넣는다.
    pub fn submit_code(&self, code: &str) {
        let effects = self.machine.lock().unwrap().on_submit(code);
        self.apply(effects);
    }

    pub fn url(&self) -> Option<String> {
        self.url.lock().unwrap().clone()
    }

    /// 프로세스가 끝나고 종료 처리까지 마쳤다(시험이 기다린다)
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }

    /// 할 일을 한다. 상태 기계의 잠금은 이미 풀린 뒤라 `Host` 가 보드 잠금을 잡아도 얽히지 않는다.
    fn apply(&self, effects: Vec<Effect>) {
        for effect in effects {
            match effect {
                // 코드는 로그에 남기지 않는다
                Effect::Write(bytes) => {
                    if let Err(e) = self.pty.write_all(&bytes) {
                        board_log(&format!("로그인 명령에 쓰지 못했습니다: {e}"));
                    }
                }
                Effect::Emit(event) => {
                    if let Some(url) = &event.url {
                        *self.url.lock().unwrap() = Some(url.clone());
                    }
                    self.host.emit(event);
                }
                Effect::Fail(message) => self.host.fail(&message),
            }
        }
    }
}

/// 로그인 명령을 가짜 콘솔에 띄우고 지켜보는 스레드 둘을 시작한다.
/// 하나는 출력을 읽다가 프로세스가 끝나면 종료를 처리하고, 하나는 보드가 이 로그인을 접었는지 보다가 프로세스를 거두고 끝났다고 알린다.
pub fn start(host: Arc<dyn Host>, launch: &Launch, watch_interval: Duration) -> io::Result<Arc<Session>> {
    let pty = Pty::spawn_env(&launch.program, &launch.args, &launch.cwd, COLUMNS, ROWS, &launch.vars)?;
    let session = Arc::new(Session {
        pty: Arc::new(pty),
        machine: Mutex::new(Machine::default()),
        host,
        url: Mutex::new(None),
        finished: AtomicBool::new(false),
    });
    let supervised = Arc::clone(&session);
    std::thread::Builder::new().name("account-login".into()).spawn(move || supervise(&supervised))?;
    let watched = Arc::clone(&session);
    std::thread::Builder::new().name("account-login-watch".into()).spawn(move || watch(&watched, watch_interval))?;
    Ok(session)
}

/// 출력을 읽고, 프로세스가 끝나면 콘솔을 닫아 남은 출력을 다 읽은 뒤 종료를 상태 기계에 알린다.
fn supervise(session: &Arc<Session>) {
    let (drained_tx, drained_rx) = mpsc::channel();
    let reader = Arc::clone(session);
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 8192];
        while let Ok(n @ 1..) = reader.pty.read(&mut buf) {
            let effects = reader.machine.lock().unwrap().on_output(&buf[..n]);
            reader.apply(effects);
        }
        reader.pty.close_output();
        let _ = drained_tx.send(());
    });
    let code = session.pty.wait_exit();
    // 콘솔을 닫아야 출력 파이프가 끊긴다. 닫기가 오래 걸려도 읽기는 계속된다
    let closer = Arc::clone(&session.pty);
    std::thread::spawn(move || closer.close_console());
    session.pty.release();
    let _ = drained_rx.recv_timeout(OUTPUT_DRAIN_LIMIT);
    let pending = session.host.pending();
    let effects = session.machine.lock().unwrap().on_exit(code, pending);
    session.apply(effects);
    session.finished.store(true, Ordering::SeqCst);
}

/// 보드가 이 로그인을 접을 때까지 지켜보다가, 접으면 프로세스를 거두고 끝났다고 알린다.
fn watch(session: &Arc<Session>, interval: Duration) {
    while session.host.pending() {
        std::thread::sleep(interval);
    }
    session.pty.kill();
    let effects = session.machine.lock().unwrap().on_ended();
    session.apply(effects);
}

// ── 보드에 붙이기 ────────────────────────────────────────────

/// 지금 도는 로그인 — `submit_login_code`·`open_login_url` 이 찾는다.
static CURRENT: Mutex<Option<Arc<Session>>> = Mutex::new(None);

fn current() -> Option<Arc<Session>> {
    CURRENT.lock().unwrap().clone()
}

struct BoardHost {
    board: Arc<Board>,
    slot: PathBuf,
    /// 가짜 claude — 로그인 창을 열지 않는다(가짜 주소는 어디에도 닿지 않는다)
    test_mode: bool,
    window: login_window::LoginWindow,
}

impl BoardHost {
    fn user_closed(board: &Arc<Board>, slot: &Path) {
        let still_waiting = board.accounts.lock().unwrap().login_slot() == Some(slot);
        if !still_waiting {
            return;
        }
        accounts::cancel_add(board);
        board.accounts.lock().unwrap().flash("로그인 창을 닫아 계정 추가를 취소했습니다", false);
        board.publish();
    }
}

impl Host for BoardHost {
    fn emit(&self, event: AccountLoginEvent) {
        if let Some(url) = event.url.as_deref().filter(|_| !self.test_mode) {
            let (board, slot) = (self.board.clone(), self.slot.clone());
            self.window.open(url, move || Self::user_closed(&board, &slot));
        }
        if event.done == Some(true) {
            self.window.close();
        }
        emit(&self.board.app, &event);
    }

    fn pending(&self) -> bool {
        self.board.accounts.lock().unwrap().login_slot() == Some(self.slot.as_path())
    }

    fn fail(&self, message: &str) {
        accounts::cancel_add(&self.board);
        self.board.accounts.lock().unwrap().flash(message, true);
        self.board.publish();
    }
}

/// `accounts::begin_add` 가 부른다 — 슬롯에서 로그인 명령을 띄우고 현재 로그인으로 등록한다.
pub fn start_for_board(board: &Arc<Board>, program: &Path, args: Vec<String>, slot: &Path) -> io::Result<()> {
    let test_mode = matches!(test_claude::decision_from_process(), Decision::On(_));
    let launch = Launch { program: program.to_path_buf(), args, cwd: paths::home(), vars: launch_vars(slot, test_mode, std::env::vars_os()) };
    let host = Arc::new(BoardHost { board: board.clone(), slot: slot.to_path_buf(), test_mode, window: login_window::LoginWindow::new(board.app.clone()) });
    let session = start(host, &launch, WATCH_INTERVAL)?;
    *CURRENT.lock().unwrap() = Some(session);
    Ok(())
}

// ── 계정 지우기 ──────────────────────────────────────────────

/// 지우기 전에 물을 말.
#[derive(Debug, PartialEq)]
pub struct DeleteAsk {
    pub title: String,
    pub text: String,
}

/// 지울 수 있는 계정이면 확인 문구. 저장 목록에 없거나 지금 로그인 계정이면 None(지금 계정은 보드가 지우지 않는다).
pub fn delete_ask(current_id: Option<&str>, saved: &[Account], id: &str) -> Option<DeleteAsk> {
    if current_id == Some(id) {
        return None;
    }
    let account = saved.iter().find(|a| a.id == id)?;
    Some(DeleteAsk {
        title: format!("{} 계정을 지울까요?", account.email()),
        text: "보드에 저장한 이 계정의 로그인 정보를 지웁니다. 다시 쓰려면 계정을 다시 추가해야 합니다.".to_string(),
    })
}

/// 확인을 받은 뒤에만 지운다. 확인 창과 지우기는 바깥에서 받는다 — 시험은 가짜를 넣는다.
pub fn delete_after_confirm(ask: &DeleteAsk, confirm: impl FnOnce(&DeleteAsk) -> bool, remove: impl FnOnce()) -> bool {
    if !confirm(ask) {
        return false;
    }
    remove();
    true
}

// ── 명령 ─────────────────────────────────────────────────────

/// 프론트의 `beginAddAccount(email?)`.
#[tauri::command(async)]
pub fn begin_add_account(board: State<'_, Arc<Board>>, email: Option<String>) {
    accounts::begin_add(&board, email);
}

/// 프론트의 `submitLoginCode(code)` — 로그인 창에 붙여 넣은 코드를 로그인 ConPTY 에 넣는다.
#[tauri::command(async)]
pub fn submit_login_code(code: String) {
    match current() {
        Some(session) => session.submit_code(&code),
        None => board_log("받을 로그인이 없어 코드를 버렸습니다"),
    }
}

/// 프론트의 `openLoginUrl()` — 지금 로그인 주소를 기본 브라우저로 연다. 주소는 프론트가 아니라 로그인 명령의 출력에서 온 것만 연다.
#[tauri::command(async)]
pub fn open_login_url(app: AppHandle) {
    use tauri_plugin_opener::OpenerExt;
    let Some(url) = current().and_then(|s| s.url()).filter(|u| login_window::login_url(u).is_some()) else {
        board_log("열 로그인 주소가 없습니다");
        return;
    };
    if let Err(e) = app.opener().open_url(url, None::<&str>) {
        board_log(&format!("로그인 주소를 브라우저로 열지 못했습니다: {e}"));
    }
}

/// 프론트의 `deleteAccount(id)` — 지우기 전에 확인을 받는다(mac `confirmRemove`).
#[tauri::command(async)]
pub fn delete_account(board: State<'_, Arc<Board>>, id: String) {
    let ask = {
        let acc = board.accounts.lock().unwrap();
        delete_ask(acc.current.as_ref().map(|a| a.id.as_str()), &acc.saved, &id)
    };
    let Some(ask) = ask else {
        board_log("지울 수 없는 계정이라 무시했습니다");
        return;
    };
    delete_after_confirm(&ask, |ask| board.confirm(&ask.title, &ask.text, "지우기", "취소"), || accounts::remove_account(&board, id));
}

#[cfg(test)]
mod tests {
    use super::*;
    use claude_board_core::Record;
    use serde_json::json;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn event_sends_only_the_fields_that_are_set() {
        let url = AccountLoginEvent { url: Some("https://claude.ai/oauth/authorize?x=1".into()), ..Default::default() };
        assert_eq!(serde_json::to_value(&url).unwrap(), serde_json::json!({"url": "https://claude.ai/oauth/authorize?x=1"}));
        let done = AccountLoginEvent { done: Some(true), error: Some("취소".into()), ..Default::default() };
        assert_eq!(serde_json::to_value(&done).unwrap(), serde_json::json!({"done": true, "error": "취소"}));
        assert_eq!(serde_json::to_value(AccountLoginEvent::default()).unwrap(), serde_json::json!({}));
    }

    fn pairs(list: &[(&str, &str)]) -> impl Iterator<Item = (OsString, OsString)> {
        list.iter().map(|(k, v)| (OsString::from(k), OsString::from(v))).collect::<Vec<_>>().into_iter()
    }

    #[test]
    fn launch_vars_always_point_at_the_slot_and_forward_only_fake_login_knobs_in_test_mode() {
        let slot = Path::new("C:\\n\\accounts\\login-ab12cd34");
        let env = [("FAKE_LOGIN_LOG", "C:\\t\\log.jsonl"), ("FAKE_LOGIN_CODE", "c1"), ("PATH", "C:\\bin"), ("CLAUDE_CONFIG_DIR", "C:\\Users\\me\\.claude"), ("TOKEN", "secret")];
        let real = launch_vars(slot, false, pairs(&env));
        assert_eq!(real, [("CLAUDE_CONFIG_DIR".to_string(), slot.to_string_lossy().into_owned())], "진짜 모드는 보드 환경을 하나도 넘기지 않는다");
        let fake = launch_vars(slot, true, pairs(&env));
        assert_eq!(
            fake,
            [
                ("CLAUDE_CONFIG_DIR".to_string(), slot.to_string_lossy().into_owned()),
                ("FAKE_LOGIN_LOG".to_string(), "C:\\t\\log.jsonl".to_string()),
                ("FAKE_LOGIN_CODE".to_string(), "c1".to_string()),
            ],
            "보드 환경의 CLAUDE_CONFIG_DIR 는 슬롯을 덮지 못한다"
        );
    }

    fn saved_account(uuid: &str, email: &str) -> Account {
        let oauth: Record = json!({"accountUuid": uuid, "organizationUuid": "org", "emailAddress": email}).as_object().unwrap().clone();
        Account::new(&oauth, None).unwrap()
    }

    #[test]
    fn delete_asks_with_the_email_and_refuses_unknown_or_current_accounts() {
        let saved = [saved_account("a", "a@x.com"), saved_account("b", "b@x.com")];
        let ask = delete_ask(Some("a|org"), &saved, "b|org").unwrap();
        assert_eq!(ask.title, "b@x.com 계정을 지울까요?");
        assert!(ask.text.contains("다시 쓰려면 계정을 다시 추가"));
        assert_eq!(delete_ask(Some("a|org"), &saved, "a|org"), None, "지금 계정은 지우지 않는다");
        assert_eq!(delete_ask(Some("a|org"), &saved, "ghost|org"), None);
        assert!(delete_ask(None, &saved, "a|org").is_some());
    }

    #[test]
    fn delete_removes_only_after_a_yes() {
        let ask = DeleteAsk { title: "t".into(), text: "x".into() };
        let removed = AtomicUsize::new(0);
        assert!(!delete_after_confirm(&ask, |_| false, || {
            removed.fetch_add(1, Ordering::SeqCst);
        }));
        assert_eq!(removed.load(Ordering::SeqCst), 0, "취소하면 지우지 않는다");
        assert!(delete_after_confirm(&ask, |a| a.title == "t", || {
            removed.fetch_add(1, Ordering::SeqCst);
        }));
        assert_eq!(removed.load(Ordering::SeqCst), 1);
    }

    // ── 가짜 로그인 스크립트로 ConPTY 끝까지(진짜 claude 는 어디에도 없다). ConPTY 는 Windows 에만 있다 ──

    #[cfg(windows)]
    mod through_conpty {
    use super::*;
    use crate::accounts::{AccountPaths, LoginPoll};
    use crate::util::short_id;
    use std::time::Instant;

    #[derive(Default)]
    struct TestHost {
        events: Mutex<Vec<AccountLoginEvent>>,
        failures: Mutex<Vec<String>>,
        stopped: AtomicBool,
    }

    impl TestHost {
        fn waiting() -> Arc<Self> {
            Arc::new(TestHost::default())
        }

        /// 보드가 로그인을 접었다(취소·저장·시간 초과)
        fn stop_waiting(&self) {
            self.stopped.store(true, Ordering::SeqCst);
        }

        fn events(&self) -> Vec<AccountLoginEvent> {
            self.events.lock().unwrap().clone()
        }

        fn has(&self, wanted: impl Fn(&AccountLoginEvent) -> bool) -> bool {
            self.events().iter().any(wanted)
        }
    }

    impl Host for TestHost {
        fn emit(&self, event: AccountLoginEvent) {
            self.events.lock().unwrap().push(event);
        }

        fn pending(&self) -> bool {
            !self.stopped.load(Ordering::SeqCst)
        }

        fn fail(&self, message: &str) {
            self.failures.lock().unwrap().push(message.to_string());
            self.stop_waiting();
        }
    }

    fn wait_until(what: &str, limit: Duration, condition: impl Fn() -> bool) {
        let started = Instant::now();
        while !condition() {
            assert!(started.elapsed() < limit, "기다리다 시간이 지났다: {what}");
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn find_on_path(name: &str) -> PathBuf {
        std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|d| d.join(name))
            .find(|p| p.is_file())
            .unwrap_or_else(|| panic!("{name} 이 PATH 에 없다 — 가짜 로그인 시험에는 node 가 필요하다"))
    }

    /// 가짜 로그인 스크립트 — 저장소의 windows/ci
    fn fake_script() -> PathBuf {
        // canonicalize 는 `\\?\` 접두 경로를 만들어 node 가 못 읽을 수 있어 `..` 를 그대로 둔다
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("ci").join("fake-claude-login.cjs");
        assert!(script.is_file(), "가짜 로그인 스크립트가 없다: {}", script.display());
        script
    }

    struct Scratch {
        root: PathBuf,
        slot: PathBuf,
        log: PathBuf,
    }

    impl Scratch {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("cb-account-login-{}", short_id()));
            let slot = root.join("accounts").join("login-test0001");
            std::fs::create_dir_all(&slot).unwrap();
            Scratch { log: root.join("fake-login.jsonl"), root, slot }
        }

        /// 시험 모드 결정(`Decision::On`)으로 고른 가짜를 slot 에 띄운다 — 진짜 claude 를 찾는 길은 아예 넘기지 않는다
        fn launch(&self, extra_vars: &[(&str, &str)]) -> Launch {
            let decided = Decision::On(vec![find_on_path("node.exe"), fake_script()]);
            let never_real = || -> Option<PathBuf> { panic!("시험이 진짜 claude 를 찾았다") };
            let (program, mut args) = test_claude::login_program(decided, never_real).unwrap();
            args.extend(["auth".to_string(), "login".into(), "--claudeai".into()]);
            let mut vars = launch_vars(&self.slot, false, std::iter::empty());
            vars.push(("FAKE_LOGIN_LOG".into(), self.log.to_string_lossy().into_owned()));
            vars.extend(extra_vars.iter().map(|(k, v)| (k.to_string(), v.to_string())));
            Launch { program, args, cwd: std::env::temp_dir(), vars }
        }

        fn log_events(&self) -> Vec<serde_json::Value> {
            std::fs::read_to_string(&self.log).unwrap_or_default().lines().filter_map(|l| serde_json::from_str(l).ok()).collect()
        }

        fn account_paths(&self) -> AccountPaths {
            let claude_dir = self.root.join("claude-home");
            std::fs::create_dir_all(&claude_dir).unwrap();
            AccountPaths {
                credentials: claude_dir.join(".credentials.json"),
                claude_json: self.root.join(".claude.json"),
                meta: self.root.join("notifier").join("accounts.json"),
                root: self.root.join("accounts"),
                claude_dir,
            }
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    const LIMIT: Duration = Duration::from_secs(20);

    #[test]
    fn right_code_runs_the_whole_flow_through_a_real_conpty() {
        let scratch = Scratch::new();
        let host = TestHost::waiting();
        let session = start(host.clone(), &scratch.launch(&[]), Duration::from_millis(50)).unwrap();

        // 주소 — 한 번만, 가짜의 예약 도메인
        wait_until("로그인 주소", LIMIT, || host.has(|e| e.url.is_some()));
        let url = session.url().expect("알려 준 주소");
        assert!(url.starts_with("https://login.fake.test/oauth/authorize?"), "{url}");
        assert!(login_window::login_url(&url).is_some());

        // 코드를 쓰면 가짜가 슬롯에 계정을 쓰고 0 으로 끝난다
        session.submit_code("  fake-code-1234 ");
        wait_until("명령 종료", LIMIT, || session.finished());
        let events = host.events();
        assert_eq!(events.iter().filter(|e| e.url.is_some()).count(), 1);
        assert!(events.iter().any(|e| e.note.as_deref() == Some("코드를 보냈습니다. 확인하는 중…")));
        assert!(events.iter().any(|e| e.note.as_deref() == Some("로그인을 마쳤습니다. 계정 정보를 확인하는 중…")), "{events:?}");
        assert!(host.failures.lock().unwrap().is_empty());

        // 보드의 슬롯 지켜보기(poll_login)가 그 슬롯에서 계정을 찾는다 — 사용자 폴더가 아니라 임시 폴더만 쓴다
        let paths = scratch.account_paths();
        let LoginPoll::Done(Ok(account)) = accounts::poll_login(&paths, &scratch.slot) else { panic!("슬롯에서 계정을 찾지 못했다") };
        assert_eq!(account.email(), "fake-login@example.test");

        // 보드가 로그인을 끝내면 끝났다고 한 번 알린다
        host.stop_waiting();
        wait_until("끝남 알림", LIMIT, || host.has(|e| e.done == Some(true)));
        assert_eq!(host.events().iter().filter(|e| e.done == Some(true)).count(), 1);
        assert!(host.events().iter().all(|e| e.error.is_none()));

        // 가짜가 받은 것 — 인자와 슬롯, 코드는 앞뒤 공백 없이 한 번
        let log = scratch.log_events();
        let start = log.iter().find(|e| e["event"] == "start").expect("start 기록");
        assert_eq!(start["argv"], json!(["auth", "login", "--claudeai"]));
        assert_eq!(start["slot"], json!(scratch.slot.to_string_lossy()));
        assert_eq!(log.iter().filter(|e| e["event"] == "code").count(), 1);
        assert_eq!(log.iter().find(|e| e["event"] == "code").unwrap()["code"], "fake-code-1234");
    }

    #[test]
    fn wrong_code_fails_with_the_reason_and_leaves_the_slot_empty() {
        let scratch = Scratch::new();
        let host = TestHost::waiting();
        let session = start(host.clone(), &scratch.launch(&[]), Duration::from_millis(50)).unwrap();
        wait_until("로그인 주소", LIMIT, || host.has(|e| e.url.is_some()));
        session.submit_code("wrong-code");
        wait_until("끝남 알림", LIMIT, || host.has(|e| e.done == Some(true)));
        let message = "계정을 추가하지 못했습니다: Login failed: Invalid authorization code";
        assert_eq!(*host.failures.lock().unwrap(), [message]);
        let done = host.events().into_iter().find(|e| e.done == Some(true)).unwrap();
        assert_eq!(done.error.as_deref(), Some(message));
        assert!(!scratch.slot.join(".credentials.json").exists());
        assert!(matches!(accounts::poll_login(&scratch.account_paths(), &scratch.slot), LoginPoll::Waiting));
    }

    #[test]
    fn the_board_giving_up_kills_the_command_and_reports_done_without_an_error() {
        let scratch = Scratch::new();
        let host = TestHost::waiting();
        let session = start(host.clone(), &scratch.launch(&[]), Duration::from_millis(50)).unwrap();
        wait_until("로그인 주소", LIMIT, || host.has(|e| e.url.is_some()));
        // 취소·15분 한도·로그인 창 닫기 — 보드가 접는다
        host.stop_waiting();
        wait_until("끝남 알림", LIMIT, || host.has(|e| e.done == Some(true)));
        wait_until("명령 종료", LIMIT, || session.finished());
        assert!(host.failures.lock().unwrap().is_empty(), "접은 로그인의 종료는 실패로 치지 않는다");
        assert!(host.events().iter().all(|e| e.error.is_none()));
        assert!(scratch.log_events().iter().all(|e| e["event"] != "code"));
        // 접은 뒤 온 코드는 아무 데도 쓰이지 않는다
        session.submit_code("fake-code-1234");
        std::thread::sleep(Duration::from_millis(200));
        assert!(scratch.log_events().iter().all(|e| e["event"] != "code"));
    }

    #[test]
    fn a_command_that_finishes_by_itself_waits_for_the_board_to_pick_up_the_slot() {
        // 브라우저 콜백으로 저절로 끝나는 길 — 코드를 쓰지 않는다
        let scratch = Scratch::new();
        let host = TestHost::waiting();
        let session = start(host.clone(), &scratch.launch(&[("FAKE_LOGIN_AUTO_MS", "300")]), Duration::from_millis(50)).unwrap();
        wait_until("명령 종료", LIMIT, || session.finished());
        assert!(host.has(|e| e.note.as_deref() == Some("로그인을 마쳤습니다. 계정 정보를 확인하는 중…")));
        assert!(!host.has(|e| e.done == Some(true)), "보드가 접기 전에는 끝났다고 알리지 않는다");
        assert!(matches!(accounts::poll_login(&scratch.account_paths(), &scratch.slot), LoginPoll::Done(Ok(_))));
        host.stop_waiting();
        wait_until("끝남 알림", LIMIT, || host.has(|e| e.done == Some(true)));
    }

    #[test]
    fn spawn_failure_is_an_error_not_a_hang() {
        let host = TestHost::waiting();
        let launch = Launch { program: PathBuf::from("C:\\no-such-dir\\no-such.exe"), args: vec![], cwd: std::env::temp_dir(), vars: vec![] };
        assert!(start(host, &launch, Duration::from_millis(50)).is_err());
    }
    }
}
