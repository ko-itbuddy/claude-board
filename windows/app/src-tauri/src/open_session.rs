//! 세션 열기 요청 (W3 설계 §4) — 끝난 세션 이어 열기(`reopen`)·창으로 이동(`focus`)을 프론트(보드 터미널 허브)에 넘긴다.
//! 이벤트 `board://open-session` 로 보내되, 프론트가 `take_open_session` 을 처음 부르기 전에 생긴 요청은 줄에 쌓아 둔다
//! (보드가 막 떠서 `--action` 이 먼저 오는 경우).

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::board::Board;
use crate::model::Session;
use crate::util::board_log;

pub const OPEN_SESSION_EVENT: &str = "board://open-session";

/// 프론트로 가는 요청 — `types.ts OpenSessionPayload` 와 같은 모양.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSessionPayload {
    pub id: String,
    pub action: String,
    pub carry_on: bool,
    pub project_dir: String,
    pub resume_args: Vec<String>,
}

/// 프론트가 준비되기 전 요청을 쌓는 줄.
#[derive(Debug, Default)]
pub struct OpenQueue {
    ready: bool,
    pending: Vec<OpenSessionPayload>,
}

impl OpenQueue {
    /// 준비됐으면 바로 보낼 요청을 돌려주고, 아니면 쌓는다.
    pub fn push(&mut self, p: OpenSessionPayload) -> Option<OpenSessionPayload> {
        if self.ready {
            return Some(p);
        }
        self.pending.push(p);
        None
    }

    /// 프론트가 준비됐다고 표시하고 쌓인 요청을 낸다 — (가장 오래된 것, 나머지).
    pub fn take(&mut self) -> (Option<OpenSessionPayload>, Vec<OpenSessionPayload>) {
        self.ready = true;
        let mut all = std::mem::take(&mut self.pending);
        let first = (!all.is_empty()).then(|| all.remove(0));
        (first, all)
    }
}

fn emit(app: &AppHandle, p: &OpenSessionPayload) {
    if let Err(e) = app.emit(OPEN_SESSION_EVENT, p) {
        board_log(&format!("세션 열기 요청을 보내지 못했습니다: {e}"));
    }
}

/// 세션을 보드 터미널 허브에서 열어 달라고 요청한다. `action` 은 `reopen` 또는 `focus`.
pub fn request(board: &Board, s: &Session, action: &str, carry_on: bool) {
    let payload = OpenSessionPayload {
        id: s.id.clone(),
        action: action.to_string(),
        carry_on,
        project_dir: s.project_dir.clone(),
        resume_args: s.resume_args.clone(),
    };
    let now = board.open_queue.lock().unwrap().push(payload.clone());
    if let Some(p) = now {
        emit(&board.app, &p);
    }
}

/// 프론트가 시작할 때 한 번 부른다 — 그 전에 쌓인 가장 오래된 요청을 돌려주고 나머지는 이벤트로 다시 보낸다.
#[tauri::command(async)]
pub fn take_open_session(board: State<'_, Arc<Board>>) -> Option<OpenSessionPayload> {
    let (first, rest) = board.open_queue.lock().unwrap().take();
    for p in &rest {
        emit(&board.app, p);
    }
    first
}

/// 보드 창을 펼쳐 앞으로 가져온다(mac `showBoard`).
#[tauri::command(async)]
pub fn show_board(app: AppHandle) {
    crate::window::show(&app, true);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(id: &str) -> OpenSessionPayload {
        OpenSessionPayload { id: id.into(), action: "reopen".into(), carry_on: true, project_dir: "C:/p".into(), resume_args: vec!["--resume".into(), id.into()] }
    }

    #[test]
    fn requests_before_the_frontend_is_ready_are_queued_in_order() {
        let mut q = OpenQueue::default();
        assert_eq!(q.push(payload("a")), None);
        assert_eq!(q.push(payload("b")), None);
        let (first, rest) = q.take();
        assert_eq!(first, Some(payload("a")));
        assert_eq!(rest, vec![payload("b")]);
        // 한 번 비운 뒤에는 바로 보낸다
        assert_eq!(q.push(payload("c")), Some(payload("c")));
        assert_eq!(q.take(), (None, Vec::new()));
    }

    #[test]
    fn empty_queue_still_marks_the_frontend_ready() {
        let mut q = OpenQueue::default();
        assert_eq!(q.take(), (None, Vec::new()));
        assert_eq!(q.push(payload("a")), Some(payload("a")));
    }

    #[test]
    fn payload_serializes_to_the_typescript_shape() {
        let v = serde_json::to_value(payload("a")).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"id": "a", "action": "reopen", "carryOn": true, "projectDir": "C:/p", "resumeArgs": ["--resume", "a"]})
        );
    }
}
