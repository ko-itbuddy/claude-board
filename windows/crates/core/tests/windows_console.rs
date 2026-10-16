//! Windows 실기 시험 — 콘솔 붙기·제목·입력 넣기 (`console`, SPEC §3.2 제목·§3.4 headless·§5.7 터미널 제어).
//! Windows CI 러너(기본 터미널이 Windows Terminal 이라 새 콘솔이 ConPTY 탭으로 넘어갈 수 있다)에서 돈다.
//!
//! - 콘솔 붙기는 프로세스 전체 상태라 이 파일의 시험은 `serial()` 로 하나씩 돈다.
//! - 자식은 `CreateProcessW` 로 직접 띄운다 — std `Command` 는 부모의 표준 핸들(CI 에서는 파이프)을
//!   `STARTF_USESTDHANDLES` 로 넘기므로, 새 콘솔을 받은 자식의 표준 입력이 그 콘솔이 되지 않는다.
//! - 자식(node.exe)은 준비되면 준비 파일을 쓰고, 잡히지 않은 오류는 `<준비 파일>.err` 에 남긴다(새 콘솔에 쓴 글은 시험이 볼 수 없다).
//! - 창에 관한 assert(`terminal_window` 가 보이는 창, headless 거짓)는 화면 세션(데스크톱)이 있어야 성립한다 — CI 러너에는 있다.
//!   창 없는 콘솔(`CREATE_NO_WINDOW`)은 조상 사슬에 따라 달라지므로 값만 기록에 남긴다.
#![cfg(windows)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, Once};
use std::time::Duration;

use claude_board_core::console::{self, Input};
use claude_board_core::proc::{self, ProcessTable};
use claude_board_core::quote;
use common::{note, read_or_none, wait_for, TempDir};
use serde_json::Value;
use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows_sys::Win32::Storage::FileSystem::GetFileType;
use windows_sys::Win32::System::Console::{
    AttachConsole, FreeConsole, GetConsoleWindow, GetStdHandle, SetStdHandle, STD_ERROR_HANDLE, STD_HANDLE,
    STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, TerminateProcess, WaitForSingleObject, CREATE_NEW_CONSOLE, CREATE_NO_WINDOW,
    DETACHED_PROCESS, PROCESS_INFORMATION, STARTUPINFOW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClassNameW, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
};

static SERIAL: Mutex<()> = Mutex::new(());
static CONTEXT: Once = Once::new();

/// 이 파일의 시험을 하나씩 돌린다(앞 시험이 패닉해도 다음 시험은 돈다). 처음 한 번 시험 프로세스의 처지를 기록에 남긴다.
fn serial() -> MutexGuard<'static, ()> {
    let guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    CONTEXT.call_once(|| {
        let me = std::process::id();
        let table = ProcessTable::capture();
        let chain: Vec<String> =
            table.ancestors(me, 12).iter().map(|&p| format!("{p}({})", table.name(p).unwrap_or("?"))).collect();
        // SAFETY: 인자 없는 조회
        let own = unsafe { GetConsoleWindow() } as isize;
        note(&format!(
            "시험 프로세스 pid {me} 조상 [{}] / 표준 핸들(값, 파일 종류) {:?} / 자기 콘솔 창 {} / WindowsTerminal.exe {:?} OpenConsole.exe {:?} conhost.exe {} 개",
            chain.join(" → "),
            std_state(),
            describe_window(own),
            table.pids_named("WindowsTerminal.exe"),
            table.pids_named("OpenConsole.exe"),
            table.pids_named("conhost.exe").len(),
        ));
    });
    guard
}

const STD: [STD_HANDLE; 3] = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];

/// 표준 핸들 세 개의 (값, `GetFileType`) — 값이 같아도 닫힌 핸들이면 종류가 UNKNOWN(0)으로 바뀐다.
fn std_state() -> [(usize, u32); 3] {
    // SAFETY: 핸들 값 읽기와 종류 조회만 한다
    STD.map(|which| unsafe {
        let h = GetStdHandle(which);
        (h as usize, GetFileType(h))
    })
}

