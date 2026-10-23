//! `drive` 의 시험 — 가짜 세션과 가짜 `Io` 로 설계 §7 의 표를 하나씩 본다.

use std::cell::RefCell;
use std::path::PathBuf;

use super::*;
use crate::model::PidPlace;

fn session(pid: u32, state: &str, tty: Option<&str>, mode: Option<&'static str>, headless: bool, tab: Option<&str>) -> Session {
    Session {
        id: "sid".into(),
        cwd: String::new(),
        state: state.into(),
        since: 0,
        updated: 0,
        headless,
        title: String::new(),
        topic: String::new(),
        message: String::new(),
        name: None,
        ctx: None,
        brain: None,
        subagents: Vec::new(),
        shells: 0,
        paused: false,
        muted: false,
        limit_at: None,
        can_drive: false,
        place: PidPlace { tty: tty.map(str::to_string), tab_ancestor: tab.map(str::to_string), mode, app_name: None },
        pid,
        pid_ctime: None,
        file: PathBuf::new(),
        project_dir: String::new(),
        resume_args: Vec::new(),
        since_s: 0.0,
        updated_s: 0.0,
        limit_s: None,
    }
}

fn runs(ids: &[&str]) -> Vec<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

const BOTH: [Purpose; 2] = [Purpose::Keys, Purpose::Stop];

#[test]
fn ended_or_pidless_session_has_no_target() {
    for p in BOTH {
        assert_eq!(drive_target(&session(5, "ended", Some("t1"), None, false, None), &runs(&["t1"]), true, p), Target::None);
        assert_eq!(drive_target(&session(0, "working", Some("t1"), None, false, None), &runs(&["t1"]), true, p), Target::None);
    }
}

#[test]
fn interactive_tab_session_writes_to_its_surface() {
    let s = session(5, "working", Some("t1"), None, false, None);
    for p in BOTH {
        assert_eq!(drive_target(&s, &runs(&["t1"]), true, p), Target::Surface("t1".into()));
    }
}

#[test]
fn tab_session_not_in_foreground_is_only_killable() {
    let s = session(5, "working", Some("t1"), None, false, None);
    assert_eq!(drive_target(&s, &runs(&["t2"]), true, Purpose::Keys), Target::None);
    assert_eq!(drive_target(&s, &runs(&["t2"]), true, Purpose::Stop), Target::Kill(5));
}

#[test]
fn print_mode_in_a_tab_is_only_killable() {
    let s = session(5, "working", Some("t1"), Some("-p"), false, None);
    assert_eq!(drive_target(&s, &runs(&["t1"]), true, Purpose::Keys), Target::None);
    assert_eq!(drive_target(&s, &runs(&["t1"]), true, Purpose::Stop), Target::Kill(5));
}

#[test]
fn tab_owned_headless_is_only_killable_and_background_is_untouched() {
    let owned = session(5, "working", None, Some("-p"), true, Some("t1"));
    assert_eq!(drive_target(&owned, &[], true, Purpose::Keys), Target::None);
    assert_eq!(drive_target(&owned, &[], true, Purpose::Stop), Target::Kill(5));
    let background = session(5, "working", None, Some("--bg"), true, None);
    for p in BOTH {
        assert_eq!(drive_target(&background, &[], true, p), Target::None);
    }
}

#[test]
fn foreign_console_session_follows_attachability() {
    let s = session(5, "working", None, None, false, None);
    for p in BOTH {
        assert_eq!(drive_target(&s, &[], true, p), Target::Console(5));
    }
    assert_eq!(drive_target(&s, &[], false, Purpose::Keys), Target::None);
    assert_eq!(drive_target(&s, &[], false, Purpose::Stop), Target::Kill(5));
}

