//! 로그인할 때 열기(설정 > 일반, mac `LoginItem`) — 설치가 등록하는 시작 프로그램(HKCU Run)을 켜고 끈다.
//!
//! 끄면 등록을 지우고 표지 파일 `login-launch-off`(`claude_board_core::paths::login_launch_off_file`)를 남겨 업데이트의 `setup` 이 다시
//! 등록하지 않게 한다. 지금 도는 보드는 그대로 둔다. 켜면 표지를 지우고 등록을 다시 쓴다.
//!
//! TODO(P2.2): `set_login_launch` 본문 — Run 값 쓰기·지우기(`crates/hook/src/setup.rs set_run`·`delete_run`)와 표지 파일.
//! 개발 식별자(`CLAUDE_BOARD_HOME` 로 띄운 보드)는 설치된 보드의 Run 값을 건드리지 않는다.

use std::path::Path;

use claude_board_core::paths;

/// 표지 파일이 없으면 켠 것(처음 설치는 켠 채로 시작한다).
fn enabled_at(off_mark: &Path) -> bool {
    !off_mark.exists()
}

/// 프론트의 `loginLaunch()` — 지금 로그인할 때 열기가 켜져 있는가.
#[tauri::command(async)]
pub fn login_launch() -> bool {
    enabled_at(&paths::login_launch_off_file())
}

/// 프론트의 `setLoginLaunch(on)`.
#[tauri::command(async)]
pub fn set_login_launch(on: bool) {
    // TODO(P2.2): 본문
    let _ = on;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_unless_the_off_mark_exists() {
        let dir = std::env::temp_dir().join(format!("cb-login-item-{}", crate::util::short_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mark = dir.join("login-launch-off");
        assert!(enabled_at(&mark));
        std::fs::write(&mark, b"").unwrap();
        assert!(!enabled_at(&mark));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
