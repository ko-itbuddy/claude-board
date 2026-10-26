//! 앱 쪽 md-render — 답변 꾸미기 플러그인(`plugins/md-render`)을 `%USERPROFILE%\.claude\notifier\plugins\md-render\` 로 옮기고,
//! 보드 탭에서 띄우는 claude 에 `--plugin-dir` 을 붙일 때 그 경로를 프론트에 알려 준다(mac `mdRenderDir`).
//!
//! TODO(P1.2): 지금은 계약만 있다 — `md_render_dir` 는 늘 None(플래그를 붙이지 않는다). 시작 때 바뀐 파일만 옮기는 `sync` 와
//! 옮기기 계획 `copy_plan` 은 P1.2 가 채운다.

use std::sync::Arc;

use tauri::State;

use crate::board::Board;
use crate::settings::set_in;

/// 플러그인에 싣는 다섯 파일(저장소 `plugins/md-render` 기준 상대 경로) — `tests/` 는 싣지 않는다.
#[allow(dead_code)] // TODO(P1.2): sync·copy_plan 이 쓴다
pub const PLUGIN_FILES: [&str; 5] = ["LICENSE", "README.md", ".claude-plugin/plugin.json", "hooks/hooks.json", "hooks/register.tsx"];

/// 프론트의 `mdRenderDir()` — 옮겨 둔 md-render 폴더. 옮기지 못했으면 None(그러면 `--plugin-dir` 을 붙이지 않는다).
#[tauri::command(async)]
pub fn md_render_dir() -> Option<String> {
    None
}

/// 프론트의 `rememberMdRenderOff(id)` — 꾸미기를 끄고 띄운 세션 id 를 설정(`mdRenderOff`)에 넣는다.
#[tauri::command(async)]
pub fn remember_md_render_off(board: State<'_, Arc<Board>>, id: String) {
    board.update_settings(|s| set_in(&mut s.md_render_off, &id, true));
}