#[test]
fn encode_maps_keys_and_text() {
    assert_eq!(encode(&Step::Escape), Ok(vec![0x1b]));
    assert_eq!(encode(&Step::Enter), Ok(vec![0x0d]));
    assert_eq!(encode(&Step::Text("/exit".into())), Ok(b"/exit".to_vec()));
    assert_eq!(encode(&Step::Text("한글 ok".into())), Ok("한글 ok".as_bytes().to_vec()));
    assert_eq!(encode(&Step::Wait(10)), Ok(Vec::new()));
}

#[test]
fn encode_rejects_control_characters_in_text() {
    for bad in ["a\nb", "a\rb", "a\x1bb", "a\x03b", "\t", "\x7f"] {
        assert!(encode(&Step::Text(bad.into())).is_err(), "{bad:?}");
    }
}

#[test]
fn steps_for_matches_the_mac_lists() {
    assert_eq!(steps_for(SessionAction::Pause), vec![Step::Escape]);
    assert_eq!(steps_for(SessionAction::Resume), vec![Step::Text(CARRY_ON_PROMPT.into()), Step::Wait(200), Step::Enter]);
    assert_eq!(
        steps_for(SessionAction::Stop),
        vec![Step::Escape, Step::Wait(400), Step::Text("/exit".into()), Step::Wait(200), Step::Enter]
    );
    assert!(steps_for(SessionAction::Focus).is_empty());
}

#[test]
fn carry_on_prompt_is_safe_to_type() {
    assert!(encode(&Step::Text(CARRY_ON_PROMPT.into())).is_ok());
}

/// 부른 것을 적어 두는 가짜 — `ok` 가 false 면 쓰기·입력·끝내기가 모두 실패한다.
struct Fake {
    ok: bool,
    log: RefCell<Vec<String>>,
}

impl Fake {
    fn new(ok: bool) -> Self {
        Fake { ok, log: RefCell::new(Vec::new()) }
    }
    fn calls(&self) -> Vec<String> {
        self.log.borrow().clone()
    }
}

impl Io for Fake {
    fn surface(&self, id: &str, bytes: &[u8]) -> bool {
        self.log.borrow_mut().push(format!("surface {id} {bytes:02x?}"));
        self.ok
    }
    fn console(&self, pid: u32, inputs: &[Input]) -> bool {
        self.log.borrow_mut().push(format!("console {pid} {}", inputs.len()));
        self.ok
    }
    fn kill(&self, pid: u32) -> bool {
        self.log.borrow_mut().push(format!("kill {pid}"));
        self.ok
    }
    fn sleep(&self, ms: u64) {
        self.log.borrow_mut().push(format!("sleep {ms}"));
    }
}

#[test]
fn pause_reports_success_only_when_the_writer_succeeds() {
    let target = Target::Surface("t1".into());
    let ok = Fake::new(true);
    assert!(run(&target, &steps_for(SessionAction::Pause), Purpose::Keys, &ok));
    assert_eq!(ok.calls(), vec!["surface t1 [1b]"]);
    assert!(!run(&target, &steps_for(SessionAction::Pause), Purpose::Keys, &Fake::new(false)));
}

#[test]
fn stop_on_a_surface_sends_the_exit_sequence_with_waits() {
    let io = Fake::new(true);
    assert!(run(&Target::Surface("t1".into()), &steps_for(SessionAction::Stop), Purpose::Stop, &io));
    assert_eq!(
        io.calls(),
        vec!["surface t1 [1b]", "sleep 400", "surface t1 [2f, 65, 78, 69, 74]", "sleep 200", "surface t1 [0d]"]
    );
}

#[test]
fn stop_on_a_console_kills_when_input_fails() {
    let io = Fake::new(false);
    assert!(!run(&Target::Console(7), &steps_for(SessionAction::Stop), Purpose::Stop, &io));
    assert_eq!(io.calls(), vec!["console 7 5", "kill 7"]);
    let keys = Fake::new(false);
    assert!(!run(&Target::Console(7), &steps_for(SessionAction::Pause), Purpose::Keys, &keys));
    assert_eq!(keys.calls(), vec!["console 7 1"]);
}

