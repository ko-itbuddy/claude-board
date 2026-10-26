//! 시험용 claude 바꿔치기 — 환경변수 `CLAUDE_BOARD_CLAUDE`(`|` 로 이은 절대 경로: 프로그램과 앞쪽 인자)가 있고
//! `CLAUDE_BOARD_HOME` 도 있을 때만 보드가 치는 명령의 맨 앞 `claude` 를 그 값으로 바꾼다. 진짜 claude 를 띄우지 않고 명령 길을 시험하려는 장치다.
//!
//! TODO(P1.T): 지금은 계약만 있다 — `test_claude_prefix` 는 늘 None(바꾸지 않는다).
//! `prefix(env)` 규칙(둘 다 있을 때만, 모든 조각이 절대 경로이고 실재할 때만, 아니면 None 과 board.log 한 줄)은 P1.T 가 채운다.

/// 프론트의 `testClaudePrefix()` — 바꿔칠 프로그램·앞쪽 인자. 시험 모드가 아니면 None.
#[tauri::command(async)]
pub fn test_claude_prefix() -> Option<Vec<String>> {
    None
}
