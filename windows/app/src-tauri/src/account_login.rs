//! 계정 추가 로그인(설정 > 계정) — 보드가 ConPTY 로 `claude auth login --claudeai` 를 돌리고(`CLAUDE_CONFIG_DIR=<슬롯>`),
//! 출력에서 주소를 찾아(`login_output.rs`) 프론트에 알리고, 사용자가 붙여 넣은 코드를 그 ConPTY 에 넣는다. 15분 안에 안 끝나면 접는다.
//!
//! 프론트와는 이벤트 `board://account-login`(`AccountLoginEvent`)과 명령 `begin_add_account`·`submit_login_code`·`cancel_add_account`(board.rs)·
//! `delete_account` 로 말한다.
//!
//! TODO(P2.5): ConPTY 로그인 본문. 지금 `begin_add_account` 는 예전 길(새 콘솔에서 로그인, `accounts::begin_add`)을 그대로 부르고,
//! `submit_login_code` 는 받을 로그인이 없어 아무것도 하지 않는다.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::accounts;
use crate::board::Board;
use crate::util::board_log;

/// 프론트로 보내는 로그인 진행 이벤트.
pub const ACCOUNT_LOGIN_EVENT: &str = "board://account-login";

/// 이벤트 내용 — 없는 칸은 보내지 않는다. `url` 은 열 로그인 주소, `note` 는 안내 한 줄, `done` 은 끝남, `error` 는 사유.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLoginEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// 프론트에 로그인 진행을 알린다.
#[allow(dead_code)]
pub fn emit(app: &AppHandle, event: &AccountLoginEvent) {
    if let Err(e) = app.emit(ACCOUNT_LOGIN_EVENT, event) {
        board_log(&format!("계정 로그인 이벤트를 보내지 못했습니다: {e}"));
    }
}

/// 프론트의 `beginAddAccount(email?)`.
#[tauri::command(async)]
pub fn begin_add_account(board: State<'_, Arc<Board>>, email: Option<String>) {
    // TODO(P2.5): ConPTY 로그인
    accounts::begin_add(&board, email);
}

/// 프론트의 `submitLoginCode(code)` — 로그인 창에 붙여 넣은 코드를 로그인 ConPTY 에 넣는다.
#[tauri::command(async)]
pub fn submit_login_code(code: String) {
    // TODO(P2.5): 로그인 ConPTY 에 쓰기
    let _ = code;
}

/// 프론트의 `deleteAccount(id)` — 확인은 프론트가 받는다.
#[tauri::command(async)]
pub fn delete_account(board: State<'_, Arc<Board>>, id: String) {
    accounts::remove_account(&board, id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_sends_only_the_fields_that_are_set() {
        let url = AccountLoginEvent { url: Some("https://claude.ai/oauth/authorize?x=1".into()), ..Default::default() };
        assert_eq!(serde_json::to_value(&url).unwrap(), serde_json::json!({"url": "https://claude.ai/oauth/authorize?x=1"}));
        let done = AccountLoginEvent { done: Some(true), error: Some("취소".into()), ..Default::default() };
        assert_eq!(serde_json::to_value(&done).unwrap(), serde_json::json!({"done": true, "error": "취소"}));
        assert_eq!(serde_json::to_value(AccountLoginEvent::default()).unwrap(), serde_json::json!({}));
    }
}
