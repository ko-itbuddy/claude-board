//! 전역 단축키(Ctrl+Alt+B 붙이기 · Ctrl+Alt+T 터미널 펼치기·접기)와 단축키 표 저장 명령.
//!
//! 앱 안 단축키(탭 이동·새 세션 등)는 프론트(`shortcuts/dispatch.ts`)가 맞추고, 여기는 어느 앱에서든 듣는 둘과 사용자가 바꾼 키(`shortcuts`)의 저장만 맡는다.
//! 저장 모양은 `settings.rs Settings::set_shortcut` 이 쥔다 — 동작 id → `[키 코드, 수정키 비트]`, 비움 `[]`, 기본값이면 항목 없음.
//!
//! TODO(P2.4): 저장한 `snap`·`terminal` 키로 전역 단축키를 다시 등록한다(설정 화면이 키를 받는 동안은 풀어 둔다).
//! 등록에 실패한 동작은 `board://settings` 가 아니라 상태 묶음으로 알린다(설계 §4 `hotkeys::`). 지금은 Ctrl+Alt+B/T 고정이다.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager, State};

use crate::board::Board;
use crate::util::board_log;

/// 전역 단축키 플러그인을 켜고 Ctrl+Alt+B/T 를 등록한다. 보드를 켤 때 한 번 부른다.
pub fn register(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
    let snap = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyB);
    let terminal = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyT);
    let plugin = tauri_plugin_global_shortcut::Builder::new()
        .with_handler(move |app, shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            let Some(board) = app.try_state::<Arc<Board>>() else { return };
            if *shortcut == snap {
                // 터미널을 접고 보드 쪽에 붙여 앞으로 — 포커스는 지금 앱에 남긴다
                board.snap_right();
            } else if *shortcut == terminal {
                board.toggle_terminal();
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        board_log(&format!("전역 단축키 플러그인을 켜지 못했습니다: {e}"));
        return;
    }
    // 다른 앱이 먼저 잡은 단축키면 등록에 실패한다 — 보드는 그대로 동작하고 로그만 남긴다
    for (shortcut, name) in [(snap, "Ctrl+Alt+B"), (terminal, "Ctrl+Alt+T")] {
        if let Err(e) = app.global_shortcut().register(shortcut) {
            board_log(&format!("전역 단축키 {name} 등록 실패 ({e})"));
        }
    }
}

/// 프론트의 `setShortcut(id, combo)` — 저장 모양 그대로 받는다. `combo` 가 없으면 기본값으로 돌아가고(항목 삭제), `[]` 는 비움이다.
#[tauri::command(async)]
pub fn set_shortcut(board: State<'_, Arc<Board>>, id: String, combo: Option<Vec<Value>>) {
    if !board.update_settings(|s| s.set_shortcut(&id, combo)) {
        board_log(&format!("단축키 {id} 의 키 모양이 틀려 저장하지 않았습니다"));
        return;
    }
    board.publish();
}

/// 프론트의 `resetShortcuts()` — 바꾼 키를 모두 지워 기본값으로(mac `Shortcuts.resetAll`).
#[tauri::command(async)]
pub fn reset_shortcuts(board: State<'_, Arc<Board>>) {
    board.update_settings(|s| s.shortcuts.clear());
    board.publish();
}