/// console.rs 머리 주석의 약속 — 붙었다 뗀 뒤 붙기 전 표준 핸들을 되돌린다. `f` 앞뒤의 표준 핸들이 같은지 본다.
fn keeps_std<T>(what: &str, f: impl FnOnce() -> T) -> T {
    let before = std_state();
    let out = f();
    let after = std_state();
    assert_eq!(after, before, "{what} 뒤 표준 핸들(값, 파일 종류)이 바뀌었다");
    out
}

/// `CreateProcessW` 로 띄운 자식 — 놓을 때 끝내고 끝날 때까지 기다린다(WT 탭으로 넘어간 콘솔은 저절로 닫히지 않을 수 있다).
struct Child {
    pid: u32,
    process: HANDLE,
    what: &'static str,
}

impl Child {
    /// `args[0]` 을 실행 파일로, 표준 핸들을 넘기지 않고(새 콘솔이면 그 콘솔이 표준 입출력이 된다) 핸들 상속 없이 띄운다.
    fn spawn(what: &'static str, args: &[&str], flags: u32) -> Child {
        let line = quote::join_windows(args);
        let app: Vec<u16> = args[0].encode_utf16().chain(std::iter::once(0)).collect();
        let mut cmd: Vec<u16> = line.encode_utf16().chain(std::iter::once(0)).collect();
        let si = STARTUPINFOW { cb: size_of::<STARTUPINFOW>() as u32, ..Default::default() };
        let mut pi = PROCESS_INFORMATION::default();
        // SAFETY: NUL 로 끝나는 문자열과 채운 구조체를 넘긴다. 받은 핸들은 Child 가 닫는다
        let ok = unsafe {
            CreateProcessW(
                app.as_ptr(),
                cmd.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                flags,
                std::ptr::null(),
                std::ptr::null(),
                &si,
                &mut pi,
            )
        } != 0;
        assert!(ok, "{what} 자식을 띄우지 못했다: {} (명령줄 {line})", std::io::Error::last_os_error());
        // SAFETY: 쓰지 않는 주 스레드 핸들을 닫는다
        unsafe { CloseHandle(pi.hThread) };
        Child { pid: pi.dwProcessId, process: pi.hProcess, what }
    }

    /// 끝났으면 종료 코드, 아직 돌면 None.
    fn exit_code(&self) -> Option<u32> {
        // SAFETY: 우리가 가진 프로세스 핸들로 상태만 읽는다(0 = WAIT_OBJECT_0, 끝남)
        if unsafe { WaitForSingleObject(self.process, 0) } != 0 {
            return None;
        }
        let mut code = 0u32;
        // SAFETY: 위와 같은 핸들로 종료 코드만 읽는다
        (unsafe { GetExitCodeProcess(self.process, &mut code) } != 0).then_some(code)
    }
}

impl Drop for Child {
    fn drop(&mut self) {
        // SAFETY: 우리가 띄운 프로세스를 끝내고 기다린 뒤 핸들을 한 번 닫는다
        unsafe {
            TerminateProcess(self.process, 1);
            WaitForSingleObject(self.process, 10_000);
            CloseHandle(self.process);
        }
    }
}

/// 모든 자식 스크립트의 머리 — `node <스크립트> <준비 파일> <받은 입력 파일> [raw]`.
const PRELUDE: &str = r#"
const fs = require('fs');
const [ready, out, mode] = process.argv.slice(2);
process.on('uncaughtException', (e) => {
  fs.writeFileSync(ready + '.err', String((e && e.stack) || e));
  process.exit(1);
});
function done(info) {
  const all = Object.assign({ pid: process.pid, stdinIsTTY: require('tty').isatty(0), node: process.version }, info);
  fs.writeFileSync(ready + '.tmp', JSON.stringify(all));
  fs.renameSync(ready + '.tmp', ready);
}
"#;

/// 아무것도 하지 않고 기다리는 자식.
const IDLE: &str = r#"
done({});
setInterval(() => {}, 1 << 30);
"#;

/// 표준 입력(콘솔)에서 받은 덩어리마다 16진수 한 줄을 붙여 쓴다. `raw` 면 Claude Code(Ink)처럼 setRawMode(true).
const READER: &str = r#"
if (!process.stdin.isTTY) throw new Error('표준 입력이 콘솔이 아니다');
if (mode === 'raw') process.stdin.setRawMode(true);
process.stdin.on('data', (d) => fs.appendFileSync(out, Buffer.from(d).toString('hex') + '\n'));
done({ raw: process.stdin.isRaw === true });
"#;

