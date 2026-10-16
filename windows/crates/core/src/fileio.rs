//! 파일 읽고 쓰기 — 원자적 바꿔 넣기와 JSON 객체 읽기 (SPEC §2.1 쓰기 규칙).

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use crate::Record;

/// `tmp` 를 `dst` 자리에 바꿔 넣는다. Windows 는 `MoveFileExW(REPLACE_EXISTING)` 이고(std `rename`),
/// 다른 프로세스가 잠깐 열어 두어 공유 위반이 나면 50ms 간격으로 5번 다시 시도한다.
pub fn replace(tmp: &Path, dst: &Path) -> io::Result<()> {
    let tries = if cfg!(windows) { 6 } else { 1 };
    let mut last = None;
    for i in 0..tries {
        match fs::rename(tmp, dst) {
            Ok(()) => return Ok(()),
            Err(e) => last = Some(e),
        }
        if i + 1 < tries {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("rename failed")))
}

/// 같은 폴더의 임시 파일 이름 — 동시에 도는 hook 끼리 겹치지 않게 프로세스·시각·순번을 섞는다.
pub fn temp_path(dir: &Path, prefix: &str) -> PathBuf {
    static SEQ: AtomicU32 = AtomicU32::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    dir.join(format!("{prefix}.{:x}{:08x}{n:x}.tmp", std::process::id(), nanos))
}

/// `dst` 와 같은 폴더에 `<prefix>.<임의>.tmp` 로 쓴 뒤 바꿔 넣는다. 실패하면 임시 파일을 지운다.
pub fn write_atomic(dst: &Path, prefix: &str, bytes: &[u8]) -> io::Result<()> {
    let dir = dst.parent().unwrap_or_else(|| Path::new("."));
    let tmp = temp_path(dir, prefix);
    let result = (|| {
        let mut f = File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all().ok();
        drop(f);
        replace(&tmp, dst)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// JSON 객체 파일을 읽는다. 없거나 깨졌거나 객체가 아니면 None.
pub fn read_record(path: &Path) -> Option<Record> {
    let bytes = fs::read(path).ok()?;
    match serde_json::from_slice(&bytes).ok()? {
        serde_json::Value::Object(m) => Some(m),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces() {
        let dir = std::env::temp_dir().join(format!("cb-fileio-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let dst = dir.join("a.json");
        write_atomic(&dst, "a", b"{\"x\":1}").unwrap();
        write_atomic(&dst, "a", b"{\"x\":2}").unwrap();
        assert_eq!(read_record(&dst).unwrap()["x"], 2);
        // 임시 파일이 남지 않는다
        let left: Vec<_> = fs::read_dir(&dir).unwrap().flatten().collect();
        assert_eq!(left.len(), 1);
        fs::write(&dst, b"[1]").unwrap();
        assert!(read_record(&dst).is_none());
        fs::remove_dir_all(&dir).unwrap();
    }
}
