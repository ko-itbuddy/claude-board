// 버전의 단일 출처 windows/VERSION 을 읽어 CLAUDE_BOARD_VERSION 으로 넣는다(읽지 못하면 "dev")
use std::path::Path;

fn main() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../VERSION");
    println!("cargo:rerun-if-changed={}", file.display());
    let version = std::fs::read_to_string(&file)
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "dev".to_string());
    println!("cargo:rustc-env=CLAUDE_BOARD_VERSION={version}");
}
