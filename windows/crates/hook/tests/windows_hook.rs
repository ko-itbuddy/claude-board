//! Windows 실기 시험 — hook 끝까지: 가짜 Claude Code(node.exe 를 `claude.exe` 로 복사한 것)가 hook 을 자식으로 띄우면
//! hook 이 실제 프로세스 트리에서 그 프로세스를 조상으로 찾아 세션 기록을 쓰는지 (SPEC §3.2–3.4). Windows CI 러너에서 돈다.
//!
//! - 가짜 claude 는 실제 세션처럼 새 콘솔(`CREATE_NEW_CONSOLE`, 러너에서는 Windows Terminal 탭일 수 있다)에서 돌고,
//!   hook 을 셸 없이 `child_process.spawnSync` 로 두 번(SessionStart → UserPromptSubmit) 띄운 뒤 결과를 파일로 남기고 기다린다.
//!   시험이 확인을 끝낼 때까지 살아 있어야 한다(끝나면 alive 판단이 달라진다). 끝나면 Drop 이 끝낸다.
//! - 실제 사용자의 ~/.claude 를 건드리지 않게 `CLAUDE_BOARD_HOME` 을 임시 폴더로 한다.
//! - -p 가 아닌 세션의 headless·콘솔 제목은 아직 무엇이 맞는지 정하지 않아 값만 기록에 남긴다.
#![cfg(windows)]

#[path = "../../core/tests/common/mod.rs"]
mod common;

use std::fs::{self, File};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use claude_board_core::proc::{self, ProcessTable};
use claude_board_core::{console, state};
use common::{listing, note, read_or_none, wait_for, TempDir};
use serde_json::{json, Value};
use windows_sys::Win32::System::Threading::{CREATE_NEW_CONSOLE, CREATE_NO_WINDOW};

/// 시험 프로세스도 콘솔에 붙었다 떼므로(제목 읽기) 시험을 하나씩 돌린다.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// 가짜 Claude Code — 환경변수 CB_HOOK(hook 실행 파일), CB_RUNS([{state, input}]), CB_SESSION(세션 파일), CB_RESULT(결과 파일).
/// hook 마다 종료 코드·출력과 그 직후 세션 파일 내용을 남긴다. Claude Code 처럼 창을 숨기고(windowsHide) 표준 입출력은 파이프.
const FAKE_CLAUDE: &str = r#"
const fs = require('fs');
const { spawnSync } = require('child_process');
const e = process.env;
const result = { pid: process.pid, argv: process.argv, runs: [] };
try {
  for (const r of JSON.parse(e.CB_RUNS)) {
    const p = spawnSync(e.CB_HOOK, ['state', r.state], { input: JSON.stringify(r.input), windowsHide: true, timeout: 30000 });
    let record = null;
    try { record = fs.readFileSync(e.CB_SESSION, 'utf8'); } catch (_) {}
    result.runs.push({
      status: p.status, signal: p.signal, error: p.error ? String(p.error) : null,
      stdout: String(p.stdout || ''), stderr: String(p.stderr || ''), record,
    });
  }
} catch (err) {
  result.error = String((err && err.stack) || err);
}
fs.writeFileSync(e.CB_RESULT + '.tmp', JSON.stringify(result));
fs.renameSync(e.CB_RESULT + '.tmp', e.CB_RESULT);
setInterval(() => {}, 1 << 30);
"#;

/// 시험이 끝나면(패닉 포함) 가짜 claude 를 끝내고 기다린다 — 그 뒤에야 임시 폴더의 claude.exe 를 지울 수 있다.
struct Kill(std::process::Child);

impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// 세션 다시 열기에 넘어가야 할 인자 — 공백·셸 특수문자·따옴표·한글·따옴표 앞 역슬래시가 든 값들.
/// 각 값은 carry_args 를 지나 **원소 하나**로 남아야 한다(SPEC §3.3).
const HOSTILE: [&str; 10] = [
    "--model",
    "x; calc & echo",
    "--add-dir",
    r"C:\dir with space\한글",
    "b&c",
    "--settings",
    r#"{"k": "v w"}"#,
    "--plugin-dir",
    r"C:\trailing dir\",
    "--chrome",
];

/// hook 직후 세션 파일 내용(JSON 객체). 없는 필드는 Null 로 읽히게 Value 로 둔다.
fn record_of(run: &Value, what: &str, result: &Value) -> Value {
    let text = run["record"].as_str().unwrap_or_else(|| panic!("{what} 뒤 세션 파일이 없다 — 결과 {result}"));
    match serde_json::from_str::<Value>(text) {
        Ok(v @ Value::Object(_)) => v,
        other => panic!("{what} 뒤 세션 파일이 JSON 객체가 아니다({other:?}): {text}"),
    }
}

