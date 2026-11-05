//! git 어댑터 — 엔진이 저장소를 만지는 유일한 길(`ports::Git`).
//! 늘 Windows `git.exe` 를 `-C <저장소>` 로 부르고, 저장소 안의 설정·훅이 코드를 돌리지 못하게 보호 환경을 건다.

use crate::ports::{Git, GitOutput, PortError, PortErrorKind, PortResult};
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 기본 시간 상한. 넘으면 git 을 죽이고 `Timeout`.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// 죽인 뒤 출력 읽는 쪽을 기다리는 시간. 손자 프로세스가 파이프를 쥐고 있어도 여기서 끊는다.
const DRAIN_WAIT: Duration = Duration::from_secs(2);
const POLL: Duration = Duration::from_millis(10);

/// 설정·훅 파일 자리에 쓰는 빈 장치. 없는 파일처럼 읽힌다.
#[cfg(windows)]
const NULL_DEVICE: &str = "NUL";
#[cfg(not(windows))]
const NULL_DEVICE: &str = "/dev/null";

#[cfg(windows)]
const GIT_EXE: &str = "git.exe";
#[cfg(not(windows))]
const GIT_EXE: &str = "git";

pub struct ProcessGit {
    timeout: Duration,
}

impl ProcessGit {
    pub fn new() -> Self {
        Self { timeout: DEFAULT_TIMEOUT }
    }

    pub fn with_timeout(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for ProcessGit {
    fn default() -> Self {
        Self::new()
    }
}

impl Git for ProcessGit {
    fn run(&self, repo: &Path, args: &[&str]) -> PortResult<GitOutput> {
        let exe = find_git().ok_or_else(|| {
            PortError::new(PortErrorKind::Unavailable, "git.exe 를 찾을 수 없다(PATH 와 Program Files\\Git\\cmd 모두)")
        })?;
        run_protected(&exe, repo, args, self.timeout)
    }
}

// ── git.exe 찾기 ─────────────────────────────────────

/// PATH 를 차례로 보고, 없으면 Program Files\Git\cmd. 작업 폴더는 보지 않는다(Command::new 는 Windows 에서 작업 폴더부터 찾는다).
fn find_git() -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    for var in ["ProgramFiles", "ProgramW6432", "ProgramFiles(x86)"] {
        if let Some(pf) = std::env::var_os(var) {
            dirs.push(PathBuf::from(pf).join("Git").join("cmd"));
        }
    }
    dirs.into_iter()
        .filter(|d| !d.as_os_str().is_empty())
        .map(|d| d.join(GIT_EXE))
        .find(|p| p.is_file())
}

// ── 실행 ─────────────────────────────────────────────

/// 늘 거는 설정(`-c`). 저장소 안의 설정·속성 파일이 엔진 프로세스 권한으로 명령을 돌리지 못하게 한다.
/// 이름이 정해진 칸만 여기서 막고, 이름을 저장소가 정하는 칸(`filter.<이름>.*`·`diff.<이름>.*`·`merge.<이름>.*`)은
/// `driver_neutralisers` 가 저장소 설정을 읽어 찾아 막는다
fn static_config() -> Vec<(&'static str, &'static str)> {
    vec![
        ("core.fsmonitor", "false"),
        ("core.hooksPath", NULL_DEVICE),
        ("core.attributesFile", NULL_DEVICE),
        ("core.excludesFile", NULL_DEVICE),
        ("diff.external", ""),
        ("core.pager", "cat"),
        ("protocol.ext.allow", "never"),
        ("commit.gpgsign", "false"),
    ]
}

/// 저장소가 이름을 붙인 코드 실행 칸(`filter.x.clean` 처럼)을 찾아 비우는 설정 쌍.
/// 쓰는 일은 `git config --list` 읽기뿐이라 코드가 돌지 않는다. 설정을 못 읽으면 빈 목록(그 저장소에 대한 본 명령도 같은 까닭으로 실패한다)
fn driver_neutralisers(exe: &Path, repo: &Path, timeout: Duration) -> Vec<(String, String)> {
    let mut cmd = base_command(exe, repo, &[]);
    cmd.args(["config", "--list", "--name-only", "-z"]);
    match exec(cmd, "config", timeout) {
        Ok(out) if out.code == 0 => neutralisers_for(&out.stdout),
        _ => Vec::new(),
    }
}

/// `git config --list --name-only -z` 출력(NUL 로 끝나는 이름들) → 비울 칸 쌍. 순수 함수
pub fn neutralisers_for(names: &str) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut put = |key: String, value: &str| {
        if !out.iter().any(|(k, _)| *k == key) {
            out.push((key, value.to_string()));
        }
    };
    for name in names.split('\0').filter(|n| !n.is_empty()) {
        let Some((section, rest)) = name.split_once('.') else { continue };
        let Some((sub, var)) = rest.rsplit_once('.') else { continue };
        match (section, var) {
            ("filter", "clean" | "smudge" | "process") => {
                put(name.to_string(), "");
                // 필수 필터가 비어 실패하면 status 가 죽는다 — 필수 표시도 끈다
                put(format!("filter.{sub}.required"), "false");
            }
            ("diff", "textconv" | "command") => put(name.to_string(), ""),
            ("merge", "driver") => put(name.to_string(), ""),
            _ => {}
        }
    }
    out
}

