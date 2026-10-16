//! Windows 실기 시험 — 프로세스 표·생성 시각·살아 있는지·명령줄·끝내기 (`proc`, SPEC §3.4·§5.1). Windows CI 러너에서 돈다.
#![cfg(windows)]

mod common;

use std::fs;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::Duration;

use claude_board_core::clock;
use claude_board_core::proc::{self, ProcessTable};
use common::{note, wait_for, TempDir};
use serde_json::Value;
use windows_sys::Win32::System::Threading::CREATE_NO_WINDOW;

/// 자기 pid·실행 인자를 준비 파일에 쓰고 기다리는 자식 — `node <스크립트> <준비 파일> <인자…>`.
const ARGV_CHILD: &str = r#"
const fs = require('fs');
const ready = process.argv[2];
fs.writeFileSync(ready + '.tmp', JSON.stringify({ pid: process.pid, argv: process.argv }));
fs.renameSync(ready + '.tmp', ready);
setInterval(() => {}, 1 << 30);
"#;

/// 시험이 끝나면(패닉 포함) 자식을 끝내고 기다린다.
struct Kill(std::process::Child);

impl Drop for Kill {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn self_table_times_and_command_line() {
    let me = std::process::id();
    let table = ProcessTable::capture();
    let exe = std::env::current_exe().unwrap();
    let exe_name = exe.file_name().unwrap().to_string_lossy().into_owned();
    let name = table.name(me);
    assert!(
        name.is_some_and(|n| n.eq_ignore_ascii_case(&exe_name)),
        "프로세스 표의 자기 이름 {name:?} 이 실행 파일 {exe_name} 과 다르다"
    );
    let parent = table.parent(me);
    assert!(parent.is_some_and(|p| p != 0), "자기 부모 pid 가 없다: {parent:?}");
    let chain: Vec<String> = table.ancestors(me, 12).iter().map(|&p| format!("{p}({})", table.name(p).unwrap_or("?"))).collect();
    note(&format!("proc: 시험 프로세스 {me}({exe_name}) 조상 [{}]", chain.join(" → ")));

    // 생성 시각 — 이 시험 프로세스는 방금 떴다
    let ctime = proc::creation_time(me);
    let now = clock::now();
    let Some(ct) = ctime else { panic!("자기 생성 시각을 읽지 못했다") };
    assert!((-2.0..600.0).contains(&(now - ct)), "자기 생성 시각 {ct} 이 지금 {now} 과 너무 멀다(차이 {}초)", now - ct);

    // 살아 있는지 — 생성 시각이 허용 오차(0.001초)보다 크게 어긋나면 다른 프로세스(pid 재사용)로 본다
    assert!(proc::alive(me, None));
    assert!(proc::alive(me, Some(ct)), "자기 생성 시각 {ct} 으로 본 alive 가 거짓");
    assert!(!proc::alive(me, Some(ct + 10.0)), "10초 어긋난 생성 시각인데 살아 있다고 한다");
    assert!(!proc::alive(me, Some(ct - 0.01)), "0.01초 어긋난 생성 시각인데 살아 있다고 한다");

    // 명령줄 — 시험 틀이 받은 인자와 같다
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(proc::command_line(me).as_deref(), Some(args.as_slice()), "자기 명령줄이 std::env::args 와 다르다");