/// node 자식을 띄우고 준비 파일을 기다린다. (자식, 준비 정보, 받은 입력 파일)
fn node_child(node: &Path, dir: &Path, name: &'static str, body: &str, mode: &str, flags: u32) -> (Child, Value, PathBuf) {
    let script = dir.join(format!("{name}.js"));
    fs::write(&script, format!("{PRELUDE}\n{body}")).unwrap();
    let ready = dir.join(format!("{name}.ready"));
    let out = dir.join(format!("{name}.out"));
    let paths = [node, script.as_path(), ready.as_path(), out.as_path()]
        .map(|p| p.to_str().expect("임시 경로가 유니코드가 아니다").to_string());
    let mut args: Vec<&str> = paths.iter().map(String::as_str).collect();
    if !mode.is_empty() {
        args.push(mode);
    }
    let child = Child::spawn(name, &args, flags);
    let err = dir.join(format!("{name}.ready.err"));
    let got = wait_for(Duration::from_secs(60), || {
        if let Ok(s) = fs::read_to_string(&ready) {
            return Some(Some(s));
        }
        (err.exists() || child.exit_code().is_some()).then_some(None)
    });
    let Some(Some(text)) = got else {
        panic!(
            "{name} 자식(pid {})이 준비되지 않았다 — 종료 코드 {:?}, 오류 파일: {}",
            child.pid,
            child.exit_code(),
            read_or_none(&err)
        );
    };
    let info: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("{name} 준비 파일이 JSON 이 아니다({e}): {text}"));
    note(&format!("{name}: pid {} 준비됨 {info}", child.pid));
    (child, info, out)
}

/// 창 하나의 모습 — 클래스·보임·주인 프로세스·소유 창. 0 이면 "없음".
fn describe_window(hwnd: isize) -> String {
    if hwnd == 0 {
        return "없음".to_string();
    }
    let h = hwnd as HWND;
    let mut pid = 0u32;
    let mut class = [0u16; 256];
    // SAFETY: 창 조회만 한다. 버퍼 크기를 정확히 넘긴다
    let (n, visible, owner) = unsafe {
        GetWindowThreadProcessId(h, &mut pid);
        let n = GetClassNameW(h, class.as_mut_ptr(), class.len() as i32).max(0) as usize;
        (n, IsWindowVisible(h) != 0, GetWindow(h, GW_OWNER) as isize)
    };
    let table = ProcessTable::capture();
    format!(
        "HWND {hwnd:#x} 클래스 {:?} 보임 {visible} 주인 pid {pid}({}) 소유 창 {owner:#x}",
        String::from_utf16_lossy(&class[..n]),
        table.name(pid).unwrap_or("?")
    )
}

/// 진단용 — 그 프로세스의 콘솔 창(`GetConsoleWindow`)을 직접 붙어서 읽는다. terminal_window 가 무엇을 보고 골랐는지 남기려고.
fn console_window_of(pid: u32) -> Option<isize> {
    // SAFETY: 표준 핸들 값을 읽어 두고, 붙었다 뗀 뒤 되돌린다(console.rs 와 같은 순서)
    unsafe {
        let saved = STD.map(|which| GetStdHandle(which));
        FreeConsole();
        let w = (AttachConsole(pid) != 0).then(|| GetConsoleWindow() as isize);
        FreeConsole();
        for (which, h) in STD.iter().zip(saved) {
            SetStdHandle(*which, h);
        }
        w
    }
}

/// 붙기·콘솔 창·터미널 창·headless·제목을 읽어 기록에 남기고 돌려준다(실패 메시지에도 그 줄을 넣는다).
struct Seen {
    term: Option<isize>,
    headless: bool,
    title: Option<String>,
    line: String,
}