fn session(tag: &str, print_mode: bool) {
    let _serial = serial();
    let Some(node) = common::node_or_skip(tag) else { return };
    let dir = TempDir::new(tag);
    let hook = env!("CARGO_BIN_EXE_claude-board-hook");

    // node.exe 를 claude.exe 로 복사 — proc.rs 의 is_claude_exe 는 이름이 claude.exe 면 Claude Code 로 본다(네이티브 설치와 같은 이름)
    let bin = dir.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let claude = bin.join("claude.exe");
    fs::copy(&node, &claude).unwrap_or_else(|e| panic!("{} 를 {} 로 복사하지 못했다: {e}", node.display(), claude.display()));
    let probe = Command::new(&claude)
        .args(["-e", "process.stdout.write('ok ' + process.version)"])
        .stdin(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    match &probe {
        Ok(o) if o.status.success() && o.stdout.starts_with(b"ok ") => {}
        other => panic!("node.exe 를 claude.exe 로 복사한 것이 돌지 않는다: {other:?}"),
    }

    let home = dir.path().join("home");
    let proj = dir.path().join("proj 한글");
    let transcript = proj.join("t.jsonl");
    let sid = format!("wintest-{tag}-{}", std::process::id());
    let sessions = home.join(".claude").join("notifier").join("sessions");
    let session_file = sessions.join(format!("{sid}.json"));
    let result_file = dir.path().join("result.json");
    let script = dir.path().join("fake-claude.js");
    fs::write(&script, FAKE_CLAUDE).unwrap();
    let base = json!({"session_id": sid, "transcript_path": transcript.to_str().unwrap(), "cwd": proj.to_str().unwrap()});
    let input = |extra: Value| {
        let mut m = base.as_object().unwrap().clone();
        m.extend(extra.as_object().unwrap().clone());
        Value::Object(m)
    };
    let prompt = "hello 한글 prompt";
    let runs = json!([
        {"state": "idle", "input": input(json!({"hook_event_name": "SessionStart", "source": "startup"}))},
        {"state": "working", "input": input(json!({"hook_event_name": "UserPromptSubmit", "prompt": prompt}))},
    ]);

    let log = dir.path().join("fake-claude.log");
    let log_file = File::create(&log).unwrap();
    let mut cmd = Command::new(&claude);
    cmd.arg(&script);
    if print_mode {
        cmd.arg("-p");
    }
    cmd.args(HOSTILE)
        .env("CLAUDE_BOARD_HOME", &home)
        .env_remove("CLAUDE_BOARD_NOTIFIER_DIR")
        .env("CB_HOOK", hook)
        .env("CB_RUNS", runs.to_string())
        .env("CB_SESSION", &session_file)
        .env("CB_RESULT", &result_file)
        .stdin(Stdio::null())
        .stdout(log_file.try_clone().unwrap())
        .stderr(log_file)
        .creation_flags(CREATE_NEW_CONSOLE);
    let mut child = Kill(cmd.spawn().unwrap_or_else(|e| panic!("가짜 claude 를 띄우지 못했다: {e}")));
    let pid = child.0.id();
    let me = std::process::id();

    let text = wait_for(Duration::from_secs(90), || {
        if let Ok(s) = fs::read_to_string(&result_file) {
            return Some(Some(s));
        }
        matches!(child.0.try_wait(), Ok(Some(_))).then_some(None)
    });
    let table = ProcessTable::capture();
    let tree = format!("가짜 claude pid {pid} 이름 {:?} 부모 {:?}(시험 {me})", table.name(pid), table.parent(pid));
    let Some(Some(text)) = text else {
        panic!(
            "가짜 claude 가 결과를 쓰지 않았다 — 종료 상태 {:?}, {tree}, 로그: {}, 폴더 [{}]",
            child.0.try_wait(),
            read_or_none(&log),
            listing(dir.path())
        );
    };
    let result: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("결과가 JSON 이 아니다({e}): {text}"));
    assert!(result.get("error").is_none(), "가짜 claude 오류: {result}");
    assert_eq!(result["pid"], pid, "가짜 claude 가 쓴 pid 가 다르다: {result}");
    assert!(table.name(pid).is_some_and(|n| n.eq_ignore_ascii_case("claude.exe")), "{tree}");
    assert_eq!(table.parent(pid), Some(me), "{tree}");
    let runs = result["runs"].as_array().unwrap_or_else(|| panic!("결과에 runs 가 없다: {result}"));
    assert_eq!(runs.len(), 2, "hook 실행 수: {result}");
    for (i, run) in runs.iter().enumerate() {
        // hook 은 아무것도 출력하지 않고 늘 0 으로 끝난다(SPEC §3.2)
        assert_eq!(run["status"], 0, "hook {i} 종료 코드: {run}");
        assert_eq!(run["stdout"], "", "hook {i} 이 stdout 에 썼다: {run}");
        assert_eq!(run["stderr"], "", "hook {i} 이 stderr 에 썼다: {run}");
    }

    // 1) SessionStart — 조상에서 찾은 claude 프로세스 정보
    let rec = record_of(&runs[0], "SessionStart", &result);
    let ctx = format!("{tree}, 가짜 claude 가 받은 argv {}, 기록 {rec}", result["argv"]);
    assert_eq!(rec["state"], "idle", "{ctx}");
    assert_eq!(rec["pid"], pid, "hook 이 찾은 pid 가 가짜 claude 가 아니다 — {ctx}");
    let want_ctime = proc::creation_time(pid);
    let got_ctime = rec.get("pid_ctime").and_then(Value::as_f64);
    assert!(
        matches!((got_ctime, want_ctime), (Some(a), Some(b)) if (a - b).abs() < 1e-6),
        "pid_ctime {got_ctime:?} 이 creation_time {want_ctime:?} 과 다르다 — {ctx}"
    );
    let carried: Vec<&str> = HOSTILE.to_vec();
    assert_eq!(rec["args"], json!(carried), "carry_args 결과가 다르다(값마다 원소 하나여야 한다) — {ctx}");
    let mut resume = carried.clone();
    resume.extend(["--resume", sid.as_str()]);
    assert_eq!(rec["resume_args"], json!(resume), "resume_args 가 다르다 — {ctx}");
    assert_eq!(rec["tty"], Value::Null, "{ctx}");
    assert_eq!(rec["session_id"], sid.as_str(), "{ctx}");
    assert_eq!(rec["cwd"], proj.to_str().unwrap(), "{ctx}");
    assert_eq!(rec["project"], "proj 한글", "{ctx}");
    assert_eq!(rec["transcript_path"], transcript.to_str().unwrap(), "{ctx}");
    let headless = rec["headless"].clone();
    if print_mode {
        assert_eq!(headless, true, "-p 로 띄운 세션이 headless 가 아니다 — {ctx}");
    }

    // 2) UserPromptSubmit — 기록된 pid·pid_ctime 으로 alive 가 참이어야 프로세스를 다시 찾지 않는다(started 그대로)
    let rec2 = record_of(&runs[1], "UserPromptSubmit", &result);
    let ctx2 = format!("{tree}, 첫 기록 {rec}, 둘째 기록 {rec2}");
    assert_eq!(rec2["state"], "working", "{ctx2}");
    assert_eq!(rec2["topic"], prompt, "{ctx2}");
    assert_eq!(rec2["pid"], pid, "{ctx2}");
    assert_eq!(rec2["started"], rec["started"], "기록된 pid_ctime 으로 본 alive 가 거짓이라 프로세스를 다시 찾았다 — {ctx2}");
    assert_eq!(rec2["headless"], headless, "{ctx2}");
    assert_eq!(rec2["resume_args"], json!(resume), "{ctx2}");

    // 아직 assert 하지 않는 값 — headless(-p 아님)·콘솔 제목·터미널 창. headless 가 거짓이면 hook 이 제목을 바꿨어야 한다
    let expect_title = state::title_text("working", rec2.as_object().unwrap());
    let title = console::title(pid);
    note(&format!(
        "hook {tag}: headless {headless} / 콘솔 제목 {title:?} (headless=false 면 기대 {expect_title:?}) / can_attach {} / terminal_window {:?} / {tree}",
        console::can_attach(pid),
        console::terminal_window(pid).map(|w| format!("{w:#x}"))
    ));
    assert_eq!(fs::read_to_string(&session_file).ok().as_deref(), runs[1]["record"].as_str(), "세션 파일이 마지막 hook 뒤에 바뀌었다");
}

#[test]
fn hook_finds_fake_claude_in_console() {
    session("hook-console", false);
}

#[test]
fn hook_print_mode_is_headless() {
    session("hook-print", true);
}
