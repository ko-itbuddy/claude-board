// windows/VERSION 을 읽어 CLAUDE_BOARD_VERSION 으로 넣는다 — 업데이트 확인·제목 옆 버전 표시가 쓴다.
// 못 읽으면 빈 값(개발 빌드로 본다)
fn main() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../VERSION");
    println!("cargo:rerun-if-changed={}", path.display());
    let version = std::fs::read_to_string(&path).map(|s| s.trim().to_string()).unwrap_or_default();
    println!("cargo:rustc-env=CLAUDE_BOARD_VERSION={version}");
    tauri_build::build()
}
