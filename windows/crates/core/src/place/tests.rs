//! `place` 의 시험 — 가짜 `ProcessView`(해시맵)로 설계 §4 의 자리를 하나씩 본다.

use std::collections::HashMap;

use super::*;

/// 보고 싶은 프로세스 사실 하나.
#[derive(Debug, Clone)]
struct Node {
    ppid: u32,
    ctime: Option<f64>,
    host: Host,
    mode: Option<Mode>,
}

/// 가짜 조사 — 없는 pid 는 아무것도 모르는 프로세스다(부모 None·시각 None·Unknown).
struct FakeView {
    nodes: HashMap<u32, Node>,
}

impl FakeView {
    fn new() -> Self {
        Self { nodes: HashMap::new() }
    }

    fn add(&mut self, pid: u32, ppid: u32, ctime: f64, host: Host, mode: Option<Mode>) {
        self.nodes.insert(pid, Node { ppid, ctime: Some(ctime), host, mode });
    }

    fn add_without_ctime(&mut self, pid: u32, ppid: u32, host: Host, mode: Option<Mode>) {
        self.nodes.insert(pid, Node { ppid, ctime: None, host, mode });
    }
}

impl ProcessView for FakeView {
    fn parent(&self, pid: u32) -> Option<u32> {
        self.nodes.get(&pid).map(|n| n.ppid)
    }

    fn ctime(&self, pid: u32) -> Option<f64> {
        self.nodes.get(&pid).and_then(|n| n.ctime)
    }

    fn host(&self, pid: u32) -> Host {
        self.nodes.get(&pid).map(|n| n.host).unwrap_or(Host::Unknown)
    }

    fn mode(&self, pid: u32) -> Option<Mode> {
        self.nodes.get(&pid).and_then(|n| n.mode)
    }
}

fn term(id: &str, shell_pid: u32, shell_ctime: f64, host: Host) -> BoardTerm {
    BoardTerm { id: id.to_string(), shell_pid, shell_ctime: Some(shell_ctime), host }
}

fn placed(tty: Option<&str>, tab_ancestor: Option<&str>, mode: Option<Mode>) -> Placement {
    Placement { tty: tty.map(str::to_string), tab_ancestor: tab_ancestor.map(str::to_string), mode }
}

/// 기본 무대 — 보드 탭(셸 10), 그 안의 claude(25). 세션 노드는 각 시험이 둔다.
fn base() -> (FakeView, Vec<BoardTerm>) {
    let mut view = FakeView::new();
    view.add(1, 0, 9.0, Host::Detached, None);          // System
    view.add(10, 1, 100.0, Host::Console(700), None);   // 보드 탭의 셸
    view.add(25, 10, 115.0, Host::Console(700), None);  // 탭에서 도는 claude
    let terms = vec![term("t1", 10, 100.0, Host::Console(700))];
    (view, terms)
}

fn session(pid: u32, headless: bool) -> SessionProc {
    SessionProc { pid, headless, ended: false }
}

#[test]
fn same_host_pairs_as_tty() {
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Console(700), None);
    assert_eq!(place(&session(40, false), &terms, &view), placed(Some("t1"), None, None));
}

#[test]
fn different_host_pair_nothing() {
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Console(900), None);
    assert_eq!(place(&session(40, false), &terms, &view), placed(None, None, None));
}

#[test]
fn unknown_host_falls_back_to_ancestor_shell_tty() {
    // 세션과 터미널이 모두 콘솔을 모른다 — 걸어 올려 닿는 셸로 짝짓는다
    let (mut view, mut terms) = base();
    terms[0].host = Host::Unknown;
    view.nodes.get_mut(&10).unwrap().host = Host::Unknown;
    view.add(40, 25, 130.0, Host::Unknown, None);
    assert_eq!(place(&session(40, false), &terms, &view), placed(Some("t1"), None, None));

    // 세션만 콘솔을 모른다
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Unknown, None);
    assert_eq!(place(&session(40, false), &terms, &view), placed(Some("t1"), None, None));

    // 터미널만 콘솔을 모른다(세션 콘솔은 어느 탭과도 다른 900)
    let (mut view, mut terms) = base();
    view.nodes.get_mut(&10).unwrap().host = Host::Unknown;
    terms[0].host = Host::Unknown;
    view.add(40, 25, 130.0, Host::Console(900), None);
    assert_eq!(place(&session(40, false), &terms, &view), placed(Some("t1"), None, None));
}