    // 없는 pid — 홀수(커널은 pid 의 아래 두 비트를 무시)
    let ghost = 0x7FFF_FFFD_u32;
    assert!(table.name(ghost & !3).is_none(), "없을 줄 안 pid {:#x} 가 있다", ghost & !3);
    assert!(!proc::alive(ghost, None), "없는 pid 가 살아 있다고 한다");
    assert_eq!(proc::creation_time(ghost), None);
    assert_eq!(proc::command_line(ghost), None);
}

#[test]
fn child_tree_command_line_and_terminate() {
    let Some(node) = common::node_or_skip("자식 프로세스") else { return };
    let dir = TempDir::new("proc");
    let script = dir.path().join("argv.js");
    fs::write(&script, ARGV_CHILD).unwrap();
    let ready = dir.path().join("argv.ready");
    let script_s = script.to_str().unwrap().to_string();
    let ready_s = ready.to_str().unwrap().to_string();
    // CommandLineToArgvW 로 되돌리기 까다로운 인자들 — 공백·따옴표·한글·빈 인자·셸 특수문자·따옴표 앞 역슬래시
    let hostile = ["a b", "say \"hi\"", "한글 인자", "", "x; calc & echo", r"C:\dir with space\", r"back\\slash", "--model=\"m n\""];

    let me = std::process::id();
    let t0 = clock::now();
    let child = Command::new(&node)
        .arg(&script_s)
        .arg(&ready_s)
        .args(hostile)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .unwrap_or_else(|e| panic!("node 자식을 띄우지 못했다: {e}"));
    let t1 = clock::now();
    let mut child = Kill(child);
    let pid = child.0.id();

    let text = wait_for(Duration::from_secs(60), || {
        if let Ok(s) = fs::read_to_string(&ready) {
            return Some(Some(s));
        }
        matches!(child.0.try_wait(), Ok(Some(_))).then_some(None)
    });
    let Some(Some(text)) = text else {
        panic!("node 자식(pid {pid})이 준비되지 않았다 — 종료 상태 {:?}, 폴더 [{}]", child.0.try_wait(), common::listing(dir.path()));
    };
    let info: Value = serde_json::from_str(&text).unwrap_or_else(|e| panic!("준비 파일이 JSON 이 아니다({e}): {text}"));
    assert_eq!(info["pid"], pid, "자식이 쓴 pid 가 다르다: {info}");

    // 프로세스 표 — 부모는 이 시험 프로세스, 이름은 node.exe
    let table = ProcessTable::capture();
    assert_eq!(table.parent(pid), Some(me), "자식의 부모가 다르다(자식 이름 {:?})", table.name(pid));
    assert!(table.name(pid).is_some_and(|n| n.eq_ignore_ascii_case("node.exe")), "자식 이름이 {:?}", table.name(pid));
    assert!(table.children(me).contains(&pid), "children(시험) 에 자식 {pid} 가 없다: {:?}", table.children(me));
    assert_eq!(table.ancestors(pid, 1), [me]);

    // 생성 시각 — 띄우기 앞뒤 사이
    let ct = proc::creation_time(pid).unwrap_or_else(|| panic!("자식 생성 시각을 읽지 못했다"));
    assert!(t0 - 1.0 <= ct && ct <= t1 + 1.0, "자식 생성 시각 {ct} 이 띄운 시각 {t0}~{t1} 밖이다");
    assert!(proc::alive(pid, Some(ct)), "자식이 살아 있다고 하지 않는다");
    assert!(!proc::alive(pid, Some(ct + 10.0)), "10초 어긋난 생성 시각인데 자식이 살아 있다고 한다");

    // 명령줄 — 넘긴 인자가 그대로 돌아온다(argv[0] 은 실행 파일)
    let cl = proc::command_line(pid).unwrap_or_else(|| panic!("자식 명령줄을 읽지 못했다"));
    let mut want = vec![script_s.clone(), ready_s.clone()];
    want.extend(hostile.iter().map(|s| s.to_string()));
    assert!(cl.first().is_some_and(|a| a.to_ascii_lowercase().ends_with("node.exe")), "argv[0] 이 node.exe 가 아니다: {cl:?}");
    assert_eq!(cl[1..], want[..], "command_line 이 넘긴 인자와 다르다(node 가 받은 argv: {})", info["argv"]);
    let node_argv: Vec<String> =
        info["argv"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
    assert_eq!(node_argv.get(2..), Some(&want[1..]), "node 가 받은 인자가 넘긴 것과 다르다");

    // 끝내기 — TerminateProcess(1), 끝난 뒤에는 (핸들을 쥐고 있어 pid 가 재사용되지 않는 동안) alive 가 거짓
    assert!(proc::terminate(pid), "terminate 가 실패했다");
    let status = wait_for(Duration::from_secs(10), || child.0.try_wait().ok().flatten())
        .unwrap_or_else(|| panic!("terminate 뒤 10초가 지나도 자식이 끝나지 않았다"));
    assert_eq!(status.code(), Some(1), "종료 코드가 TerminateProcess 의 1 이 아니다");
    assert!(!proc::alive(pid, Some(ct)), "끝난 자식이 살아 있다고 한다");
    assert!(!proc::alive(pid, None), "끝난 자식이 (생성 시각 없이) 살아 있다고 한다");
    note(&format!("proc: 자식 {pid} 명령줄 {cl:?} 생성 시각 {ct}"));
}
