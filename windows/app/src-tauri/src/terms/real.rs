use super::*;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicUsize;
use std::time::Instant;

/// 끝나면 남은 터미널을 모두 끝낸다.
struct Guard(TermHub);

impl std::ops::Deref for Guard {
    type Target = TermHub;
    fn deref(&self) -> &TermHub {
        &self.0
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.kill_all();
    }
}

/// 한 터미널의 출력을 처음부터 모으는 시험용 보는 쪽. 받은 만큼 ack 한다.
#[derive(Default)]
struct View {
    bytes: Mutex<Vec<u8>>,
    exit: Mutex<Option<u32>>,
    gaps: AtomicUsize,
}

impl View {
    fn text(&self) -> String {
        String::from_utf8_lossy(&lock(&self.bytes)).into_owned()
    }

    fn wait_text(&self, pat: &str, secs: u64) {
        let end = Instant::now() + Duration::from_secs(secs);
        while !self.text().contains(pat) {
            assert!(Instant::now() < end, "{secs}초 안에 \"{pat}\" 가 나오지 않았다. 지금까지: {:?}", self.text());
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn wait_exit(&self, secs: u64) -> Option<u32> {
        let end = Instant::now() + Duration::from_secs(secs);
        while Instant::now() < end {
            if let Some(c) = *lock(&self.exit) {
                return Some(c);
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        None
    }
}

fn view(hub: &TermHub, id: &str) -> Arc<View> {
    let v = Arc::new(View::default());
    let (me, stream) = (Arc::clone(&v), Arc::clone(&hub.get(id).unwrap().stream));
    let sink: Sink = Arc::new(move |o| {
        match o {
            Out::Data(f) => {
                let seq = u64::from_le_bytes(f[..8].try_into().unwrap());
                lock(&me.bytes).extend_from_slice(&f[8..]);
                stream.ack(seq + f.len() as u64 - 8);
            }
            Out::Gap(_) => {
                me.gaps.fetch_add(1, Ordering::Relaxed);
            }
            Out::Exit(c) => *lock(&me.exit) = Some(c),
        }
        true
    });
    hub.attach(id, 0, sink).unwrap();
    v
}

fn find_node() -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.into_iter().map(|d| d.join("node.exe")).find(|p| p.is_file())
}

/// node 스크립트를 가짜 콘솔에서 돌린다. node 가 없으면(CI 가 아닐 때) None.
fn run_node(hub: &TermHub, script: &str, cols: u16, rows: u16) -> Option<(TermInfo, Arc<View>, PathBuf)> {
    let Some(node) = find_node() else {
        assert!(std::env::var_os("CI").is_none(), "CI 인데 PATH 에서 node.exe 를 찾지 못했다");
        eprintln!("node.exe 가 PATH 에 없어 이 시험을 건너뛴다");
        return None;
    };
    let dir = std::env::temp_dir().join(format!("cb-term-{}", crate::util::short_id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("child.js");
    fs::write(&file, script).unwrap();
    let info = hub.spawn_program("cmd", &node, &[file.to_string_lossy().into_owned()], &dir, cols, rows).unwrap();
    let v = view(hub, &info.id);
    Some((info, v, dir))
}

const KEEP_ALIVE: &str = "process.stdout.write('UP\\n'); setInterval(() => {}, 1000);";

/// ① cmd 의 한글 출력(UTF-8)과 종료 코드.
#[test]
fn cmd_prints_hangul_as_utf8_and_reports_exit_code() {
    let hub = Guard(TermHub::default());
    let cmd = std::env::var_os("ComSpec").map(PathBuf::from).expect("ComSpec");
    let args = ["/c".to_string(), "echo 한글 & exit 3".to_string()];
    let info = hub.spawn_program("cmd", &cmd, &args, &paths::home(), 80, 24).unwrap();
    let v = view(&hub, &info.id);
    assert_eq!(v.wait_exit(20), Some(3), "출력: {:?}", v.text());
    assert!(v.text().contains("한글"), "UTF-8 한글이 없다: {:?}", v.text());
    let after = hub.list().remove(0);
    assert_eq!((after.status, after.exit_code), ("exited", Some(3)));
    // 끝난 뒤에 붙어도 같은 출력과 종료가 다시 온다
    let late = view(&hub, &info.id);
    assert_eq!(late.wait_exit(5), Some(3));
    assert!(late.text().contains("한글"));
}

/// ② 원시 모드 에코: ESC·Ctrl+C·한글·문구가 바이트 그대로 자식에게 닿는다(자식이 받은 바이트를 16진수로 되돌려 준다).
#[test]
fn input_bytes_reach_a_raw_mode_child_unchanged() {
    let hub = Guard(TermHub::default());
    let script = "process.stdin.setRawMode(true); process.stdin.resume(); process.stdout.write('READY\\n');\
        process.stdin.on('data', d => process.stdout.write('HEX:' + d.toString('hex') + '\\n'));";
    let Some((info, v, dir)) = run_node(&hub, script, 100, 30) else { return };
    v.wait_text("READY", 20);
    let pieces: [&[u8]; 4] = [b"\x1b", b"\x03", "한글".as_bytes(), b"hello"];
    for p in pieces {
        hub.write(&info.id, p).unwrap();
        std::thread::sleep(Duration::from_millis(300));
    }
    let want: String = pieces.iter().flat_map(|p| p.iter()).map(|b| format!("{b:02x}")).collect();
    let end = Instant::now() + Duration::from_secs(10);
    let got = loop {
        let got: String = v.text().split("HEX:").skip(1).map(|s| s.chars().take_while(char::is_ascii_hexdigit).collect::<String>()).collect();
        if got.len() >= want.len() || Instant::now() > end {
            break got;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(got, want, "자식이 받은 바이트가 다르다. 출력: {:?}", v.text());
    let _ = fs::remove_dir_all(dir);
}

/// ③ 크기 바꾸기: 100×40 으로 바꾸면 자식이 `[Console]::WindowWidth == 100` 으로 본다. 무시해야 하는 값은 무시한다.
/// (node 는 쓰지 않는다 — libuv 는 가짜 콘솔의 크기 변화를 `columns` 에 반영하지 않았다.)
#[test]
fn resize_is_seen_by_the_child() {
    let hub = Guard(TermHub::default());
    let shell = pty::resolve_shell().expect("셸");
    let script = "$last = ''; while ($true) { $s = 'SIZE=' + [Console]::WindowWidth + 'x' + [Console]::WindowHeight;            if ($s -ne $last) { $last = $s; Write-Output $s }; Start-Sleep -Milliseconds 50 }";
    let args = ["-NoProfile".to_string(), "-Command".to_string(), script.to_string()];
    let info = hub.spawn_program("pwsh", &shell.path, &args, &paths::home(), 80, 24).unwrap();
    let v = view(&hub, &info.id);
    v.wait_text("SIZE=80x24", 30);
    hub.resize(&info.id, 100, 40).unwrap();
    v.wait_text("SIZE=100x40", 10);
    for (c, r) in [(1, 40), (100, 0), (100, 40)] {
        hub.resize(&info.id, c, r).unwrap();
    }
    assert_eq!((hub.list()[0].cols, hub.list()[0].rows), (100, 40));
}

/// ④ 보는 쪽 없이 고리(4MiB)보다 많은 6MiB 를 쏟아내도 자식이 끝나고, 고리는 오래된 것을 버린다.
#[test]
fn child_finishes_when_nobody_watches_even_past_ring_capacity() {
    let hub = Guard(TermHub::default());
    let script = "const line = 'x'.repeat(1023) + '\\n'; for (let i = 0; i < 6144; i++) process.stdout.write(line);\
        process.stdout.write('ALLDONE\\n');";
    let Some(node) = find_node() else { return };
    let dir = std::env::temp_dir().join(format!("cb-term-{}", crate::util::short_id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("child.js"), script).unwrap();
    let info = hub.spawn_program("cmd", &node, &[dir.join("child.js").to_string_lossy().into_owned()], &dir, 80, 24).unwrap();
    let t = hub.get(&info.id).unwrap();
    assert!(t.stream.wait_exit(Duration::from_secs(90)), "6MiB 를 쏟는 자식이 90초 안에 끝나지 않았다");
    std::thread::sleep(Duration::from_millis(500));
    let after = hub.list().remove(0);
    assert_eq!(after.exit_code, Some(0));
    assert!(after.next_seq >= 6 << 20, "받은 총 바이트 {}", after.next_seq);
    assert!(after.oldest_seq > 0, "고리가 오래된 것을 버리지 않았다");
    let v = view(&hub, &info.id);
    assert_eq!(v.wait_exit(30), Some(0));
    assert!(v.text().ends_with("ALLDONE\r\n") || v.text().contains("ALLDONE"), "마지막 출력이 없다");
    assert_eq!(v.gaps.load(Ordering::Relaxed), 0);
    let _ = fs::remove_dir_all(dir);
}

/// ⑤ kill 은 6초 안에 프로세스를 끝낸다.
#[test]
fn kill_ends_the_process_within_the_grace_period() {
    let hub = Guard(TermHub::default());
    let Some((info, v, dir)) = run_node(&hub, KEEP_ALIVE, 80, 24) else { return };
    v.wait_text("UP", 20);
    let t0 = Instant::now();
    hub.kill(&info.id).unwrap();
    // kill 이 끝난 터미널은 허브에서 바로 지워지므로 보는 쪽에 종료 이벤트가 닿기 전에 정리될 수 있다.
    // 프로세스가 끝났는지는 허브 상태(지워졌거나 종료 코드가 있음)로 본다
    let deadline = Instant::now() + Duration::from_secs(8);
    let code = loop {
        match hub.get(&info.id) {
            Err(_) => break Some(()),
            Ok(t) if t.stream.exit_code().is_some() => break Some(()),
            Ok(_) if Instant::now() >= deadline => break None,
            Ok(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    let took = t0.elapsed();
    assert!(code.is_some() && took < KILL_GRACE, "kill 뒤 {took:?} 에도 끝나지 않았다");
    eprintln!("kill 뒤 종료까지 {took:?}");
    let _ = fs::remove_dir_all(dir);
}

/// ⑥ 파이프 끝은 상속 불가다.
#[test]
fn pipe_ends_are_not_inheritable() {
    use windows_sys::Win32::Foundation::{GetHandleInformation, HANDLE_FLAG_INHERIT};
    let hub = Guard(TermHub::default());
    let Some((info, v, dir)) = run_node(&hub, KEEP_ALIVE, 80, 24) else { return };
    v.wait_text("UP", 20);
    let (input, output) = hub.get(&info.id).unwrap().pty.raw_pipe_handles();
    assert!(input != 0 && output != 0, "파이프 핸들이 비었다");
    for (name, h) in [("입력", input), ("출력", output)] {
        let mut flags = 0u32;
        // SAFETY: 아직 열려 있는 핸들(터미널이 살아 있다)
        let ok = unsafe { GetHandleInformation(h as _, &mut flags) };
        assert!(ok != 0, "{name} 핸들 정보를 못 읽었다: {}", std::io::Error::last_os_error());
        assert_eq!(flags & HANDLE_FLAG_INHERIT, 0, "{name} 파이프 끝이 상속 가능이다");
    }
    let _ = fs::remove_dir_all(dir);
}

/// ⑦ 기본 셸은 이 PC 에서 pwsh 로 풀리고, `exit` 는 종료 코드 0 으로 끝난다.
#[test]
fn default_shell_is_pwsh_and_exit_returns_zero() {
    let hub = Guard(TermHub::default());
    let shell = pty::resolve_shell().expect("셸을 찾지 못했다");
    eprintln!("기본 셸: {shell:?}");
    assert_eq!(shell.kind, "pwsh", "기본 셸이 pwsh 가 아니다: {shell:?}");
    let info = hub.spawn(SpawnSpec { cwd: None, cols: 120, rows: 30 }).unwrap();
    assert_eq!((info.shell.as_str(), info.status, info.cols, info.rows), ("pwsh", "running", 120, 30));
    assert_eq!(info.cwd, paths::home().to_string_lossy());
    assert_eq!(info.windows_build, pty::windows_build());
    let v = view(&hub, &info.id);
    hub.write(&info.id, b"exit\r").unwrap();
    assert_eq!(v.wait_exit(40), Some(0), "출력: {:?}", v.text());
    assert_eq!(hub.list()[0].status, "exited");
}

fn wait_until(what: &str, secs: u64, f: impl Fn() -> bool) {
    let end = Instant::now() + Duration::from_secs(secs);
    while !f() {
        assert!(Instant::now() < end, "{secs}초 안에 \"{what}\" 가 되지 않았다");
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn spawn_cmd_idle(hub: &TermHub) -> TermInfo {
    let cmd = std::env::var_os("ComSpec").map(PathBuf::from).expect("ComSpec");
    hub.spawn_program("cmd", &cmd, &[], &paths::home(), 80, 24).unwrap()
}

/// (A) resize 를 kill 과 겹치고, 자연 종료와 겹쳐 많이 돌려도 죽지 않는다.
#[test]
fn resize_races_kill_and_natural_exit_without_crashing() {
    let hub = Arc::new(Guard(TermHub::default()));
    let cmd = std::env::var_os("ComSpec").map(PathBuf::from).expect("ComSpec");
    for i in 0..20u16 {
        // 자연 종료: 바로 끝나는 cmd
        let info = hub.spawn_program("cmd", &cmd, &["/c".into(), "exit 0".into()], &paths::home(), 80, 24).unwrap();
        // kill: 계속 사는 cmd
        let alive = spawn_cmd_idle(&hub);
        let mut hs = Vec::new();
        for id in [info.id.clone(), alive.id.clone()] {
            let h = Arc::clone(&hub);
            hs.push(std::thread::spawn(move || {
                for k in 0..200u16 {
                    let _ = h.resize(&id, 60 + (k + i) % 40, 20 + k % 10);
                }
            }));
        }
        hub.kill(&alive.id).unwrap();
        for h in hs {
            h.join().unwrap();
        }
        hub.kill(&info.id).ok();
    }
    wait_until("모두 치워짐", 30, || hub.list().is_empty());
}

/// (B) 끝난 터미널에 kill 하면 바로 목록에서 빠진다.
#[test]
fn kill_removes_an_exited_terminal() {
    let hub = Guard(TermHub::default());
    let cmd = std::env::var_os("ComSpec").map(PathBuf::from).expect("ComSpec");
    let info = hub.spawn_program("cmd", &cmd, &["/c".into(), "exit 3".into()], &paths::home(), 80, 24).unwrap();
    wait_until("종료", 20, || hub.list().first().is_some_and(|t| t.status == "exited"));
    // 끝났지만 죽이라고 하지 않았고 보는 쪽도 안 닫혔으니 남는다
    assert_eq!(hub.list().len(), 1);
    hub.kill(&info.id).unwrap();
    assert!(hub.list().is_empty());
    assert!(hub.kill(&info.id).is_err());
}

/// (B) 도는 터미널에 kill 하면 끝난 뒤 목록에서 빠지고 보내기 스레드도 끝난다.
#[test]
fn kill_removes_a_running_terminal_and_ends_its_batcher() {
    let hub = Guard(TermHub::default());
    let info = spawn_cmd_idle(&hub);
    let v = view(&hub, &info.id);
    let stream = Arc::clone(&hub.get(&info.id).unwrap().stream);
    wait_until("배처 시작", 5, || stream.active_batchers() == 1);
    hub.kill(&info.id).unwrap();
    wait_until("목록에서 빠짐", 15, || hub.list().is_empty());
    wait_until("배처 종료", 5, || stream.active_batchers() == 0);
    drop(v);
}

/// (B) 끝난 뒤 보던 웹뷰가 닫히면(받는 쪽이 false) 죽이라고 하지 않아도 치운다.
#[test]
fn exited_terminal_whose_viewer_closed_is_removed() {
    let hub = Guard(TermHub::default());
    let cmd = std::env::var_os("ComSpec").map(PathBuf::from).expect("ComSpec");
    let info = hub.spawn_program("cmd", &cmd, &["/c".into(), "exit 0".into()], &paths::home(), 80, 24).unwrap();
    hub.attach(&info.id, 0, Arc::new(|_| false)).unwrap();
    wait_until("목록에서 빠짐", 20, || hub.list().is_empty());
}
