//! 끌 때 확인과 탭 목록 저장 — 트레이 "종료"·업데이트 직전에 도는 세션을 보여 주고 확인을 받는다(mac `confirmShutdown`),
//! 다시 뜰 때 이어 열 탭 목록을 설정에 남긴다(mac `saveTabs`·`restoreTabs`).
//!
//! 확인 창은 보드 창 안의 Svelte 모달이다. 백엔드가 창을 보이고 `board://confirm-shutdown` 을 보내면 프론트가 목록을 만들어 보여 주고
//! `answer_shutdown` 으로 답한다. 목록 규칙(누구를 보이고 누구에게 `.paused` 를 남기나)은 프론트 `terminal/shutdownList.ts` 가 쥔다.
//!
//! TODO(P1.1): 지금은 계약만 있다 — `quit`·`confirm_update` 는 확인 없이 지나가고, `answer_shutdown` 은 받아 버리고, 탭 목록은 설정에 그대로 넣고 꺼낸다.
//! 확인 대기·`.paused` 쓰기(`terminal.rs paused_mark` 를 거친다)·응답 없음 10초 뒤 진행은 P1.1 이 채운다.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, State};

use crate::board::Board;

/// 끄기 전에 프론트에 확인 창을 띄우라고 보내는 이벤트 — 내용은 `types.ts ConfirmShutdownPayload`.
#[allow(dead_code)] // TODO(P1.1): 확인 요청을 보내는 쪽(request)이 이 이름을 쓴다
pub const CONFIRM_SHUTDOWN_EVENT: &str = "board://confirm-shutdown";

/// 트레이 "종료". 확인 없이 바로 끝낸다.
/// TODO(P1.1): 도는 세션이 있으면 `CONFIRM_SHUTDOWN_EVENT` 로 확인을 받은 뒤 끝낸다.
pub fn quit(app: &AppHandle) {
    app.exit(0);
}

/// 업데이트를 시작해도 되는지. 확인 없이 늘 true.
/// TODO(P1.1): 도는 세션이 있으면 확인을 받아 취소하면 false.
pub fn confirm_update(_board: &Board) -> bool {
    true
}

/// 프론트의 `answerShutdown(ok, pause)` — 확인 창의 답. `pause` 는 `.paused` 를 남길 세션 id.
/// TODO(P1.1): 기다리는 `request` 를 깨우고 `pause` 에 표시를 쓴다.
#[tauri::command(async)]
pub fn answer_shutdown(_ok: bool, _pause: Vec<String>) {}

/// 프론트의 `saveTabs(entries)` — 다시 뜰 때 이어 열 탭 목록을 설정(`terminalTabs`)에 통째로 바꿔 넣는다. 항목 모양은 프론트가 쥔다.
#[tauri::command(async)]
pub fn save_tabs(board: State<'_, Arc<Board>>, entries: Vec<Value>) {
    board.update_settings(|s| s.terminal_tabs = entries);
}

/// 프론트의 `loadTabs()` — 저장해 둔 탭 목록. 없으면 빈 목록.
#[tauri::command(async)]
pub fn load_tabs(board: State<'_, Arc<Board>>) -> Vec<Value> {
    board.settings.lock().unwrap().terminal_tabs.clone()
}
