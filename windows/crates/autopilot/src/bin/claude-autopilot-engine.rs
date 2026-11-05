//! claude-autopilot-engine.exe — 작업 스케줄러가 띄우는 엔진. 창이 뜨지 않게 창 없는 하위 시스템이다.
//! `run` 한 가지만 받는다. 본체는 `cli` 모듈이 소유한다(청크 4.3). 지금은 사용법만 내고 실패한다.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    std::process::exit(2);
}