#[test]
fn unknown_host_headless_gets_tab_ancestor_not_tty() {
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Unknown, Some(Mode::Print));
    assert_eq!(
        place(&session(40, true), &terms, &view),
        placed(None, Some("t1"), Some(Mode::Print))
    );
}

#[test]
fn headless_hidden_console_child_pairs_tab_ancestor() {
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Console(2000), None);
    assert_eq!(place(&session(40, true), &terms, &view), placed(None, Some("t1"), None));
}

#[test]
fn worker_under_detached_supervisor_pairs_nothing() {
    let (mut view, terms) = base();
    // 감시자 꾸러미의 일꾼 같은 중간 프로세스(30) — 콘솔이 없어 걸음의 경계다
    view.add(30, 25, 120.0, Host::Detached, Some(Mode::Background));
    view.add(40, 30, 130.0, Host::Console(900), None);
    assert_eq!(place(&session(40, true), &terms, &view), placed(None, None, None));
}

#[test]
fn intermediate_bg_stops_walk() {
    let (mut view, terms) = base();
    // 중간 걸음(30)이 콘솔은 탭과 같지만 `--bg` 자라 걸음을 막는다
    view.add(30, 25, 120.0, Host::Console(700), Some(Mode::Background));
    view.add(40, 30, 130.0, Host::Detached, None);
    assert_eq!(place(&session(40, true), &terms, &view), placed(None, None, None));
}

#[test]
fn session_itself_bg_pairs_nothing() {
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Unknown, Some(Mode::Background));
    assert_eq!(
        place(&session(40, true), &terms, &view),
        placed(None, None, Some(Mode::Background))
    );
}

#[test]
fn stale_ppid_stops_walk() {
    let (mut view, terms) = base();
    // 죽은 세션 자리를 다시 쓴 프로세스(50) — 부모가 자식보다 늦게 태어났다
    view.add(50, 10, 140.0, Host::Console(900), None);
    view.add(40, 50, 130.0, Host::Console(900), None);
    assert_eq!(place(&session(40, true), &terms, &view), placed(None, None, None));
}

#[test]
fn unknown_ctime_stops_walk() {
    // 걸음이 바로 위 부모의 생성 시각을 모르면 멈춘다
    let (mut view, terms) = base();
    view.add_without_ctime(25, 10, Host::Console(700), None);
    view.add(40, 25, 130.0, Host::Console(900), None);
    assert_eq!(place(&session(40, true), &terms, &view), placed(None, None, None));

    // 세션 자신의 생성 시각을 모르면 아예 걷지 않는다
    let (mut view, terms) = base();
    view.add_without_ctime(40, 25, Host::Console(900), None);
    assert_eq!(place(&session(40, true), &terms, &view), placed(None, None, None));
}

#[test]
fn ended_session_pairs_nothing() {
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Console(700), None);
    let ended = SessionProc { pid: 40, headless: false, ended: true };
    assert_eq!(place(&ended, &terms, &view), placed(None, None, None));

    // pid 를 알지 못하는 세션도 같다
    assert_eq!(place(&session(0, false), &terms, &view), placed(None, None, None));
}

#[test]
fn walk_stops_after_twelve_hops() {
    let mut view = FakeView::new();
    // 10 — 25 — 40 쇠사슬: 걸음 수로 닿고 못 닿고가 갈린다
    for pid in 11..=25 {
        view.add(pid, pid - 1, 100.0 + (pid - 10) as f64, Host::Console(700), None);
    }
    view.add(10, 1, 100.0, Host::Console(700), None);
    let terms = vec![term("t1", 10, 100.0, Host::Console(700))];
    // 세션 40 — 셸 10 은 12 걸음째라 닿는다
    view.add(40, 21, 120.0, Host::Console(900), None);
    assert_eq!(place(&session(40, true), &terms, &view), placed(None, Some("t1"), None));
    // 세션 41 — 셸 10 은 13 걸음째라 12 걸음 상한에 걸린다
    view.add(41, 22, 120.0, Host::Console(900), None);
    assert_eq!(place(&session(41, true), &terms, &view), placed(None, None, None));
}