/// `diff`·`show`·`log` 에는 외부 diff·textconv 를 끄는 옵션을 앞에 끼운다(이미 있어도 무해)
fn with_safe_flags(args: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = args.iter().map(|a| a.to_string()).collect();
    if matches!(args.first().copied(), Some("diff" | "show" | "log")) {
        out.splice(1..1, ["--no-ext-diff".to_string(), "--no-textconv".to_string()]);
    }
    out
}

/// 보호 환경을 건 `git -C <저장소>` 명령. `extra_config` 는 환경 변수 설정 쌍(`GIT_CONFIG_COUNT`)으로 건다 —
/// 키에 `=` 가 있어도 `-c` 처럼 갈라지지 않는다
fn base_command(exe: &Path, repo: &Path, extra_config: &[(String, String)]) -> Command {
    let mut cmd = Command::new(exe);
    cmd.arg("--no-optional-locks");
    cmd.arg("-C").arg(repo);
    // -c 는 저장소 설정보다 우선한다. 훅 폴더는 비워 훅을 못 찾게 한다
    for (key, value) in static_config() {
        cmd.arg("-c").arg(format!("{key}={value}"));
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    apply_env(&mut cmd);
    if !extra_config.is_empty() {
        cmd.env("GIT_CONFIG_COUNT", extra_config.len().to_string());
        for (i, (key, value)) in extra_config.iter().enumerate() {
            cmd.env(format!("GIT_CONFIG_KEY_{i}"), key).env(format!("GIT_CONFIG_VALUE_{i}"), value);
        }
    }
    hide_window(&mut cmd);
    cmd
}

fn run_protected(exe: &Path, repo: &Path, args: &[&str], timeout: Duration) -> PortResult<GitOutput> {
    let neutral = driver_neutralisers(exe, repo, timeout);
    let mut cmd = base_command(exe, repo, &neutral);
    cmd.args(with_safe_flags(args));
    exec(cmd, args.first().copied().unwrap_or(""), timeout)
}

/// 띄워서 끝까지(또는 시간 상한까지) 기다리고 출력을 모은다
fn exec(mut cmd: Command, what: &str, timeout: Duration) -> PortResult<GitOutput> {
    let mut child = cmd.spawn()?;
    let out_rx = drain(child.stdout.take());
    let err_rx = drain(child.stderr.take());

    let start = Instant::now();
    let code = loop {
        if let Some(status) = child.try_wait()? {
            // 신호로 끝나 코드가 없으면 -1
            break status.code().unwrap_or(-1);
        }
        if start.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(PortError::new(
                PortErrorKind::Timeout,
                format!("git {what} 가 {}초 안에 끝나지 않았다", timeout.as_secs_f64()),
            ));
        }
        std::thread::sleep(POLL);
    };

    Ok(GitOutput {
        code,
        stdout: String::from_utf8_lossy(&out_rx.recv_timeout(DRAIN_WAIT).unwrap_or_default()).into_owned(),
        stderr: String::from_utf8_lossy(&err_rx.recv_timeout(DRAIN_WAIT).unwrap_or_default()).into_owned(),
    })
}

/// 상속된 git 환경을 걷어내고 보호 값을 건다.
fn apply_env(cmd: &mut Command) {
    // GIT_DIR 등이 남아 있으면 -C 와 상관없이 엉뚱한 저장소를 만진다. GIT_CONFIG_* 는 COUNT/KEY_n/VALUE_n 도 포함한다.
    // 외부 diff·속성 원본·편집기·pager·ssh·인증 도우미를 가리키는 변수도 코드를 돌릴 수 있어 걷어낸다
    const STRIPPED: [&str; 14] = [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_EXTERNAL_DIFF",
        "GIT_ATTR_SOURCE",
        "GIT_PAGER",
        "GIT_EDITOR",
        "GIT_SEQUENCE_EDITOR",
        "GIT_SSH",
        "GIT_SSH_COMMAND",
        "GIT_ASKPASS",
        "GIT_PROXY_COMMAND",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_OBJECT_DIRECTORY",
    ];
    let stripped: Vec<OsString> = std::env::vars_os()
        .map(|(k, _)| k)
        .filter(|k| {
            let up = k.to_string_lossy().to_ascii_uppercase();
            STRIPPED.contains(&up.as_str()) || up.starts_with("GIT_CONFIG")
        })
        .collect();
    for k in stripped {
        cmd.env_remove(k);
    }
    cmd.env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", NULL_DEVICE)
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0");
}