fn report_window(label: &str, pid: u32) -> Seen {
    let attach = keeps_std("can_attach", || console::can_attach(pid));
    let raw = console_window_of(pid);
    let term = keeps_std("terminal_window", || console::terminal_window(pid));
    let argv = vec!["claude.exe".to_string()];
    let headless = keeps_std("is_headless", || console::is_headless(pid, &argv));
    let title = keeps_std("title", || console::title(pid));
    let raw_text = match raw {
        Some(w) => describe_window(w),
        None => "붙지 못함".to_string(),
    };
    let line = format!(
        "{label}(pid {pid}): can_attach {attach} / 콘솔 창 {raw_text} / terminal_window {} / is_headless(-p 없음) {headless} / 제목 {title:?}",
        term.map_or_else(|| "None".to_string(), describe_window)
    );
    note(&line);
    Seen { term, headless, title, line }
}

/// 받은 입력 파일의 16진수 줄들(다 쓴 줄만)을 이어 붙인 바이트.
fn received(out: &Path) -> (Vec<u8>, Vec<String>) {
    let text = fs::read_to_string(out).unwrap_or_default();
    let lines: Vec<String> = text.split_inclusive('\n').filter(|l| l.ends_with('\n')).map(|l| l.trim().to_string()).collect();
    let bytes = lines
        .iter()
        .flat_map(|l| (0..l.len() / 2).map(move |i| u8::from_str_radix(&l[2 * i..2 * i + 2], 16).unwrap_or(0)))
        .collect();
    (bytes, lines)
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" ")
}

/// 자식이 받은 바이트가 정확히 `want` 가 될 때까지 기다린다(넘치는 바이트도 잡도록 다 받은 뒤 잠깐 더 본다).
fn expect_bytes(child: &Child, out: &Path, want: &[u8], what: &str) {
    let _ = wait_for(Duration::from_secs(15), || (received(out).0.len() >= want.len()).then_some(()));
    std::thread::sleep(Duration::from_millis(300));
    let (got, chunks) = received(out);
    assert_eq!(
        got,
        want,
        "{what}: {} 가 받은 바이트가 다르다\n  받음 [{}] (덩어리 {chunks:?}, {:?})\n  기대 [{}] ({:?})\n  자식 종료 코드 {:?}",
        child.what,
        hex(&got),
        String::from_utf8_lossy(&got),
        hex(want),
        String::from_utf8_lossy(want),
        child.exit_code()
    );
}

#[test]
fn new_console_title_round_trip() {
    let _serial = serial();
    let Some(node) = common::node_or_skip("새 콘솔 제목") else { return };
    let dir = TempDir::new("console-title");
    let (child, _, _) = node_child(&node, dir.path(), "idle", IDLE, "", CREATE_NEW_CONSOLE);

    assert!(keeps_std("can_attach", || console::can_attach(child.pid)), "새 콘솔 자식(pid {})에 붙지 못했다", child.pid);
    let before = keeps_std("title", || console::title(child.pid));
    // 보드가 실제로 쓰는 모양(상태 아이콘 = 서로게이트 쌍, 한글, 따옴표)
    let title = format!("🔵 cb-test {} · 한글 é \"q\"", child.pid);
    assert!(keeps_std("set_title", || console::set_title(child.pid, &title)), "set_title 이 실패했다(pid {})", child.pid);
    let after = keeps_std("title", || console::title(child.pid));
    assert_eq!(after.as_deref(), Some(title.as_str()), "제목 왕복이 다르다(바꾸기 전 제목 {before:?})");

    // -p 로 띄운 세션은 콘솔·창과 상관없이 headless
    let argv: Vec<String> = ["claude.exe", "-p", "hi"].map(String::from).to_vec();
    assert!(keeps_std("is_headless", || console::is_headless(child.pid, &argv)), "-p 인데 headless 가 아니다");
    let seen = report_window("새 콘솔 자식", child.pid);
    // 화면 세션(데스크톱)이 있어야 한다 — 콘솔 창(WT 탭이면 그 소유 창인 WT 창)이 보이는 터미널 창이고, 그래서 headless 가 아니다
    let Some(term) = seen.term else { panic!("새 콘솔 자식의 터미널 창을 찾지 못했다 — {}", seen.line) };
    // SAFETY: 창 핸들 조회만 한다
    assert!(unsafe { IsWindowVisible(term as HWND) } != 0, "터미널 창이 보이지 않는다 — {}", seen.line);
    assert!(!seen.headless, "새 콘솔 자식이 headless 로 판별됐다 — {}", seen.line);
}

