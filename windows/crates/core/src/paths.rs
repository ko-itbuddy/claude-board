//! 경로 — `%USERPROFILE%\.claude\notifier\` 아래 파일 약속 (SPEC §1.2, §2).
//!
//! 시험·개발용으로 환경변수로 바꿀 수 있다.
//! - `CLAUDE_BOARD_HOME`: 홈 폴더(`~`) 대신 쓸 폴더 — `.claude`·`.claude.json`·`.local\bin` 이 모두 이 아래로 간다.
//! - `CLAUDE_BOARD_NOTIFIER_DIR`: notifier 폴더만 바꾼다. hook 표시(`.claude/notifier/`)가 경로에 없으면
//!   `install-hooks` 가 넣은 hook 을 나중에 알아보지 못하므로, hook 등록까지 시험할 때는 `CLAUDE_BOARD_HOME` 을 쓴다.

use std::env;
use std::path::PathBuf;

pub const HOME_ENV: &str = "CLAUDE_BOARD_HOME";
pub const NOTIFIER_ENV: &str = "CLAUDE_BOARD_NOTIFIER_DIR";

/// hook 이 실행하는 사본의 파일 이름(Windows 는 `.exe`).
pub const HOOK_EXE: &str = if cfg!(windows) { "claude-board-hook.exe" } else { "claude-board-hook" };

fn env_path(key: &str) -> Option<PathBuf> {
    env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// 홈 폴더 — `CLAUDE_BOARD_HOME` > Windows `USERPROFILE` / 그 밖 `HOME`.
pub fn home() -> PathBuf {
    if let Some(p) = env_path(HOME_ENV) {
        return p;
    }
    let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env_path(key).unwrap_or_else(|| PathBuf::from("."))
}

/// 환경변수로 홈이나 notifier 폴더를 바꿨는지 — macOS 개발 기기에서 실제 ~/.claude 를 지키는 데 쓴다.
pub fn overridden() -> bool {
    env_path(HOME_ENV).is_some() || env_path(NOTIFIER_ENV).is_some()
}

/// `~/.claude`
pub fn claude_dir() -> PathBuf {
    home().join(".claude")
}

/// `~/.claude/notifier` (또는 `CLAUDE_BOARD_NOTIFIER_DIR`)
pub fn notifier_dir() -> PathBuf {
    env_path(NOTIFIER_ENV).unwrap_or_else(|| claude_dir().join("notifier"))
}

/// `notifier/sessions` — 세션 기록과 `.lock`·`.paused`·`.mute` 표지
pub fn sessions_dir() -> PathBuf {
    notifier_dir().join("sessions")
}

/// `~/.claude/settings.json` — hook 을 등록하는 전역 설정
pub fn settings_file() -> PathBuf {
    claude_dir().join("settings.json")
}

/// `~/.claude/.credentials.json` — 지금 Claude Code 로그인
pub fn credentials_file() -> PathBuf {
    claude_dir().join(".credentials.json")
}

/// `~/.claude.json` — `oauthAccount` 등 Claude Code 계정 정보
pub fn claude_json() -> PathBuf {
    home().join(".claude.json")
}

/// `notifier/claude-board-hook(.exe)` — settings.json 의 hook 이 실행하는 사본
pub fn hook_exe() -> PathBuf {
    notifier_dir().join(HOOK_EXE)
}

/// `notifier/app-path` — ClaudeBoard.exe 전체 경로 한 줄(setup 이 쓰고 notify 가 읽는다)
pub fn app_path_file() -> PathBuf {
    notifier_dir().join("app-path")
}

/// `notifier/mute-all` — 전체 알림 끔 표지
pub fn mute_all_file() -> PathBuf {
    notifier_dir().join("mute-all")
}

/// `notifier/notify-position` — 알림 위치(없으면 가운데)
pub fn notify_position_file() -> PathBuf {
    notifier_dir().join("notify-position")
}

/// `notifier/board-theme` — TUI 색 테마 이름
pub fn board_theme_file() -> PathBuf {
    notifier_dir().join("board-theme")
}

/// `~/.local/bin` — `claude-board.cmd`·`claude-board-tui.cmd` 를 두는 곳(Claude Code 네이티브 설치가 PATH 에 넣는다)
pub fn local_bin() -> PathBuf {
    home().join(".local").join("bin")
}

/// 경로의 마지막 이름 — Python `os.path.basename` 처럼 끝이 구분자면 빈 문자열.
/// Windows 는 `\`·`/` 를 모두 구분자로 본다(SPEC §2.1 `project`).
pub fn basename(path: &str) -> &str {
    let seps: &[char] = if cfg!(windows) { &['/', '\\'] } else { &['/'] };
    path.rsplit(seps).next().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basename_like_python() {
        assert_eq!(basename("/tmp/demo-proj"), "demo-proj");
        assert_eq!(basename("/tmp/demo-proj/"), "");
        assert_eq!(basename("demo"), "demo");
        if cfg!(windows) {
            assert_eq!(basename(r"C:\work\proj"), "proj");
        }
    }
}