#[cfg(windows)]
fn hide_window(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
}

#[cfg(not(windows))]
fn hide_window(_cmd: &mut Command) {}

/// 파이프를 별도 스레드에서 끝까지 읽는다(가득 차 git 이 막히는 것을 막는다).
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<Vec<u8>> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut p) = pipe {
            let _ = p.read_to_end(&mut buf);
        }
        let _ = tx.send(buf);
    });
    rx
}

// ── 순수 도우미 ──────────────────────────────────────

/// `git status --porcelain=v1 -z` 출력 → (상태 두 글자, 경로). 바꾼 이름·복사는 새 경로만 남기고 옛 경로 항목은 건너뛴다.
pub fn parse_porcelain_status(out: &str) -> Vec<(String, String)> {
    let mut entries = out.split('\0').filter(|e| !e.is_empty());
    let mut result = Vec::new();
    while let Some(entry) = entries.next() {
        // "XY path": 상태 두 글자, 빈칸, 경로
        let Some(xy) = entry.get(..2) else { continue };
        let Some(path) = entry.get(3..) else { continue };
        if xy.contains('R') || xy.contains('C') {
            entries.next(); // 옛 경로
        }
        result.push((xy.to_string(), path.to_string()));
    }
    result
}

/// `git diff --shortstat`(또는 `--stat` 끝 줄) → (파일 수, 추가 줄, 지운 줄). 요약 줄이 없으면 (0, 0, 0).
pub fn parse_diff_stat(out: &str) -> (usize, usize, usize) {
    let Some(line) = out.lines().rev().find(|l| l.contains("changed")) else {
        return (0, 0, 0);
    };
    let mut files = 0;
    let mut ins = 0;
    let mut del = 0;
    for part in line.split(',') {
        let part = part.trim();
        let Some(n) = part.split_whitespace().next().and_then(|w| w.parse::<usize>().ok()) else { continue };
        if part.contains("changed") {
            files = n;
        } else if part.contains("insertion") {
            ins = n;
        } else if part.contains("deletion") {
            del = n;
        }
    }
    (files, ins, del)
}

/// 자율주행 가지 이름. 실행 id 는 `[A-Za-z0-9._-]{1,64}` 이고 `..` 을 품지 않아야 한다(가지 이름 규칙·경로 탈출 방지).
pub fn autopilot_branch_name(run_id: &str) -> PortResult<String> {
    let ok_chars = run_id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    let ok = ok_chars
        && !run_id.is_empty()
        && run_id.len() <= 64
        && !run_id.contains("..")
        // git 은 '.lock' 으로 끝나거나 '.' 으로 시작·끝나는 ref 이름을 거부한다
        && !run_id.starts_with('.')
        && !run_id.ends_with('.')
        && !run_id.ends_with(".lock");
    if ok {
        Ok(format!("autopilot/{run_id}"))
    } else {
        Err(PortError::new(PortErrorKind::Invalid, format!("실행 id 가 가지 이름으로 맞지 않다: {run_id:?}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_basic_and_rename() {
        let out = " M a.txt\0?? b c.txt\0R  new.txt\0old.txt\0A  z\0";
        assert_eq!(
            parse_porcelain_status(out),
            vec![
                (" M".into(), "a.txt".into()),
                ("??".into(), "b c.txt".into()),
                ("R ".into(), "new.txt".into()),
                ("A ".into(), "z".into()),
            ]
        );
        assert!(parse_porcelain_status("").is_empty());
    }

    #[test]
    fn diff_stat_lines() {
        assert_eq!(parse_diff_stat(" 3 files changed, 10 insertions(+), 2 deletions(-)\n"), (3, 10, 2));
        assert_eq!(parse_diff_stat(" 1 file changed, 1 insertion(+)\n"), (1, 1, 0));
        assert_eq!(parse_diff_stat(" 1 file changed, 4 deletions(-)\n"), (1, 0, 4));
        assert_eq!(parse_diff_stat(" a | 2 +-\n 1 file changed, 1 insertion(+), 1 deletion(-)\n"), (1, 1, 1));
        assert_eq!(parse_diff_stat(""), (0, 0, 0));
    }
}
