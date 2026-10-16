//! claude-board-hook — Claude Board 의 hook·설치·TUI 명령줄 실행 파일 (SPEC §1.1, §3).
//!
//! - `state <working|waiting|idle|ended|subagent>` / `notify <stop|notification>`: Claude Code hook.
//!   hook 입력 JSON 은 stdin. Claude Code 를 막거나 실패시키면 안 되므로 패닉까지 모두 잡고, 아무것도 출력하지 않고, 늘 0 으로 끝난다.
//! - `setup --app <ClaudeBoard.exe>` / `uninstall` / `install-hooks` / `uninstall-hooks`: 설치 도구. 짧은 진행 문구를 쓰고 실패하면 0 이 아닌 코드.
//! - `tui`: 터미널판 보드.

mod setup;
mod spawn;
mod tui;

use std::io::Read;
use std::process::ExitCode;

use claude_board_core::{notify, paths, state};
use serde_json::{Map, Value};

/// windows/VERSION (build.rs 가 넣는다)
const VERSION: &str = env!("CLAUDE_BOARD_VERSION");

const USAGE: &str = "사용법: claude-board-hook <명령>
  state <working|waiting|idle|ended|subagent>   hook 입력(stdin)을 세션 기록에 남긴다
  notify <stop|notification>                    알림 패널을 띄운다
  setup --app <ClaudeBoard.exe 경로>            hook 사본·hook 등록·명령·자동 실행을 설정한다
  uninstall                                     setup 이 한 일을 되돌린다
  install-hooks | uninstall-hooks               ~/.claude/settings.json 의 hook 만 넣거나 뺀다
  tui                                           터미널판 보드
  --version";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let arg = |i: usize| args.get(i).map(String::as_str);
    match arg(1).unwrap_or("") {
        "state" => {
            let s = arg(2).unwrap_or("idle").to_string();
            silent(move || run_state(&s))
        }
        "notify" => {
            let kind = arg(2).unwrap_or("stop").to_string();
            silent(move || run_notify(&kind))
        }
        "setup" => setup::setup(&args[2..]),
        "uninstall" => setup::uninstall(),
        "install-hooks" => setup::install_hooks(),
        "uninstall-hooks" => setup::uninstall_hooks(),
        "tui" => tui::run(),
        "--version" | "-V" | "version" => {
            println!("claude-board-hook {VERSION}");
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// hook 본체를 패닉까지 잡아 실행하고 늘 0 으로 끝낸다. 패닉 문구도 stderr 에 쓰지 않는다.
fn silent(f: impl FnOnce()) -> ExitCode {
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    ExitCode::SUCCESS
}

fn read_stdin() -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut buf);
    buf
}

fn run_state(state_arg: &str) {
    let input = read_stdin();
    state::run(&paths::sessions_dir(), state_arg, &input, &state::SystemEnv);
}

fn run_notify(kind: &str) {
    let input = read_stdin();
    // 깨진 입력은 빈 입력으로 본다(notify.py 와 같다). 객체가 아닌 JSON 은 띄우지 않는다
    let data = match serde_json::from_slice::<Value>(&input) {
        Ok(Value::Object(m)) => m,
        Ok(_) => return,
        Err(_) => Map::new(),
    };
    let cwd = std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    if let Some(panel) = notify::prepare(kind, &data, &paths::notifier_dir(), &cwd) {
        spawn::detached(&panel.app, &panel.args);
    }
}
