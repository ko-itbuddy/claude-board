//! `claude auth login --claudeai` 출력 읽기 — 순수 함수만 둔다(mac 의 파서와 같은 규칙).
//!
//! TODO(P2.5): 본문. 지금은 중립 값을 돌려준다(아무도 부르지 않는다).
//! - `strip_ansi` — 색·커서 이동 제어열을 걷어 낸다.
//! - `find_login_url` — `oauth/authorize` 가 든 https 주소를 공백 앞까지(mac 정규식).
//! - `failure_reason` — 마지막 비어 있지 않은 줄에서 `prompted >` 뒤의 글.

/// 제어열을 걷어 낸 글.
#[allow(dead_code)]
pub fn strip_ansi(text: &str) -> String {
    text.to_string()
}

/// 로그인 주소 — 출력에서 처음 찾은 것.
#[allow(dead_code)]
pub fn find_login_url(text: &str) -> Option<String> {
    let _ = text;
    None
}

/// 로그인이 막혔을 때의 사유.
#[allow(dead_code)]
pub fn failure_reason(text: &str) -> Option<String> {
    let _ = text;
    None
}
