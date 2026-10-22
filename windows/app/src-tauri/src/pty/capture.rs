//! D1 재현용 캡처 — 출력이 쏟아지는 중에 ConPTY 크기를 바꾸면 ConPTY 가 무엇을 내보내는지 원시 바이트로 남긴다.
//!
//! 시험이 아니라 측정 도구라 `#[ignore]` 다. 실행:
//! `D1_OUT=<폴더> cargo test --lib pty::capture -- --ignored --test-threads=1 --nocapture`
//! 환경변수: `D1_OUT`(결과 폴더, 기본 임시 폴더) · `D1_FLAGS`(CreatePseudoConsole 플래그 목록, 기본 `0,2`) ·
//! `D1_RESIZE_MS`(첫 바이트 뒤 몇 ms 에 줄이나, 쉼표 목록. 없으면 500 한 번) · `D1_FROM`/`D1_TO`(기본 120x30 → 80x30).
//! 산출물: `d1-cap-flags<플래그>[-r<ms>].bin`(받은 바이트 전부) + `.json`(크기 바꾼 순간의 바이트 위치·덩어리 시각·열/행).
//! `D1_RESIZE_MS` 가 없을 때는 대조군 `d1-cap-ctl80.bin`(처음부터 80×30, 크기 안 바꿈)도 같이 만든다.

use super::conpty::{ConPty, ConptyApi, SYSTEM_API};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows_sys::core::HRESULT;
use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Console::{CreatePseudoConsole, COORD, HPCON};

/// 이 시험이 `CreatePseudoConsole` 에 넘길 플래그. 함수 표는 값을 못 싣고 시험은 한 줄로 도니 정적 값 하나로 건넨다.
static FLAGS: AtomicU32 = AtomicU32::new(0);

unsafe extern "system" fn create_flagged(size: COORD, i: HANDLE, o: HANDLE, _ignored: u32, out: *mut HPCON) -> HRESULT {
    CreatePseudoConsole(size, i, o, FLAGS.load(Ordering::SeqCst), out)
}

static FLAGGED_API: ConptyApi = ConptyApi { create: create_flagged, resize: SYSTEM_API.resize, close: SYSTEM_API.close };

/// 줄 번호가 든 `line <n> <x 96개>` 를 50줄씩 묶어 빠르게 쏟는다(1000줄마다 잠깐 쉼). 끝에 `DONE`.
const SCRIPT: &str = r#"
let n = 0; const x = 'x'.repeat(96);
function burst() {
  let s = '';
  for (let i = 0; i < 50 && n < 6000; i++) { n++; s += 'line ' + n + ' ' + x + '\r\n'; }
  process.stdout.write(s);
  if (n >= 6000) { process.stdout.write('DONE\r\n'); setTimeout(() => process.exit(0), 400); return; }
  setTimeout(burst, n % 1000 === 0 ? 60 : 1);
}
burst();
"#;

fn find_node() -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    dirs.into_iter().map(|d| d.join("node.exe")).find(|p| p.is_file())
}

