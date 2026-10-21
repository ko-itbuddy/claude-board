//! 내장 터미널 백엔드 (SPEC §5.12 터미널 계약) — 터미널 목록과 `term_*` 명령.
//!
//! 터미널마다 스레드 셋이 돈다. 리더는 출력 파이프를 늘 비워 [`Stream`] 고리에 쓰고(웹뷰를 기다리지 않는다),
//! 웨이터는 프로세스가 끝나면 종료 코드를 남기고 가짜 콘솔을 닫는다. 웹뷰로 보내는 배처는 `term_attach` 마다 [`Stream`] 이 띄운다.
//! 명령 등록(`generate_handler!`)과 `TermHub` 상태 등록(`manage`)은 `board.rs` 가 한다.

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use claude_board_core::paths;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::State;

use crate::pty::{self, Pty};
use crate::ring::{Out, Sink, Stream, RING_CAP};

/// `term_kill` 이 가짜 콘솔을 닫은 뒤 프로세스가 끝나길 기다리는 시간. 넘으면 강제 종료한다.
const KILL_GRACE: Duration = Duration::from_secs(6);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnSpec {
    pub cwd: Option<String>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TermInfo {
    pub id: String,
    pub pid: u32,
    /// `pwsh` · `powershell` · `cmd`
    pub shell: String,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    /// `running` · `exited`
    pub status: &'static str,
    pub exit_code: Option<u32>,
    pub windows_build: Option<u32>,
    pub oldest_seq: u64,
    pub next_seq: u64,
}

struct Term {
    id: String,
    shell: String,
    cwd: String,
    size: Mutex<(u16, u16)>,
    pty: Arc<Pty>,
    stream: Arc<Stream>,
}

impl Term {
    fn info(&self) -> TermInfo {
        let (cols, rows) = *lock(&self.size);
        let exit_code = self.stream.exit_code();
        let b = self.stream.bounds();
        TermInfo {
            id: self.id.clone(),
            pid: self.pty.pid(),
            shell: self.shell.clone(),
            cwd: self.cwd.clone(),
            cols,
            rows,
            status: if exit_code.is_some() { "exited" } else { "running" },
            exit_code,
            windows_build: pty::windows_build(),
            oldest_seq: b.oldest,
            next_seq: b.next,
        }
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 터미널 목록. 만든 순서를 지키고 `TermId`("t1", "t2", …)는 프로세스 수명 동안 겹치지 않는다.
#[derive(Default)]
pub struct TermHub {
    terms: Mutex<Vec<Arc<Term>>>,
    next_id: AtomicU64,
}

impl TermHub {
    fn get(&self, id: &str) -> Result<Arc<Term>, String> {
        lock(&self.terms).iter().find(|t| t.id == id).cloned().ok_or_else(|| format!("없는 터미널: {id}"))
    }

    /// 기본 셸(pwsh → powershell → ComSpec)을 `spec` 의 크기로 띄운다. 시작 폴더는 `spec.cwd`, 없으면 홈 폴더.
    pub fn spawn(&self, spec: SpawnSpec) -> Result<TermInfo, String> {
        let shell = pty::resolve_shell().ok_or("셸(pwsh·powershell·cmd)을 찾지 못했다")?;
        let cwd = spec.cwd.map(Into::into).filter(|p: &std::path::PathBuf| p.is_dir()).unwrap_or_else(paths::home);
        self.spawn_program(shell.kind, &shell.path, &shell.args, &cwd, spec.cols, spec.rows)
    }

    /// 프로그램 하나를 가짜 콘솔에 띄우고 리더·웨이터를 돌린다.
    fn spawn_program(&self, shell: &str, program: &Path, args: &[String], cwd: &Path, cols: u16, rows: u16) -> Result<TermInfo, String> {
        let (cols, rows) = (cols.max(2), rows.max(1));
        let pty = Arc::new(Pty::spawn(program, args, cwd, cols, rows).map_err(|e| format!("{} 을(를) 띄우지 못했다: {e}", program.display()))?);
        let stream = Arc::new(Stream::new(RING_CAP));
        let id = format!("t{}", self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        spawn_reader(&id, &pty, &stream);
        spawn_waiter(&id, &pty, &stream);
        let term = Arc::new(Term {
            id,
            shell: shell.to_string(),
            cwd: cwd.to_string_lossy().into_owned(),
            size: Mutex::new((cols, rows)),
            pty,
            stream,
        });
        let info = term.info();
        lock(&self.terms).push(term);
        Ok(info)
    }

    pub fn list(&self) -> Vec<TermInfo> {
        lock(&self.terms).iter().map(|t| t.info()).collect()
    }

    /// 보는 쪽을 `sink` 로 바꾸고 `from_seq`(0 이면 고리에 남은 처음)부터 보낸다.
    pub fn attach(&self, id: &str, from_seq: u64, sink: Sink) -> Result<TermInfo, String> {
        let t = self.get(id)?;
        t.stream.attach(from_seq, sink);
        Ok(t.info())
    }

    pub fn ack(&self, id: &str, seq: u64) -> Result<(), String> {
        self.get(id)?.stream.ack(seq);
        Ok(())
    }

    pub fn write(&self, id: &str, data: &[u8]) -> Result<(), String> {
        self.get(id)?.pty.write_all(data).map_err(|e| format!("터미널 입력 실패: {e}"))
    }

    /// 열이 2 미만이거나 행이 1 미만이거나 지금과 같은 크기면 무시한다.
    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let t = self.get(id)?;
        if cols < 2 || rows < 1 || t.stream.exit_code().is_some() {
            return Ok(());
        }
        let mut size = lock(&t.size);
        if *size == (cols, rows) {
            return Ok(());
        }
        t.pty.resize(cols, rows).map_err(|e| format!("터미널 크기 바꾸기 실패: {e}"))?;
        *size = (cols, rows);
        Ok(())
    }

    /// 가짜 콘솔을 별도 스레드에서 닫는다. 6초 안에 프로세스가 안 끝나면 강제로 끝낸다. 기다리지 않고 돌아온다.
    pub fn kill(&self, id: &str) -> Result<(), String> {
        let t = self.get(id)?;
        if t.stream.exit_code().is_some() {
            return Ok(());
        }
        std::thread::spawn(move || {
            let closer = Arc::clone(&t.pty);
            std::thread::spawn(move || closer.close_console());
            if !t.stream.wait_exit(KILL_GRACE) {
                t.pty.kill();
            }
        });
        Ok(())
    }

    /// 앱을 끝낼 때: 모든 터미널의 프로세스를 바로 끝내고 가짜 콘솔을 닫는다. 기다리지 않는다.
    pub fn kill_all(&self) {
        for t in lock(&self.terms).iter().cloned() {
            t.pty.kill();
            std::thread::spawn(move || t.pty.close_console());
        }
    }
}

/// 출력 파이프를 끝까지 비워 고리에 쓴다. 끝(EOF)에 닿으면 `Exit` 이 나갈 수 있게 표시한다.
fn spawn_reader(id: &str, pty: &Arc<Pty>, stream: &Arc<Stream>) {
    let (pty, stream) = (Arc::clone(pty), Arc::clone(stream));
    std::thread::Builder::new()
        .name(format!("term-read-{id}"))
        .spawn(move || {
            let mut buf = vec![0u8; 64 << 10];
            while let Ok(n @ 1..) = pty.read(&mut buf) {
                stream.push(&buf[..n]);
            }
            pty.close_output();
            stream.finish_output();
        })
        .expect("리더 스레드를 만들지 못했다");
}

/// 프로세스가 끝나길 기다려 종료 코드를 남기고, 가짜 콘솔을 별도 스레드에서 닫는다
/// (자손이 붙어 있으면 콘솔을 닫아야 출력 파이프가 끊긴다. 닫기가 오래 걸려도 리더는 계속 비운다).
fn spawn_waiter(id: &str, pty: &Arc<Pty>, stream: &Arc<Stream>) {
    let (pty, stream) = (Arc::clone(pty), Arc::clone(stream));
    std::thread::Builder::new()
        .name(format!("term-wait-{id}"))
        .spawn(move || {
            let code = pty.wait_exit();
            stream.set_exit(code);
            let closer = Arc::clone(&pty);
            std::thread::spawn(move || closer.close_console());
            pty.release();
        })
        .expect("웨이터 스레드를 만들지 못했다");
}

/// Tauri 채널을 배처의 받는 쪽으로: 원시 프레임은 그대로, 제어(`gap`·`exit`)는 JSON.
fn channel_sink(ch: Channel<InvokeResponseBody>) -> Sink {
    Arc::new(move |out| {
        let body = match out {
            Out::Data(frame) => InvokeResponseBody::Raw(frame),
            Out::Gap(seq) => InvokeResponseBody::Json(json!({ "t": "gap", "seq": seq }).to_string()),
            Out::Exit(code) => InvokeResponseBody::Json(json!({ "t": "exit", "code": code }).to_string()),
        };
        ch.send(body).is_ok()
    })
}

type Hub<'a> = State<'a, TermHub>;

#[tauri::command(async)]
pub fn term_spawn(hub: Hub<'_>, spec: SpawnSpec) -> Result<TermInfo, String> {
    hub.spawn(spec)
}

#[tauri::command(async)]
pub fn term_list(hub: Hub<'_>) -> Vec<TermInfo> {
    hub.list()
}

#[tauri::command(async)]
pub fn term_attach(hub: Hub<'_>, id: String, from_seq: u64, on_data: Channel<InvokeResponseBody>) -> Result<TermInfo, String> {
    hub.attach(&id, from_seq, channel_sink(on_data))
}

#[tauri::command(async)]
pub fn term_ack(hub: Hub<'_>, id: String, seq: u64) -> Result<(), String> {
    hub.ack(&id, seq)
}

#[tauri::command(async)]
pub fn term_write(hub: Hub<'_>, id: String, data: String) -> Result<(), String> {
    hub.write(&id, data.as_bytes())
}

#[tauri::command(async)]
pub fn term_write_bin(hub: Hub<'_>, id: String, data: Vec<u8>) -> Result<(), String> {
    hub.write(&id, &data)
}

#[tauri::command(async)]
pub fn term_resize(hub: Hub<'_>, id: String, cols: u16, rows: u16) -> Result<(), String> {
    hub.resize(&id, cols, rows)
}

#[tauri::command(async)]
pub fn term_kill(hub: Hub<'_>, id: String) -> Result<(), String> {
    hub.kill(&id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_id_is_an_error_not_a_panic() {
        let hub = TermHub::default();
        assert!(hub.ack("t9", 0).is_err());
        assert!(hub.write("t9", b"x").is_err());
        assert!(hub.resize("t9", 80, 24).is_err());
        assert!(hub.kill("t9").is_err());
        assert!(hub.list().is_empty());
    }

    #[test]
    fn term_info_serializes_with_camel_case_keys() {
        let info = TermInfo {
            id: "t1".into(),
            pid: 4,
            shell: "pwsh".into(),
            cwd: "C:\\".into(),
            cols: 80,
            rows: 24,
            status: "exited",
            exit_code: Some(3),
            windows_build: None,
            oldest_seq: 0,
            next_seq: 9,
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(
            v,
            json!({"id":"t1","pid":4,"shell":"pwsh","cwd":"C:\\","cols":80,"rows":24,"status":"exited","exitCode":3,"windowsBuild":null,"oldestSeq":0,"nextSeq":9})
        );
        let spec: SpawnSpec = serde_json::from_value(json!({"cols": 100, "rows": 40})).unwrap();
        assert_eq!((spec.cwd, spec.cols, spec.rows), (None, 100, 40));
    }

    #[cfg(not(windows))]
    #[test]
    fn spawn_is_unsupported_off_windows() {
        let err = TermHub::default().spawn(SpawnSpec { cwd: None, cols: 80, rows: 24 }).unwrap_err();
        assert!(!err.is_empty());
    }
}

/// 실제 Windows 에서만 — ConPTY 로 진짜 프로세스를 띄워 확인한다.
#[cfg(all(test, windows))]
mod real {
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
        let code = v.wait_exit(8);
        let took = t0.elapsed();
        assert!(code.is_some() && took < KILL_GRACE, "kill 뒤 {took:?} 에도 끝나지 않았다(종료 코드 {code:?})");
        eprintln!("kill 뒤 종료까지 {took:?}, 종료 코드 {code:?}");
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
}