fn args(fields: &[&str]) -> Vec<String> {
    fields.iter().map(|field| field.to_string()).collect()
}

#[test]
fn mode_table() {
    assert_eq!(mode(&args(&[])), None);
    assert_eq!(mode(&args(&["claude"])), None);
    assert_eq!(mode(&args(&["-p"])), None); // argv[0] 은 넘긴다
    assert_eq!(mode(&args(&["claude", "-p"])), Some(Mode::Print));
    assert_eq!(mode(&args(&["claude", "--print", "-p"])), Some(Mode::Print));
    assert_eq!(mode(&args(&["claude", "--bg"])), Some(Mode::Background));
    assert_eq!(mode(&args(&["claude", "--background"])), Some(Mode::Background));
    assert_eq!(mode(&args(&["claude", "--bg-pty-host"])), Some(Mode::Background));
    assert_eq!(mode(&args(&["claude", "--bgx"])), None); // 정확히 같은 인자만 본다
    // 감시자 방식이 프린트 방식보다 우선한다(어느 자리에 있든)
    assert_eq!(mode(&args(&["claude", "-p", "--bg"])), Some(Mode::Background));
    assert_eq!(mode(&args(&["claude", "--bg", "-p"])), Some(Mode::Background));
    assert_eq!(mode(&args(&["claude", "-x"])), None);
}

#[test]
fn app_name_table() {
    assert_eq!(app_name("WindowsTerminal.EXE"), "Windows Terminal");
    assert_eq!(app_name("Code.EXE"), "VS Code");
    assert_eq!(app_name("mintty.exe"), "Git Bash");
    assert_eq!(app_name("cmd.exe"), "터미널");
    assert_eq!(app_name("C:\\tools\\Code.exe"), "VS Code");
}

#[test]
fn runs_claude_true_and_false() {
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Console(9000), None);
    let t2 = term("t2", 10, 100.0, Host::Console(800));
    // claude(25) 의 콘솔 호스트가 t1 과 같다
    assert!(runs_claude(&terms[0], &[40, 25], &view));
    assert!(!runs_claude(&terms[0], &[], &view));
    assert!(!runs_claude(&t2, &[40, 25], &view));
    // 콘솔을 못 읽은(Unknown) 쪽끼리는 같은 터미널로 보지 않는다 — 오탐 방지
    let mut unknown = base();
    unknown.0.add(26, 25, 116.0, Host::Unknown, None);
    let t3 = term("t3", 10, 100.0, Host::Unknown);
    assert!(!runs_claude(&t3, &[26], &unknown.0));
    // Detached 끼리도 마찬가지다
    let mut detached = base();
    detached.0.add(27, 25, 117.0, Host::Detached, None);
    let t4 = term("t4", 10, 100.0, Host::Detached);
    assert!(!runs_claude(&t4, &[27], &detached.0));
}

#[test]
fn term_shell_ctime_mismatch_blocks_fallback() {
    let (mut view, terms) = base();
    view.add(40, 25, 130.0, Host::Unknown, None);
    // 기록한 셸 생성 시각(100)이 실제와 같으면 짝짓는다
    assert_eq!(place(&session(40, false), &terms, &view), placed(Some("t1"), None, None));
    // 기록(50)이 실제(100)와 다르면(pid 재사용) 짝짓지 않는다
    let stale = vec![term("ta", 10, 50.0, Host::Console(700))];
    assert_eq!(place(&session(40, false), &stale, &view), placed(None, None, None));
    // 시각을 몰라 기록하지 못한 터미널도 짝짓지 않는다
    let (mut view, _) = base();
    view.add(40, 25, 130.0, Host::Unknown, None);
    let no_ctime = BoardTerm {
        id: "t9".to_string(),
        shell_pid: 10,
        shell_ctime: None,
        host: Host::Console(700),
    };
    assert_eq!(place(&session(40, false), &[no_ctime], &view), placed(None, None, None));
}