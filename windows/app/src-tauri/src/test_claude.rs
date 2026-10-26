//! 시험용 claude 바꿔치기 — 환경변수 `CLAUDE_BOARD_CLAUDE`(`|` 로 이은 절대 경로: 프로그램과 앞쪽 인자)가 있고
//! `CLAUDE_BOARD_HOME` 도 있을 때만 보드가 치는 명령의 맨 앞 `claude` 를 그 값으로 바꾼다. 진짜 claude 를 띄우지 않고 명령 길을 시험하려는 장치다.
//!
//! 규칙: 두 변수가 모두 있고, `|` 로 가른 조각이 모두 비지 않은 절대 경로이며 실재할 때만 켜진다.
//! 하나라도 어긋나면 꺼진 채(None)로 두고 board.log 에 한 줄 남긴다 — 값이 없으면(`CLAUDE_BOARD_CLAUDE` 미설정) 아무것도 남기지 않는다.
//! 홈을 따로 정한 실행에서만 켜지게 해서, 사용자 보드에 남은 환경변수 하나가 실수로 진짜 claude 를 바꿔치지 못하게 한다.
//! 바꾸는 쪽은 프론트(`terminal/testClaude.ts`)다. 보드가 직접 띄우는 곳(계정 추가 로그인 등)은 `prefix_from_process()` 를 쓴다.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::util::board_log;

pub const CLAUDE_ENV: &str = "CLAUDE_BOARD_CLAUDE";
pub const HOME_ENV: &str = claude_board_core::paths::HOME_ENV;

/// `prefix` 의 판정 — 꺼짐 / 켜짐 / 켜려 했으나 어긋남(이유)
#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Off,
    On(Vec<PathBuf>),
    Rejected(String),
}

fn non_empty(env: &dyn Fn(&str) -> Option<OsString>, key: &str) -> Option<OsString> {
    env(key).filter(|v| !v.is_empty())
}

/// 환경(`env`)과 파일 확인(`exists`)만으로 정하는 순수 판정 — 시험은 가짜 환경으로 이것을 부른다.
pub fn decide(env: &dyn Fn(&str) -> Option<OsString>, exists: &dyn Fn(&Path) -> bool) -> Decision {
    let Some(raw) = non_empty(env, CLAUDE_ENV) else { return Decision::Off };
    if non_empty(env, HOME_ENV).is_none() {
        return Decision::Rejected(format!("{CLAUDE_ENV} 는 {HOME_ENV} 도 있을 때만 쓴다 — 무시한다"));
    }
    let Some(raw) = raw.to_str() else { return Decision::Rejected(format!("{CLAUDE_ENV} 가 유니코드가 아니다 — 무시한다")) };
    let mut parts = Vec::new();
    for piece in raw.split('|') {
        if piece.is_empty() {
            return Decision::Rejected(format!("{CLAUDE_ENV} 에 빈 조각이 있다 — 무시한다"));
        }
        if piece.contains('"') {
            return Decision::Rejected(format!("{CLAUDE_ENV} 조각에 큰따옴표가 있다({piece}) — 무시한다"));
        }
        let path = PathBuf::from(piece);
        if !path.is_absolute() {
            return Decision::Rejected(format!("{CLAUDE_ENV} 조각이 절대 경로가 아니다({piece}) — 무시한다"));
        }
        if !exists(&path) {
            return Decision::Rejected(format!("{CLAUDE_ENV} 조각이 없는 파일이다({piece}) — 무시한다"));
        }
        parts.push(path);
    }
    Decision::On(parts)
}

/// 주어진 환경에서의 바꿔칠 값. 어긋나면 None 과 board.log 한 줄.
pub fn prefix(env: &dyn Fn(&str) -> Option<OsString>) -> Option<Vec<PathBuf>> {
    match decide(env, &|p| p.exists()) {
        Decision::Off => None,
        Decision::On(parts) => Some(parts),
        Decision::Rejected(why) => {
            board_log(&format!("[시험 claude] {why}"));
            None
        }
    }
}

/// 이 프로세스의 환경에서의 바꿔칠 값 — 보드가 claude 를 직접 띄우는 곳이 쓴다.
pub fn prefix_from_process() -> Option<Vec<PathBuf>> {
    prefix(&|key| std::env::var_os(key))
}