#[test]
fn no_target_never_touches_the_outside() {
    let io = Fake::new(true);
    assert!(!run(&Target::None, &steps_for(SessionAction::Stop), Purpose::Stop, &io));
    assert!(io.calls().is_empty());
}

#[test]
fn control_character_text_sends_nothing() {
    let io = Fake::new(true);
    let steps = [Step::Escape, Step::Text("a\nb".into())];
    assert!(!run(&Target::Surface("t1".into()), &steps, Purpose::Keys, &io));
    assert!(io.calls().is_empty());
}

#[test]
fn kill_target_terminates() {
    let io = Fake::new(true);
    assert!(run(&Target::Kill(9), &[], Purpose::Stop, &io));
    assert_eq!(io.calls(), vec!["kill 9"]);
}

#[test]
fn rename_is_safe_only_when_working_or_idle_and_not_waiting() {
    assert!(rename_safe("working", Some("busy"), None));
    assert!(rename_safe("idle", None, Some("idle")));
    assert!(!rename_safe("waiting", Some("idle"), None));
    assert!(!rename_safe("working", Some("waiting"), Some("working")));
    assert!(!rename_safe("working", None, Some("waiting")));
    assert!(!rename_safe("ended", None, None));
}

#[test]
fn step_spec_parses_the_frontend_shapes() {
    let specs: Vec<StepSpec> = serde_json::from_str(r#"[{"key":"escape"},{"key":"enter"},{"text":"/rename a"},{"wait":150}]"#).unwrap();
    let steps: Vec<Step> = specs.into_iter().map(|s| s.into_step().unwrap()).collect();
    assert_eq!(steps, vec![Step::Escape, Step::Enter, Step::Text("/rename a".into()), Step::Wait(150)]);
    assert!(StepSpec::Wait { wait: 60_000 }.into_step().is_err());
}

#[test]
fn stale_snapshot_never_drives_or_kills() {
    let old = session(5, "working", Some("t1"), None, false, None);
    let mut same = old.clone();
    same.state = "idle".into();
    assert!(still_same_session(&old, Some(same)).is_some());
    let mut ended = old.clone();
    ended.state = "ended".into();
    assert!(still_same_session(&old, Some(ended)).is_none());
    assert!(still_same_session(&old, None).is_none());
    let mut reused = old.clone();
    reused.pid = 6;
    assert!(still_same_session(&old, Some(reused)).is_none());
    let mut other_start = old.clone();
    other_start.pid_ctime = Some(1.0);
    assert!(still_same_session(&old, Some(other_start)).is_none());
}

#[test]
fn stop_falls_back_to_kill_when_the_surface_write_fails() {
    let io = Fake::new(false);
    let target = Target::Surface("t1".into());
    assert!(!run_with_fallback(&target, 5, &steps_for(SessionAction::Stop), Purpose::Stop, &io));
    assert_eq!(io.calls().last().map(String::as_str), Some("kill 5"));
    // 중단이 아니면 끝내지 않는다
    let keys = Fake::new(false);
    assert!(!run_with_fallback(&target, 5, &steps_for(SessionAction::Pause), Purpose::Keys, &keys));
    assert!(!keys.calls().iter().any(|c| c.starts_with("kill")));
    // 쓰기가 되면 끝내지 않는다
    let ok = Fake::new(true);
    assert!(run_with_fallback(&target, 5, &steps_for(SessionAction::Stop), Purpose::Stop, &ok));
    assert!(!ok.calls().iter().any(|c| c.starts_with("kill")));
}

#[test]
fn total_wait_is_capped() {
    assert!(total_wait_ok(&[Step::Wait(5000), Step::Wait(5000)]));
    assert!(!total_wait_ok(&[Step::Wait(5000), Step::Wait(5000), Step::Wait(1)]));
    assert!(total_wait_ok(&steps_for(SessionAction::Stop)));
}
