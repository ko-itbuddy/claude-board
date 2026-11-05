//! 엔진 본체 — `engine::decide` 를 구동하고 `Action` 을 포트 위에서 수행한다(설계 원본 macOS `main.swift` 의 `Engine`).
//! 이 파일은 순수 부품(모델 글·실행 기록 행·파일 → 상태)의 문만 연다. 루프와 수행은 이 위에 이어 쓴다.
//! 순수 부품은 포트를 부르지 않는다.

mod load;
mod prompt;
mod rows;

pub use load::{
    abnormal_recent, caps_of, frozen_digest, is_abnormal, load_state, next_from, next_text, session_uuid, starts_line, Loaded,
};
pub use prompt::{first_message, is_under, PromptUnit, RESULT_FORMAT, SYSTEM_PROMPT};
pub use rows::{project_row, run_row, unit_row};
