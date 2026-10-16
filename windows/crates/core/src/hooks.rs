//! 전역 settings.json 에 Claude Board hook 을 넣거나 뺀다 — macOS `hooks.py` 이식 (SPEC §3.1).
//!
//! - exec form: `{"type":"command","command":"C:/Users/<u>/.claude/notifier/claude-board-hook.exe","args":[…],"timeout":5}`
//!   — 셸을 거치지 않는다. 명령 경로는 슬래시로 써서 macOS 와 같은 표시 `.claude/notifier/` 가 들어가게 한다.
//! - 다른 hook·설정은 건드리지 않는다. 명령에 `.claude/notifier/` 나 `.claude\notifier\` 가 든 hook 만 우리 것으로 본다.
//! - 바꾸기 전에 `settings.json.bak-claude-board-<YYYYmmdd-HHMMSS>` 로 백업하고, JSON 이 깨져 있으면 고치지 않는다.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{json, Map, Value};

use crate::{fileio, Record};

/// 우리 hook 을 알아보는 표시(macOS `hooks.py` 의 MARK 와 Windows 역슬래시 판).
pub const MARKERS: [&str; 2] = [".claude/notifier/", ".claude\\notifier\\"];
/// 탭 제목은 state hook 이 상태 아이콘과 함께 쓰므로 Claude Code 의 제목 쓰기를 끈다.
pub const ENV_KEY: &str = "CLAUDE_CODE_DISABLE_TERMINAL_TITLE";

/// (이벤트, matcher, hook 마다의 인자 목록)
pub type HookGroup = (&'static str, Option<&'static str>, &'static [&'static [&'static str]]);

/// 도구 이벤트만 matcher "*" 를 쓴다. `hooks.py` 의 HOOKS 와 같은 순서·묶음.
pub const HOOKS: &[HookGroup] = &[
    ("SessionStart", None, &[&["state", "idle"]]),
    ("UserPromptSubmit", None, &[&["state", "working"]]),
    ("PreToolUse", Some("*"), &[&["state", "working"]]),
    ("PostToolUse", Some("*"), &[&["state", "working"]]),
    ("PermissionRequest", Some("*"), &[&["state", "waiting"]]),
    ("Notification", None, &[&["notify", "notification"], &["state", "waiting"]]),
    ("Stop", None, &[&["notify", "stop"], &["state", "idle"]]),
    ("StopFailure", None, &[&["state", "idle"]]),
    ("SessionEnd", None, &[&["state", "ended"]]),
    ("SubagentStart", None, &[&["state", "subagent"]]),
    ("SubagentStop", None, &[&["state", "subagent"]]),
];

#[derive(Debug)]
pub enum Error {
    /// settings.json 이 JSON 으로 읽히지 않는다 — 고치지 않는다.
    Malformed(String),
    /// JSON 이지만 `hooks`·`env` 모양이 예상과 다르다 — 고치지 않는다.
    Shape(&'static str),
    Io(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed(e) => write!(f, "settings.json 을 읽지 못했습니다(JSON 오류: {e}). 파일을 고친 뒤 다시 실행하세요."),
            Error::Shape(what) => write!(f, "settings.json 의 {what} 형식이 예상과 달라 고치지 않았습니다. 파일을 확인한 뒤 다시 실행하세요."),
            Error::Io(e) => write!(f, "settings.json 을 쓰지 못했습니다: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::Io(e)
    }
}

/// 설치·제거 결과 — 만든 백업 파일(원래 settings.json 이 없었으면 None).
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    pub backup: Option<PathBuf>,
}

/// hook 명령 경로 — 역슬래시를 슬래시로 바꾼다(`C:/Users/<u>/.claude/notifier/claude-board-hook.exe`).
pub fn command_path(exe: &Path) -> String {
    exe.to_string_lossy().replace('\\', "/")
}

fn is_ours(hook: &Value) -> bool {
    hook.get("command").and_then(Value::as_str).is_some_and(|c| MARKERS.iter().any(|m| c.contains(m)))
}

/// 우리 hook 을 모두 빼고, 비게 된 묶음·이벤트도 지운다. 모양이 다른 항목은 그대로 둔다.
pub fn strip(settings: &mut Record) {
    let Some(Value::Object(hooks)) = settings.get_mut("hooks") else { return };
    let events: Vec<String> = hooks.keys().cloned().collect();
    for event in events {
        let Some(Value::Array(groups)) = hooks.get_mut(&event) else { continue };
        let mut kept_groups = Vec::new();
        for g in groups.drain(..) {
            let Value::Object(mut group) = g else {
                kept_groups.push(g);
                continue;
            };
            let kept: Vec<Value> = match group.get("hooks") {
                Some(Value::Array(items)) => items.iter().filter(|h| !is_ours(h)).cloned().collect(),
                _ => Vec::new(),
            };
            if !kept.is_empty() {
                group.insert("hooks".into(), Value::Array(kept));
                kept_groups.push(Value::Object(group));
            }
        }
        if kept_groups.is_empty() {
            hooks.shift_remove(&event);
        } else {
            *groups = kept_groups;
        }
    }
    if hooks.is_empty() {
        settings.shift_remove("hooks");
    }
}

/// HOOKS 표대로 우리 hook 을 넣고 env 키를 켠다(먼저 [`strip`] 해야 겹치지 않는다).
pub fn add(settings: &mut Record, command: &str) {
    if !settings.get("hooks").is_some_and(Value::is_object) {
        settings.insert("hooks".into(), Value::Object(Map::new()));
    }
    if let Some(Value::Object(hooks)) = settings.get_mut("hooks") {
        for (event, matcher, items) in HOOKS {
            let items: Vec<Value> = items
                .iter()
                .map(|args| json!({"type": "command", "command": command, "args": args, "timeout": 5}))
                .collect();
            let mut group = Map::new();
            if let Some(m) = matcher {
                group.insert("matcher".into(), json!(m));
            }
            group.insert("hooks".into(), Value::Array(items));
            let list = hooks.entry(event.to_string()).or_insert_with(|| json!([]));
            if let Value::Array(list) = list {
                list.push(Value::Object(group));
            }
        }
    }
    if !settings.get("env").is_some_and(Value::is_object) {
        settings.insert("env".into(), Value::Object(Map::new()));
    }
    if let Some(Value::Object(env)) = settings.get_mut("env") {
        env.insert(ENV_KEY.into(), json!("1"));
    }
}

/// env 키를 지우고, env 가 비면 env 도 지운다.
pub fn remove_env(settings: &mut Record) {
    if let Some(Value::Object(env)) = settings.get_mut("env") {
        env.shift_remove(ENV_KEY);
        if env.is_empty() {
            settings.shift_remove("env");
        }
    }
}

/// 백업 파일 이름 — `<settings>.bak-claude-board-<stamp>`.
pub fn backup_path(settings: &Path, stamp: &str) -> PathBuf {
    let mut name = settings.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".bak-claude-board-{stamp}"));
    settings.with_file_name(name)
}

/// 읽기 → 모양 확인 → 백업. 파일이 없으면 폴더를 만들고 빈 설정으로 시작한다.
fn load(path: &Path, stamp: &str) -> Result<(Record, Option<PathBuf>), Error> {
    if !path.exists() {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        return Ok((Map::new(), None));
    }
    let text = fs::read_to_string(path)?;
    let settings = if text.trim().is_empty() {
        Map::new()
    } else {
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Object(m)) => m,
            Ok(_) => return Err(Error::Shape("최상위")),
            Err(e) => return Err(Error::Malformed(e.to_string())),
        }
    };
    match settings.get("hooks") {
        None => {}
        Some(Value::Object(h)) if h.values().all(Value::is_array) => {}
        Some(_) => return Err(Error::Shape("hooks")),
    }
    if settings.get("env").is_some_and(|e| !e.is_object()) {
        return Err(Error::Shape("env"));
    }
    let backup = backup_path(path, stamp);
    fs::copy(path, &backup)?;
    Ok((settings, Some(backup)))
}