#[derive(Default)]
struct Shared {
    bytes: Vec<u8>,
    /// (첫 바이트부터 마이크로초, 덩어리 길이, 읽은 뒤 누적 바이트)
    chunks: Vec<(u64, usize, usize)>,
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

/// 한 번 돌려서 (바이트, json) 을 돌려준다. `resize_to` 가 None 이면 크기를 안 바꾼다(대조군).
fn capture(flags: u32, from: (u16, u16), resize_to: Option<(u16, u16)>, resize_ms: u64) -> (Vec<u8>, serde_json::Value) {
    let node = find_node().expect("PATH 에 node.exe 가 없다");
    let dir = std::env::temp_dir().join(format!("cb-d1-{}", crate::util::short_id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("burst.js");
    std::fs::write(&file, SCRIPT).unwrap();
    FLAGS.store(flags, Ordering::SeqCst);
    let pty = Arc::new(ConPty::spawn_with(&FLAGGED_API, &node, &[file.to_string_lossy().into_owned()], &dir, from.0, from.1).unwrap());
    let sh = Arc::new(Mutex::new(Shared::default()));
    let t0 = Arc::new(Mutex::new(None::<Instant>));
    let reader = {
        let (pty, sh, t0) = (Arc::clone(&pty), Arc::clone(&sh), Arc::clone(&t0));
        std::thread::spawn(move || {
            let mut buf = vec![0u8; 64 << 10];
            while let Ok(n @ 1..) = pty.read(&mut buf) {
                let now = Instant::now();
                let start = *t0.lock().unwrap().get_or_insert(now);
                let mut g = sh.lock().unwrap();
                g.bytes.extend_from_slice(&buf[..n]);
                let total = g.bytes.len();
                g.chunks.push((now.duration_since(start).as_micros() as u64, n, total));
            }
        })
    };
    let spawned = Instant::now();
    // 첫 바이트가 올 때까지 기다린 뒤 resize_ms 를 센다
    let wait_end = Instant::now() + Duration::from_secs(20);
    let first = loop {
        if let Some(t) = *t0.lock().unwrap() {
            break t;
        }
        assert!(Instant::now() < wait_end, "20초 안에 첫 출력이 없다");
        std::thread::sleep(Duration::from_millis(2));
    };
    let mut resize_info = serde_json::Value::Null;
    if let Some((rc, rr)) = resize_to {
        let at = first + Duration::from_millis(resize_ms);
        while Instant::now() < at {
            std::thread::sleep(Duration::from_micros(200));
        }
        let before = sh.lock().unwrap().bytes.len();
        let a = Instant::now();
        pty.resize(rc, rr).expect("ResizePseudoConsole");
        let done = Instant::now();
        let after = sh.lock().unwrap().bytes.len();
        resize_info = json!({
            "toCols": rc, "toRows": rr,
            "callStartUs": a.duration_since(first).as_micros() as u64,
            "callReturnUs": done.duration_since(first).as_micros() as u64,
            "bytesReadBeforeCall": before, "bytesReadAfterReturn": after,
        });
    }
    // DONE 가 오면 조금 더 기다렸다가 끝낸다
    let end = Instant::now() + Duration::from_secs(60);
    while !contains(&sh.lock().unwrap().bytes, b"DONE\r\n") {
        assert!(Instant::now() < end, "60초 안에 DONE 이 없다");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(500));
    let p2 = Arc::clone(&pty);
    let closer = std::thread::spawn(move || p2.close_console());
    let _ = closer.join();
    let _ = reader.join();
    pty.release();
    let g = sh.lock().unwrap();
    let info = json!({
        "flags": flags, "colsBefore": from.0, "rowsBefore": from.1, "resize": resize_info,
        "firstByteAfterSpawnMs": first.duration_since(spawned).as_millis() as u64,
        "totalBytes": g.bytes.len(),
        "chunks": g.chunks.iter().map(|c| json!([c.0, c.1, c.2])).collect::<Vec<_>>(),
        "chunkFields": ["usSinceFirstByte", "len", "cumulativeBytes"],
        "windowsBuild": super::windows_build(),
    });
    let _ = std::fs::remove_dir_all(&dir);
    (g.bytes.clone(), info)
}

fn save(out: &Path, name: &str, bytes: &[u8], info: &serde_json::Value) {
    std::fs::create_dir_all(out).unwrap();
    std::fs::write(out.join(format!("{name}.bin")), bytes).unwrap();
    std::fs::write(out.join(format!("{name}.json")), serde_json::to_string_pretty(info).unwrap()).unwrap();
    eprintln!("{name}: {} 바이트, 크기 바꾼 순간 {}", bytes.len(), info["resize"]);
}

/// 120×30 으로 시작해 첫 바이트 뒤 `D1_RESIZE_MS` 에 80×30 으로 줄이고 받은 바이트를 파일로 남긴다.
#[test]
#[ignore = "측정 도구: D1_OUT 에 캡처를 쓴다"]
fn d1_capture_resize_during_burst() {
    let out = std::env::var_os("D1_OUT").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    let times = std::env::var("D1_RESIZE_MS");
    let flags = std::env::var("D1_FLAGS").unwrap_or_else(|_| "0,2".into());
    // D1_FROM / D1_TO: `<열>x<행>`. 기본 120x30 → 80x30. 바꾸면 파일 이름에 `-<from>to<to>` 가 붙는다
    let size = |key: &str, default: (u16, u16)| -> (u16, u16) {
        std::env::var(key)
            .ok()
            .and_then(|v| v.split_once('x').and_then(|(c, r)| Some((c.trim().parse().ok()?, r.trim().parse().ok()?))))
            .unwrap_or(default)
    };
    let (from, to) = (size("D1_FROM", (120, 30)), size("D1_TO", (80, 30)));
    let shape = if (from, to) == ((120, 30), (80, 30)) { String::new() } else { format!("-{}x{}to{}x{}", from.0, from.1, to.0, to.1) };
    // D1_RESIZE_MS 가 없으면 500ms 한 번(이름에 시각 없음) + 대조군. 있으면 쉼표 목록마다 `-r<ms>` 이름으로
    let list: Vec<(u64, String)> = match &times {
        Ok(v) => v.split(',').filter_map(|s| s.trim().parse::<u64>().ok()).map(|ms| (ms, format!("-r{ms}"))).collect(),
        Err(_) => vec![(500, String::new())],
    };
    for (ms, suffix) in &list {
        for f in flags.split(',').filter_map(|s| s.trim().parse::<u32>().ok()) {
            let (bytes, info) = capture(f, from, Some(to), *ms);
            save(&out, &format!("d1-cap-flags{f}{shape}{suffix}"), &bytes, &info);
        }
    }
    if times.is_err() {
        let (bytes, info) = capture(0, to, None, 0);
        let name = if to == (80, 30) { "d1-cap-ctl80".to_string() } else { format!("d1-cap-ctl{}x{}", to.0, to.1) };
        save(&out, &name, &bytes, &info);
    }
}