/// 프론트의 `testClaudePrefix()` — 바꿔칠 프로그램·앞쪽 인자. 시험 모드가 아니면 None.
#[tauri::command(async)]
pub fn test_claude_prefix() -> Option<Vec<String>> {
    prefix_from_process().map(|parts| parts.iter().map(|p| p.to_string_lossy().into_owned()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;

    fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let map: HashMap<String, OsString> = pairs.iter().map(|(k, v)| (k.to_string(), OsString::from(v))).collect();
        move |key| map.get(key).cloned()
    }

    fn rejected(d: &Decision) -> &str {
        match d {
            Decision::Rejected(why) => why,
            other => panic!("거절이어야 한다: {other:?}"),
        }
    }

    // 실재하는 절대 경로 둘 — 임시 폴더 안 파일
    fn fixture() -> (PathBuf, PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("cb-test-claude-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("claude.exe");
        let script = dir.join("fake-claude-tty.cjs");
        fs::write(&exe, b"").unwrap();
        fs::write(&script, b"").unwrap();
        (dir, exe, script)
    }

    fn always(_: &Path) -> bool {
        true
    }

    #[test]
    fn unset_is_off() {
        assert_eq!(decide(&env_of(&[]), &always), Decision::Off);
        assert_eq!(decide(&env_of(&[(HOME_ENV, "C:\\h")]), &always), Decision::Off);
        assert_eq!(decide(&env_of(&[(CLAUDE_ENV, ""), (HOME_ENV, "C:\\h")]), &always), Decision::Off);
    }

    #[test]
    fn inactive_unless_both_vars_are_set() {
        let (_dir, exe, _) = fixture();
        let claude = exe.to_str().unwrap();
        let only_claude = decide(&env_of(&[(CLAUDE_ENV, claude)]), &always);
        assert!(rejected(&only_claude).contains(HOME_ENV));
        let empty_home = decide(&env_of(&[(CLAUDE_ENV, claude), (HOME_ENV, "")]), &always);
        assert!(rejected(&empty_home).contains(HOME_ENV));
        assert_eq!(prefix(&env_of(&[(CLAUDE_ENV, claude)])), None);
    }

    #[test]
    fn relative_part_is_rejected() {
        let (_dir, exe, _) = fixture();
        let abs = exe.to_str().unwrap();
        for bad in ["claude.exe", "fake\\claude.exe", ".\\claude.exe", "..\\claude.exe", "\\claude.exe", "C:claude.exe"] {
            let d = decide(&env_of(&[(CLAUDE_ENV, bad), (HOME_ENV, "C:\\h")]), &always);
            assert!(rejected(&d).contains("절대 경로"), "{bad}: {d:?}");
            // 앞쪽 인자 자리의 상대 경로도 거절 — 하나라도 어긋나면 통째로 꺼진다
            let joined = format!("{abs}|{bad}");
            let d = decide(&env_of(&[(CLAUDE_ENV, joined.as_str()), (HOME_ENV, "C:\\h")]), &always);
            assert!(rejected(&d).contains("절대 경로"), "{joined}: {d:?}");
        }
    }

    #[test]
    fn missing_file_and_empty_piece_and_quote_are_rejected() {
        let (_dir, exe, _) = fixture();
        let abs = exe.to_str().unwrap();
        let ghost = exe.with_file_name("no-such-file.exe");
        let ghost = ghost.to_str().unwrap();
        let home = (HOME_ENV, "C:\\h");
        let missing = decide(&env_of(&[(CLAUDE_ENV, ghost), home]), &|p| p.exists());
        assert!(rejected(&missing).contains("없는 파일"));
        let second_missing = format!("{abs}|{ghost}");
        let missing = decide(&env_of(&[(CLAUDE_ENV, second_missing.as_str()), home]), &|p| p.exists());
        assert!(rejected(&missing).contains("없는 파일"));
        for bad in [format!("{abs}|"), format!("|{abs}"), format!("{abs}||{abs}")] {
            let d = decide(&env_of(&[(CLAUDE_ENV, bad.as_str()), home]), &always);
            assert!(rejected(&d).contains("빈 조각"), "{bad}: {d:?}");
        }
        let quoted = format!("\"{abs}\"");
        let d = decide(&env_of(&[(CLAUDE_ENV, quoted.as_str()), home]), &always);
        assert!(rejected(&d).contains("큰따옴표"));
    }

    #[test]
    fn valid_values_keep_order_and_spaces() {
        let (dir, exe, script) = fixture();
        let spaced = dir.join("홍 길동");
        fs::create_dir_all(&spaced).unwrap();
        let tool = spaced.join("claude.exe");
        fs::write(&tool, b"").unwrap();
        let joined = format!("{}|{}", exe.display(), script.display());
        let env = env_of(&[(CLAUDE_ENV, joined.as_str()), (HOME_ENV, "C:\\h")]);
        assert_eq!(decide(&env, &|p| p.exists()), Decision::On(vec![exe.clone(), script.clone()]));
        assert_eq!(prefix(&env), Some(vec![exe, script]));
        let one = tool.to_str().unwrap();
        assert_eq!(prefix(&env_of(&[(CLAUDE_ENV, one), (HOME_ENV, "C:\\h")])), Some(vec![tool]));
    }

    #[test]
    fn does_not_touch_the_process_environment_without_the_vars() {
        // 이 프로세스에는 두 변수가 없어야 한다 — 있으면 다른 시험이 샌 것이다
        assert!(std::env::var_os(CLAUDE_ENV).is_none());
        assert_eq!(test_claude_prefix(), None);
    }
}