/// 들여쓰기 2칸, 끝에 줄바꿈, 임시 파일 후 바꿔 넣기.
fn save(path: &Path, settings: &Record) -> Result<(), Error> {
    let mut text = serde_json::to_string_pretty(settings).map_err(|e| Error::Io(io::Error::other(e)))?;
    text.push('\n');
    let mut tmp = path.as_os_str().to_os_string();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    fs::write(&tmp, text)?;
    fileio::replace(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })?;
    Ok(())
}

/// `install-hooks` — 백업 → 우리 hook 을 빼고 → HOOKS 표대로 넣고 → env 키를 켠다.
/// `command` 는 [`command_path`] 로 만든 hook 실행 파일 경로, `stamp` 는 백업 이름에 붙일 지역 시각.
pub fn install(settings_path: &Path, command: &str, stamp: &str) -> Result<Report, Error> {
    let (mut settings, backup) = load(settings_path, stamp)?;
    strip(&mut settings);
    add(&mut settings, command);
    save(settings_path, &settings)?;
    Ok(Report { backup })
}

/// `uninstall-hooks` — 백업 → 우리 hook 만 빼고 env 키를 지운다.
pub fn uninstall(settings_path: &Path, stamp: &str) -> Result<Report, Error> {
    let (mut settings, backup) = load(settings_path, stamp)?;
    strip(&mut settings);
    remove_env(&mut settings);
    save(settings_path, &settings)?;
    Ok(Report { backup })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("cb-hooks-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    const CMD: &str = "C:/Users/u/.claude/notifier/claude-board-hook.exe";

    #[test]
    fn command_path_uses_slashes() {
        assert_eq!(command_path(Path::new(r"C:\Users\u\.claude\notifier\claude-board-hook.exe")), CMD);
    }

    #[test]
    fn round_trip_keeps_other_settings() {
        let dir = temp("round");
        let path = dir.join("settings.json");
        let original = json!({
            "model": "opus",
            "hooks": {
                "Stop": [
                    {"hooks": [
                        {"type": "command", "command": "python3 \"$HOME/.claude/notifier/notify.py\" stop", "timeout": 5},
                        {"type": "command", "command": "my-own-hook.sh"}
                    ]},
                    {"matcher": "x", "hooks": [{"type": "command", "command": "C:\\Users\\u\\.claude\\notifier\\old.exe"}]}
                ],
                "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard.sh"}]}],
                "SessionEnd": [{"hooks": [{"type": "command", "command": "python3 ~/.claude/notifier/state.py ended"}]}]
            },
            "env": {"OTHER": "x"},
            "zLast": true
        });
        fs::write(&path, serde_json::to_string_pretty(&original).unwrap()).unwrap();

        let report = install(&path, CMD, "20261016-120000").unwrap();
        let backup = report.backup.unwrap();
        assert_eq!(backup, dir.join("settings.json.bak-claude-board-20261016-120000"));
        assert_eq!(fs::read_to_string(&backup).unwrap(), serde_json::to_string_pretty(&original).unwrap());

        let text = fs::read_to_string(&path).unwrap();
        assert!(text.ends_with("}\n"));
        let s: Value = serde_json::from_str(&text).unwrap();
        // 사용자 키 순서는 그대로
        let keys: Vec<&String> = s.as_object().unwrap().keys().collect();
        assert_eq!(keys, ["model", "hooks", "env", "zLast"]);
        assert_eq!(s["env"], json!({"OTHER": "x", ENV_KEY: "1"}));
        let stop = s["hooks"]["Stop"].as_array().unwrap();
        assert_eq!(stop[0], json!({"hooks": [{"type": "command", "command": "my-own-hook.sh"}]}));
        assert_eq!(
            stop[1],
            json!({"hooks": [
                {"type": "command", "command": CMD, "args": ["notify", "stop"], "timeout": 5},
                {"type": "command", "command": CMD, "args": ["state", "idle"], "timeout": 5}
            ]})
        );
        let pre = s["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre[0]["matcher"], "Bash");
        assert_eq!(pre[1], json!({"matcher": "*", "hooks": [{"type": "command", "command": CMD, "args": ["state", "working"], "timeout": 5}]}));
        assert_eq!(s["hooks"]["SessionEnd"], json!([{"hooks": [{"type": "command", "command": CMD, "args": ["state", "ended"], "timeout": 5}]}]));
        let events: Vec<&String> = s["hooks"].as_object().unwrap().keys().collect();
        assert_eq!(events.len(), 11);
        // 두 번 설치해도 겹치지 않는다
        install(&path, CMD, "20261016-120001").unwrap();
        let again: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(again, s);

        uninstall(&path, "20261016-120002").unwrap();
        let after: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            after,
            json!({
                "model": "opus",
                "hooks": {
                    "Stop": [{"hooks": [{"type": "command", "command": "my-own-hook.sh"}]}],
                    "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "guard.sh"}]}]
                },
                "env": {"OTHER": "x"},
                "zLast": true
            })
        );
        assert!(!fs::read_to_string(&path).unwrap().contains(".claude/notifier/"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn uninstall_drops_empty_env_and_hooks() {
        let dir = temp("empty");
        let path = dir.join("sub").join("settings.json");
        // 파일이 없으면 폴더를 만들고 백업하지 않는다
        let report = install(&path, CMD, "s").unwrap();
        assert_eq!(report.backup, None);
        uninstall(&path, "t").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{}\n");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn malformed_is_refused() {
        let dir = temp("bad");
        let path = dir.join("settings.json");
        for body in ["{\"hooks\": {", "[1, 2]", "{\"hooks\": []}", "{\"hooks\": {\"Stop\": {}}}", "{\"env\": 3}"] {
            fs::write(&path, body).unwrap();
            assert!(install(&path, CMD, "x").is_err(), "{body}");
            assert!(uninstall(&path, "x").is_err(), "{body}");
            assert_eq!(fs::read_to_string(&path).unwrap(), body);
        }
        // 고치지 않았으니 백업도 남기지 않는다
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        // 빈 파일은 빈 설정으로 본다
        fs::write(&path, "  \n").unwrap();
        install(&path, CMD, "y").unwrap();
        assert!(fs::read_to_string(&path).unwrap().contains(CMD));
        fs::remove_dir_all(&dir).unwrap();
    }
}
