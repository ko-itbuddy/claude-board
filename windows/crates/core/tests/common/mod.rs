//! Windows 실기 시험 공용 도구 — node.exe 찾기, 임시 폴더, 기다리기, 통과해도 남는 기록.
//! `crates/core/tests/windows_*.rs` 와 `crates/hook/tests/windows_hook.rs`(`#[path]` 로 넣는다)가 함께 쓴다.
#![allow(dead_code)]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// 통과한 시험의 값도 CI 기록에 남긴다 — `eprintln!` 은 시험 틀이 가로채 통과하면 버리므로 stderr 에 바로 쓴다.
pub fn note(msg: &str) {
    let _ = writeln!(std::io::stderr(), "[windows 시험] {msg}");
}

/// node.exe — PATH 에서 먼저 찾고, 없으면 기본 설치 위치. 못 찾으면 CI(환경변수 `CI`)에서는 실패, 그 밖에서는 건너뛴다(None).
pub fn node_or_skip(test: &str) -> Option<PathBuf> {
    let from_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .map(|d| d.join("node.exe"))
        .find(|p| p.is_file());
    let found = from_path.or_else(|| Some(PathBuf::from(r"C:\Program Files\nodejs\node.exe")).filter(|p| p.is_file()));
    if found.is_none() {
        assert!(
            std::env::var_os("CI").is_none(),
            "{test}: CI 인데 node.exe 를 찾지 못했다 (PATH={:?})",
            std::env::var_os("PATH")
        );
        note(&format!("{test}: node.exe 가 없어 건너뛴다"));
    }
    found
}

/// 시험마다 새로 만드는 임시 폴더 — 놓을 때 지운다(끝낸 자식이 파일을 늦게 놓을 수 있어 몇 번 다시 해 본다).
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("cb-win-{tag}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&path).unwrap_or_else(|e| panic!("임시 폴더 {} 를 만들지 못했다: {e}", path.display()));
        TempDir(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        for _ in 0..10 {
            if fs::remove_dir_all(&self.0).is_ok() || !self.0.exists() {
                return;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
        note(&format!("임시 폴더를 지우지 못했다: {}", self.0.display()));
    }
}

/// `f` 가 Some 을 돌려줄 때까지 50ms 마다 다시 본다. `limit` 이 지나면 None.
pub fn wait_for<T>(limit: Duration, mut f: impl FnMut() -> Option<T>) -> Option<T> {
    let start = Instant::now();
    loop {
        if let Some(v) = f() {
            return Some(v);
        }
        if start.elapsed() > limit {
            return None;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// 폴더 아래 파일 목록(상대 경로와 크기) — 실패 메시지용.
pub fn listing(dir: &Path) -> String {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            let rel = p.strip_prefix(root).unwrap_or(&p).display().to_string();
            if p.is_dir() {
                out.push(format!("{rel}\\"));
                walk(root, &p, out);
            } else {
                out.push(format!("{rel} ({}B)", e.metadata().map(|m| m.len()).unwrap_or(0)));
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out.join(", ")
}

/// 파일 내용(없거나 읽지 못하면 "(없음)") — 실패 메시지용.
pub fn read_or_none(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|_| "(없음)".to_string())
}