#[test]
fn cannot_attach_without_console() {
    let _serial = serial();
    let Some(node) = common::node_or_skip("콘솔 없는 자식") else { return };
    let dir = TempDir::new("console-none");

    let (detached, _, _) = node_child(&node, dir.path(), "detached", IDLE, "", DETACHED_PROCESS);
    assert!(proc::alive(detached.pid, None), "콘솔 없는 자식(pid {})이 살아 있어야 한다", detached.pid);
    assert!(!keeps_std("can_attach", || console::can_attach(detached.pid)), "콘솔 없는 자식(DETACHED_PROCESS)에 붙었다");
    assert!(!keeps_std("set_title", || console::set_title(detached.pid, "x")), "콘솔 없는 자식의 제목을 바꿨다고 한다");
    assert!(!keeps_std("send_input", || console::send_input(detached.pid, &[Input::Enter])), "콘솔 없는 자식에 입력을 넣었다고 한다");
    let seen = report_window("콘솔 없는 자식(DETACHED_PROCESS)", detached.pid);
    assert_eq!(seen.term, None, "콘솔 없는 자식에 터미널 창이 있다고 한다 — {}", seen.line);
    assert!(seen.headless, "콘솔 없는 자식이 headless 가 아니다 — {}", seen.line);
    assert_eq!(seen.title, None, "콘솔 없는 자식의 제목을 읽었다 — {}", seen.line);

    // 없는 pid — 홀수(pid 는 4 의 배수, 커널은 아래 두 비트를 무시)이고 u32::MAX(ATTACH_PARENT_PROCESS)가 아니다
    let ghost = 0x7FFF_FFFD_u32;
    let table = ProcessTable::capture();
    assert!(table.name(ghost & !3).is_none(), "없을 줄 안 pid {:#x} 가 있다", ghost & !3);
    assert!(!keeps_std("can_attach", || console::can_attach(ghost)), "없는 pid 에 붙었다");
    assert_eq!(keeps_std("title", || console::title(ghost)), None);

    // 창 없는 콘솔(CREATE_NO_WINDOW) — 조상 사슬에 보이는 창이 있는지에 따라 달라지므로 값만 남긴다
    // (CI: 조상이 cargo → bash → Runner.Worker → 서비스라 terminal_window None, headless 참)
    let (hidden, _, _) = node_child(&node, dir.path(), "hidden", IDLE, "", CREATE_NO_WINDOW);
    report_window("창 없는 콘솔 자식(CREATE_NO_WINDOW)", hidden.pid);
}

#[test]
fn send_input_line_mode() {
    let _serial = serial();
    let Some(node) = common::node_or_skip("줄 입력") else { return };
    let dir = TempDir::new("console-line");
    let (child, _, out) = node_child(&node, dir.path(), "line", READER, "line", CREATE_NEW_CONSOLE);

    let text = "hello 한글 é \"q\"";
    assert!(
        keeps_std("send_input", || console::send_input(child.pid, &[Input::Text(text), Input::Enter])),
        "send_input 이 실패했다(pid {})",
        child.pid
    );
    // 줄 입력(ReadConsoleW)은 Enter 를 \r\n 으로 돌려준다
    expect_bytes(&child, &out, format!("{text}\r\n").as_bytes(), "줄 입력");
}

#[test]
fn send_input_raw_mode() {
    let _serial = serial();
    let Some(node) = common::node_or_skip("raw 입력") else { return };
    let dir = TempDir::new("console-raw");
    let (child, info, out) = node_child(&node, dir.path(), "raw", READER, "raw", CREATE_NEW_CONSOLE);
    assert_eq!(info["raw"], true, "setRawMode(true) 가 먹지 않았다: {info}");

    // 일시중지 — Esc 하나
    assert!(keeps_std("send_input", || console::send_input(child.pid, &[Input::Escape])), "Esc 를 넣지 못했다");
    expect_bytes(&child, &out, b"\x1b", "raw Esc");
    // 재개 — 글자와 Enter(raw 에서는 \r)
    assert!(
        keeps_std("send_input", || console::send_input(child.pid, &[Input::Text("ab"), Input::Enter])),
        "글자·Enter 를 넣지 못했다"
    );
    expect_bytes(&child, &out, b"\x1bab\r", "raw 글자·Enter");
}
