//! hook 상태 기록 — macOS `state.py` 이식 (SPEC §3.2–3.5).
//!
//! [`apply`] 는 순수 함수다: 세션 기록 + 상태 인자 + hook 입력 + 시각 + 주입한 프로세스 정보([`Env`])
//! → 새 기록 + 바꿀 터미널 제목. 파일 잠금·읽기·쓰기·prune·`.paused` 삭제는 [`run`] 이 맡는다.

use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::text::{collapse_ws, non_empty_str, take_chars, truthy};
use crate::{fileio, paths, proc, quote, Record};

/// 상태별 터미널 제목 아이콘.
pub fn icon(state: &str) -> &'static str {
    match state {
        "working" => "🔵",
        "waiting" => "🟠",
        "idle" => "🟢",
        "ended" => "⚫",
        _ => "",
    }
}

/// hook 의 조상에서 찾은 Claude Code 프로세스.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClaudeProcess {
    pub pid: u32,
    /// 프로세스 생성 시각(유닉스 초) — Windows 에서 pid 재사용을 가려낸다. macOS 는 None.
    pub ctime: Option<f64>,
    /// 터미널 장치 이름 — macOS 만. Windows 는 늘 None.
    pub tty: Option<String>,
    pub headless: bool,
    /// 실행 인자 전체(argv[0] 포함). 읽지 못하면 빈 목록.
    pub argv: Vec<String>,
}

/// [`apply`] 가 바깥 세계에서 얻는 것들 — 시험에서는 가짜로 바꾼다.
pub trait Env {
    /// hook 프로세스의 조상 가운데 Claude Code 프로세스.
    fn find_claude(&self) -> Option<ClaudeProcess>;
    /// 그 pid 가 아직 살아 있는지(`ctime` 이 있으면 생성 시각도 같아야 한다).
    fn alive(&self, pid: u32, ctime: Option<f64>) -> bool;
    /// 그 프로세스의 실행 인자 전체(argv[0] 포함).
    fn argv(&self, pid: u32) -> Vec<String>;
    /// `<transcript 에서 .jsonl 을 뺀 경로>/subagents/agent-<id>.meta.json` 내용(없으면 빈 객체).
    fn subagent_meta(&self, transcript: &str, agent_id: &str) -> Record;
    /// hook 입력과 기록에 cwd 가 없을 때 쓸 현재 폴더.
    fn cwd(&self) -> String;
}

/// [`apply`] 의 결과.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub record: Record,
    /// 바꿀 터미널 제목(`"<아이콘> <project> · <topic 앞 20자>…"`). 상태나 topic 이 바뀌었고 터미널이 있을 때만.
    pub title: Option<String>,
    /// `UserPromptSubmit` — 보드의 일시중지 표지(`<sid>.paused`)를 지운다.
    pub remove_paused: bool,
}

// 세션을 다시 열 때 그대로 넘길 실행 인자 — 값을 받는 것, 값을 여러 개 받는 것, 값이 없는 것
const CARRY_ONE: &[&str] = &["--model", "--permission-mode", "--agent", "--settings", "--plugin-dir"];
const CARRY_MANY: &[&str] = &["--add-dir", "--mcp-config"];
const CARRY_FLAG: &[&str] = &[
    "--dangerously-skip-permissions",
    "--allow-dangerously-skip-permissions",
    "--chrome",
    "--ide",
    "--verbose",
];

/// 다시 열 때 넘길 실행 인자만 고른다(argv[0] 은 건너뛴다) — `state.py:carry_args`.
pub fn carry_args<S: AsRef<str>>(argv: &[S]) -> Vec<String> {
    let argv: Vec<&str> = argv.iter().map(AsRef::as_ref).collect();
    let mut out = Vec::new();
    let mut i = 1;
    while i < argv.len() {
        let a = argv[i];
        let name = a.split('=').next().unwrap_or(a);
        if (a.contains('=') && (CARRY_ONE.contains(&name) || CARRY_MANY.contains(&name))) || CARRY_FLAG.contains(&a) {
            out.push(a.to_string());
        } else if CARRY_ONE.contains(&a) && i + 1 < argv.len() {
            out.push(a.to_string());
            out.push(argv[i + 1].to_string());
            i += 1;
        } else if CARRY_MANY.contains(&a) {
            out.push(a.to_string());
            while i + 1 < argv.len() && !argv[i + 1].starts_with('-') {
                out.push(argv[i + 1].to_string());
                i += 1;
            }
        }
        i += 1;
    }
    out
}

/// 종료된 세션을 같은 인자와 마지막 권한 모드·모델로 다시 여는 명령 — `state.py:resume_cmd`.
/// `join` 으로 인용 규칙을 고른다([`quote::join`] 은 플랫폼 규칙).
pub fn resume_cmd_with(rec: &Record, join: fn(&[String]) -> String) -> String {
    let mut args: Vec<String> = rec
        .get("args")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_default();
    for (flag, key) in [("--permission-mode", "permission_mode"), ("--model", "model")] {
        let Some(value) = non_empty_str(rec.get(key)) else { continue };
        let with_eq = format!("{flag}=");
        if let Some(j) = args.iter().position(|a| a == flag || a.starts_with(&with_eq)) {
            let n = if args[j].contains('=') { 1 } else { 2 };
            args.drain(j..(j + n).min(args.len()));
        }
        args.push(flag.to_string());
        args.push(value.to_string());
    }
    let sid = rec.get("session_id").and_then(Value::as_str).unwrap_or_default();
    let mut all = vec!["claude".to_string()];
    all.extend(args);
    all.push("--resume".to_string());
    all.push(sid.to_string());
    join(&all)
}

/// 이 플랫폼의 인용 규칙으로 만든 `resume_cmd`.
pub fn resume_cmd(rec: &Record) -> String {
    resume_cmd_with(rec, quote::join)
}

/// 응답을 마친 뒤 입력을 오래 기다릴 때 오는 Notification 은 승인 대기가 아니라 (입력) 대기다.
pub fn effective_state(state: &str, data: &Record) -> String {
    if state == "waiting" && data.get("notification_type").and_then(Value::as_str) == Some("idle_prompt") {
        "idle".to_string()
    } else {
        state.to_string()
    }
}

/// 기록의 pid 가 양의 정수면 그 값. Python 은 pid 가 None·문자열이면 `alive()` 가 참이다.
fn record_pid(rec: &Record) -> Option<u32> {
    rec.get("pid").and_then(Value::as_u64).filter(|&p| p > 0).and_then(|p| u32::try_from(p).ok())
}

fn record_ctime(rec: &Record) -> Option<f64> {
    rec.get("pid_ctime").and_then(Value::as_f64)
}

fn set(map: &mut Record, key: &str, value: impl Into<Value>) {
    map.insert(key.to_string(), value.into());
}

/// 서브에이전트 시작·도구 호출·승인 대기·종료를 세션 기록의 subagents 에 남긴다 — `state.py:track_subagent`.
fn track_subagent(rec: &mut Record, data: &Record, state: &str, now: f64, env: &dyn Env) {
    let aid = match data.get("agent_id") {
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
        None => return,
    };
    let event = data.get("hook_event_name").and_then(Value::as_str);
    if !rec.get("subagents").is_some_and(Value::is_object) {
        set(rec, "subagents", Map::new());
    }
    let Some(Value::Object(subs)) = rec.get_mut("subagents") else { return };
    if !subs.get(&aid).is_some_and(Value::is_object) {
        let mut fresh = Map::new();
        let kind = non_empty_str(data.get("agent_type")).unwrap_or("");
        set(&mut fresh, "type", kind);
        set(&mut fresh, "started", now);
        set(&mut fresh, "state", "running");
        subs.insert(aid.clone(), Value::Object(fresh));
    }
    let Some(Value::Object(sub)) = subs.get_mut(&aid) else { return };
    if !truthy(sub.get("description")) {
        let transcript = data.get("transcript_path").and_then(Value::as_str).unwrap_or("");
        let meta = if transcript.is_empty() { Map::new() } else { env.subagent_meta(transcript, &aid) };
        if !meta.is_empty() {
            set(sub, "description", meta.get("description").cloned().unwrap_or_else(|| "".into()));
            set(sub, "background", meta.get("requestShape").and_then(Value::as_str) == Some("background"));
        }
    }
    if event == Some("SubagentStop") {
        set(sub, "state", "done");
        set(sub, "ended", now);
        if let Some(msg) = non_empty_str(data.get("last_assistant_message")) {
            set(sub, "last_message", take_chars(&collapse_ws(msg), 200));
        }
    } else {
        // 끝난 서브에이전트도 남은 백그라운드 작업 때문에 다시 깨어나 도구를 쓸 수 있다
        sub.shift_remove("ended");
        set(sub, "state", if state == "waiting" { "waiting" } else { "running" });
        if let Some(tool) = data.get("tool_name").filter(|v| truthy(Some(v))) {
            set(sub, "tool", tool.clone());
        }
    }
    set(sub, "updated", now);
    // 끝난 지 1시간이 넘은 서브에이전트는 기록에서 지운다
    subs.retain(|_, v| {
        let done = v.get("state").and_then(Value::as_str) == Some("done");
        let ended = v.get("ended").and_then(Value::as_f64).unwrap_or(now);
        !(done && now - ended > 3600.0)
    });
}

/// hook 이벤트 하나를 세션 기록에 반영한다 — `state.py:update` 의 순수한 부분.
pub fn apply(mut rec: Record, state_arg: &str, data: &Record, now: f64, env: &dyn Env) -> Outcome {
    let sub_event = truthy(data.get("agent_id"));
    let mut state = effective_state(state_arg, data);
    let event = data.get("hook_event_name").and_then(Value::as_str);

    // 처음 보는 세션이거나 --resume 으로 다시 열린 세션이면 프로세스 정보를 새로 잡는다
    let alive = record_pid(&rec).is_none_or(|pid| env.alive(pid, record_ctime(&rec)));
    if !rec.contains_key("pid") || event == Some("SessionStart") || !alive {
        match env.find_claude() {
            Some(p) => {
                set(&mut rec, "pid", p.pid);
                match p.ctime {
                    Some(ct) => set(&mut rec, "pid_ctime", ct),
                    None => {
                        rec.shift_remove("pid_ctime");
                    }
                }
                set(&mut rec, "tty", p.tty.clone().map_or(Value::Null, Value::from));
                set(&mut rec, "headless", p.headless);
                set(&mut rec, "started", now);
                set(&mut rec, "args", carry_args(&p.argv));
            }
            None => {
                set(&mut rec, "pid", Value::Null);
                rec.shift_remove("pid_ctime");
                set(&mut rec, "tty", Value::Null);
                set(&mut rec, "headless", true);
                set(&mut rec, "started", now);
            }
        }
    } else if !rec.contains_key("args") && truthy(rec.get("pid")) {
        let argv = record_pid(&rec).map(|pid| env.argv(pid)).unwrap_or_default();
        set(&mut rec, "args", carry_args(&argv));
    }
    if let Some(t) = data.get("transcript_path").filter(|v| truthy(Some(v))) {
        set(&mut rec, "transcript_path", t.clone());
    }
    for key in ["permission_mode", "model"] {
        if let (false, Some(v)) = (sub_event, non_empty_str(data.get(key))) {
            set(&mut rec, key, v);
        }
    }
    let cwd = non_empty_str(data.get("cwd"))
        .or_else(|| non_empty_str(rec.get("cwd")))
        .map(str::to_string)
        .unwrap_or_else(|| env.cwd());
    let sid = data.get("session_id").cloned().unwrap_or(Value::Null);
    set(&mut rec, "session_id", sid);
    set(&mut rec, "cwd", cwd.as_str());
    set(&mut rec, "project", paths::basename(&cwd));
    set(&mut rec, "updated", now);

    // 같은 프로젝트에서 여러 세션을 띄워도 구분되도록 마지막 요청 문구를 주제로 남긴다
    let prompt = collapse_ws(data.get("prompt").and_then(Value::as_str).unwrap_or(""));
    let head = take_chars(&prompt, 40);
    let topic_changed =
        !prompt.is_empty() && !prompt.starts_with('<') && rec.get("topic").and_then(Value::as_str) != Some(head.as_str());
    if topic_changed {
        set(&mut rec, "topic", head);
    }

    if sub_event {
        track_subagent(&mut rec, data, &state, now, env);
        // 서브에이전트의 도구 호출은 세션 상태를 바꾸지 않는다 — 승인 요청만 세션도 승인 대기로 올리고,
        // 승인이 끝나 서브에이전트가 다시 움직이면 세션을 승인 요청 전 상태로 돌려놓는다
        let current = rec.get("state").and_then(Value::as_str).map(str::to_string);
        if state == "waiting" {
            if current.as_deref() != Some("waiting") {
                set(&mut rec, "state_before_sub_wait", current.as_deref().unwrap_or("working"));
            }
        } else if current.as_deref() == Some("waiting") && rec.contains_key("state_before_sub_wait") {
            let before = rec.shift_remove("state_before_sub_wait");
            state = before.as_ref().and_then(Value::as_str).unwrap_or("working").to_string();
        } else {
            state = current.unwrap_or_else(|| "working".to_string());
        }
    } else if state == "ended" {
        if let Some(Value::Object(subs)) = rec.get_mut("subagents") {
            for sub in subs.values_mut().filter_map(Value::as_object_mut) {
                if sub.get("state").and_then(Value::as_str) != Some("done") {
                    set(sub, "state", "done");
                    set(sub, "ended", now);
                }
            }
        }
    }

    if !sub_event {
        rec.shift_remove("state_before_sub_wait");
    }
    // 사용 한도에 걸려 응답이 끊긴 세션 — 보드가 계정을 자동 전환한 뒤 이어서 진행시킨다.
    // rate_limit 은 서버의 일시적인 요청 제한일 수도 있어 한도 문구일 때만 남기고,
    // 새 요청·도구 사용·응답 완료·다시 열기가 오면 세션이 다시 움직인 것이므로 지운다
    if event == Some("StopFailure") && data.get("error").and_then(Value::as_str) == Some("rate_limit") {
        let msg = collapse_ws(data.get("last_assistant_message").and_then(Value::as_str).unwrap_or(""));
        if limit_re().is_match(&msg) {
            let mut hit = Map::new();
            set(&mut hit, "at", now);
            set(&mut hit, "message", take_chars(&msg, 200));
            set(&mut rec, "limit_hit", hit);
        }
    } else if !sub_event
        && matches!(event, Some("UserPromptSubmit" | "PreToolUse" | "PostToolUse" | "Stop" | "SessionStart"))
    {
        rec.shift_remove("limit_hit");
    }
    let changed = rec.get("state").and_then(Value::as_str) != Some(state.as_str());
    if changed {
        set(&mut rec, "state", state.as_str());
        set(&mut rec, "since", now);
    }
    if state == "idle" {
        if let Some(msg) = non_empty_str(data.get("last_assistant_message")) {
            set(&mut rec, "last_message", take_chars(&collapse_ws(msg), 200));
        }
    }
    if state == "waiting" {
        if let Some(msg) = non_empty_str(data.get("message")) {
            set(&mut rec, "last_message", take_chars(msg, 200));
        }
    }

    let cmd = resume_cmd(&rec);
    set(&mut rec, "resume_cmd", cmd);

    // 터미널이 있는 세션만 제목을 바꾼다 — macOS 는 tty, Windows 는 headless 가 아닌 콘솔
    let has_terminal = truthy(rec.get("tty")) || rec.get("headless") == Some(&Value::Bool(false));
    let title = ((changed || topic_changed) && has_terminal).then(|| title_text(&state, &rec));
    Outcome { record: rec, title, remove_paused: event == Some("UserPromptSubmit") }
}

/// `"<아이콘> <project> · <topic 앞 20자>…"` — topic 이 20자 이하면 `…` 없이, 없으면 ` · ` 부분 없음.
pub fn title_text(state: &str, rec: &Record) -> String {
    let topic = rec.get("topic").and_then(Value::as_str).unwrap_or("");
    let tail = if topic.chars().count() > 20 {
        format!(" · {}…", take_chars(topic, 20))
    } else if !topic.is_empty() {
        format!(" · {topic}")
    } else {
        String::new()
    };
    let project = rec.get("project").and_then(Value::as_str).unwrap_or("");
    format!("{} {project}{tail}", icon(state))
}

fn limit_re() -> &'static regex::Regex {
    static RE: std::sync::LazyLock<regex::Regex> =
        std::sync::LazyLock::new(|| regex::Regex::new(r"(?i)hit your|usage limit").expect("limit regex"));
    &RE
}

/// 진짜 프로세스·파일을 쓰는 [`Env`].
pub struct SystemEnv;

impl Env for SystemEnv {
    fn find_claude(&self) -> Option<ClaudeProcess> {
        proc::find_claude()
    }
    fn alive(&self, pid: u32, ctime: Option<f64>) -> bool {
        proc::alive(pid, ctime)
    }
    fn argv(&self, pid: u32) -> Vec<String> {
        proc::command_line(pid).unwrap_or_default()
    }
    fn subagent_meta(&self, transcript: &str, agent_id: &str) -> Record {
        let base = transcript.strip_suffix(".jsonl").unwrap_or(transcript);
        let path = Path::new(base).join("subagents").join(format!("agent-{agent_id}.meta.json"));
        fileio::read_record(&path).unwrap_or_default()
    }
    fn cwd(&self) -> String {
        std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
    }
}

/// 종료된 지 2일이 넘은 세션 파일을 지운다 — 보드를 띄우지 않는 동안 쌓이는 것을 막는다 (`state.py:prune`).
/// `.lock`·`.paused`·`.mute` 는 짝인 `.json` 이 없으면 지운다. `alive` 는 기록을 받아 그 프로세스가 살아 있는지 답한다.
pub fn prune(sessions: &Path, now: f64, alive: impl Fn(&Record) -> bool) {
    let limit = now - 2.0 * 86400.0;
    let Ok(entries) = fs::read_dir(sessions) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if name.ends_with(".lock") || name.ends_with(".paused") || name.ends_with(".mute") {
            let base = if let Some(stem) = name.strip_suffix(".lock") {
                stem.strip_prefix('.').unwrap_or(stem).to_string()
            } else {
                name.rsplit_once('.').map(|(b, _)| b.to_string()).unwrap_or_default()
            };
            if !sessions.join(format!("{base}.json")).exists() {
                let _ = fs::remove_file(&path);
            }
            continue;
        }
        let old = entry
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .is_some_and(|d| d.as_secs_f64() < limit);
        if !old {
            continue;
        }
        if let Some(rec) = fileio::read_record(&path) {
            if !truthy(rec.get("pid")) || !alive(&rec) {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

/// `state <상태>` hook 의 파일 쪽 — stdin 으로 받은 입력을 `sessions` 폴더의 세션 기록에 반영한다.
/// 어떤 오류도 Claude Code 를 막으면 안 되므로 실패는 조용히 버린다(부르는 쪽이 패닉도 잡는다).
pub fn run(sessions: &Path, state_arg: &str, input: &[u8], env: &dyn Env) {
    let Ok(Value::Object(data)) = serde_json::from_slice::<Value>(input) else { return };
    let state = effective_state(state_arg, &data);
    // 경로 구분자가 든 id 는 sessions 밖에 쓰게 되므로 받지 않는다
    let Some(sid) = non_empty_str(data.get("session_id")).filter(|s| !s.contains(['/', '\\']) && *s != "..") else {
        return;
    };
    if fs::create_dir_all(sessions).is_err() {
        return;
    }
    let event = data.get("hook_event_name").and_then(Value::as_str);
    if state == "idle" && event == Some("SessionStart") {
        prune(sessions, crate::clock::now(), proc::record_alive);
    }
    let path = sessions.join(format!("{sid}.json"));
    // 서브에이전트가 여럿 동시에 돌면 hook 도 동시에 돌므로, 읽고 고쳐 쓰는 동안 세션 파일을 잠근다
    let Ok(lock) = OpenOptions::new().create(true).truncate(false).write(true).open(lock_path(sessions, sid)) else {
        return;
    };
    if lock.lock().is_err() {
        return;
    }
    let rec = fileio::read_record(&path).unwrap_or_default();
    let out = apply(rec, &state, &data, crate::clock::now(), env);
    if out.remove_paused {
        let _ = fs::remove_file(sessions.join(format!("{sid}.paused")));
    }
    let Ok(bytes) = serde_json::to_vec(&out.record) else { return };
    // 동시에 도는 hook 끼리 같은 임시 파일을 덮어쓰지 않도록 프로세스마다 다른 이름을 쓴다
    if fileio::write_atomic(&path, sid, &bytes).is_err() {
        return;
    }
    if let Some(title) = out.title {
        set_terminal_title(&out.record, &title);
    }
    drop(lock);
}

/// `.<sid>.lock`
pub fn lock_path(sessions: &Path, sid: &str) -> PathBuf {
    sessions.join(format!(".{sid}.lock"))
}

/// 세션의 터미널 제목을 바꾼다. Windows 는 그 Claude 프로세스의 콘솔에 붙어서, 그 밖은 tty 에 OSC 2 를 쓴다.
fn set_terminal_title(rec: &Record, title: &str) {
    #[cfg(windows)]
    if let Some(pid) = record_pid(rec) {
        crate::console::set_title(pid, title);
    }
    #[cfg(not(windows))]
    if let Some(tty) = non_empty_str(rec.get("tty")) {
        use std::io::Write;
        if let Ok(mut t) = OpenOptions::new().write(true).open(format!("/dev/{tty}")) {
            let _ = write!(t, "\x1b]2;{title}\x07");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fake {
        found: Option<ClaudeProcess>,
        alive: bool,
        meta: Record,
    }

    impl Env for Fake {
        fn find_claude(&self) -> Option<ClaudeProcess> {
            self.found.clone()
        }
        fn alive(&self, _pid: u32, _ctime: Option<f64>) -> bool {
            self.alive
        }
        fn argv(&self, _pid: u32) -> Vec<String> {
            vec!["claude".into(), "--verbose".into()]
        }
        fn subagent_meta(&self, _t: &str, _a: &str) -> Record {
            self.meta.clone()
        }
        fn cwd(&self) -> String {
            "/fallback/dir".into()
        }
    }

    fn fake() -> Fake {
        Fake {
            found: Some(ClaudeProcess { pid: 42, ctime: Some(1.5), tty: None, headless: false, argv: vec!["claude".into(), "--chrome".into()] }),
            alive: true,
            meta: Map::new(),
        }
    }

    fn obj(v: Value) -> Record {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn carry_args_rules() {
        let argv = [
            "/usr/bin/claude", "--model", "opus", "--permission-mode=plan", "--add-dir", "a", "b", "-c",
            "--verbose", "--print", "--mcp-config=x.json", "--settings", "s.json", "--verbose=1", "--agent",
        ];
        assert_eq!(
            carry_args(&argv),
            ["--model", "opus", "--permission-mode=plan", "--add-dir", "a", "b", "--verbose", "--mcp-config=x.json", "--settings", "s.json"]
        );
        assert!(carry_args(&["claude"]).is_empty());
        // 값이 없는 마지막 --agent 는 버린다(state.py 와 같다)
        assert!(carry_args(&["claude", "--agent"]).is_empty());
    }

    #[test]
    fn resume_cmd_replaces_mode_and_model() {
        let rec = obj(json!({
            "session_id": "sid-1",
            "args": ["--model=old", "--add-dir", "my dir", "--permission-mode", "default", "--chrome"],
            "permission_mode": "plan",
            "model": "claude-opus-5-5",
        }));
        assert_eq!(
            resume_cmd_with(&rec, quote::join_posix),
            "claude --add-dir 'my dir' --chrome --permission-mode plan --model claude-opus-5-5 --resume sid-1"
        );
        assert_eq!(
            resume_cmd_with(&rec, quote::join_windows),
            "claude --add-dir \"my dir\" --chrome --permission-mode plan --model claude-opus-5-5 --resume sid-1"
        );
        // 기록에 값이 없으면 인자를 그대로 둔다. 마지막에 값 없이 남은 --model 도 지운다
        let rec = obj(json!({"session_id": "s", "args": ["--model"], "model": "m"}));
        assert_eq!(resume_cmd_with(&rec, quote::join_posix), "claude --model m --resume s");
        let rec = obj(json!({"session_id": "s"}));
        assert_eq!(resume_cmd_with(&rec, quote::join_posix), "claude --resume s");
    }

    #[test]
    fn null_pid_is_not_probed_again() {
        // state.py 의 alive(None) 은 참 — pid 가 null 로 남은 기록은 SessionStart 가 아니면 다시 찾지 않는다
        let env = Fake { found: None, alive: false, meta: Map::new() };
        let data = obj(json!({"session_id": "s", "hook_event_name": "UserPromptSubmit", "cwd": "/w/p"}));
        let rec = obj(json!({"pid": null, "started": 1.0}));
        let out = apply(rec, "working", &data, 10.0, &env);
        assert_eq!(out.record["started"], 1.0);
        assert!(out.remove_paused);
        // 처음 보는 세션은 찾고, 못 찾으면 headless
        let out = apply(Map::new(), "working", &data, 10.0, &env);
        assert_eq!(out.record["pid"], Value::Null);
        assert_eq!(out.record["headless"], true);
        assert!(!out.record.contains_key("args"));
        assert_eq!(out.title, None);
    }

    #[test]
    fn dead_pid_is_replaced_and_args_filled() {
        let mut env = fake();
        let data = obj(json!({"session_id": "s", "hook_event_name": "PreToolUse"}));
        env.alive = false;
        let rec = obj(json!({"pid": 7, "pid_ctime": 9.0, "cwd": "/old/proj", "state": "working"}));
        let out = apply(rec, "working", &data, 5.0, &env);
        assert_eq!(out.record["pid"], 42);
        assert_eq!(out.record["pid_ctime"], 1.5);
        assert_eq!(out.record["args"], json!(["--chrome"]));
        assert_eq!(out.record["project"], "proj");
        // 살아 있는데 args 만 없으면 args 만 채운다
        env.alive = true;
        let rec = obj(json!({"pid": 7, "state": "working"}));
        let out = apply(rec, "working", &data, 5.0, &env);
        assert_eq!(out.record["pid"], 7);
        assert_eq!(out.record["args"], json!(["--verbose"]));
        assert_eq!(out.record["cwd"], "/fallback/dir");
    }

    #[test]
    fn subagent_meta_and_cleanup() {
        let mut env = fake();
        env.meta = obj(json!({"description": "코드 찾기", "requestShape": "background"}));
        let rec = obj(json!({
            "pid": 42, "args": [], "state": "working",
            "subagents": {"old": {"state": "done", "ended": 100.0}, "recent": {"state": "done", "ended": 4000.0}}
        }));
        let data = obj(json!({"session_id": "s", "hook_event_name": "SubagentStart", "agent_id": "a1",
                              "agent_type": "Explore", "transcript_path": "/t/x.jsonl", "model": "ignored"}));
        let out = apply(rec, "subagent", &data, 5000.0, &env);
        let subs = out.record["subagents"].as_object().unwrap();
        assert!(!subs.contains_key("old"));
        assert!(subs.contains_key("recent"));
        assert_eq!(subs["a1"]["description"], "코드 찾기");
        assert_eq!(subs["a1"]["background"], true);
        assert_eq!(out.record["state"], "working");
        assert!(!out.record.contains_key("model"));
        assert_eq!(out.title, None);
    }

    #[test]
    fn ended_closes_running_subagents() {
        let env = fake();
        let rec = obj(json!({"pid": 42, "args": [], "state": "idle", "headless": false,
                             "subagents": {"a": {"state": "running"}, "b": {"state": "done", "ended": 1.0}}}));
        let data = obj(json!({"session_id": "s", "hook_event_name": "SessionEnd", "cwd": "/w/p"}));
        let out = apply(rec, "ended", &data, 50.0, &env);
        assert_eq!(out.record["subagents"]["a"]["state"], "done");
        assert_eq!(out.record["subagents"]["a"]["ended"], 50.0);
        assert_eq!(out.record["subagents"]["b"]["ended"], 1.0);
        assert_eq!(out.title.as_deref(), Some("⚫ p"));
    }

    #[test]
    fn title_text_shapes() {
        let rec = obj(json!({"project": "proj", "topic": "가나다라마바사아자차카타파하가나다라마바사아"}));
        assert_eq!(title_text("working", &rec), "🔵 proj · 가나다라마바사아자차카타파하가나다라마바…");
        let rec = obj(json!({"project": "proj", "topic": "짧은 주제"}));
        assert_eq!(title_text("waiting", &rec), "🟠 proj · 짧은 주제");
        let rec = obj(json!({"project": "proj"}));
        assert_eq!(title_text("idle", &rec), "🟢 proj");
    }

    #[test]
    fn limit_hit_needs_limit_words() {
        let env = fake();
        let base = obj(json!({"pid": 42, "args": [], "state": "working"}));
        let data = obj(json!({"session_id": "s", "hook_event_name": "StopFailure", "error": "rate_limit",
                              "last_assistant_message": "API Error: overloaded"}));
        let out = apply(base.clone(), "idle", &data, 1.0, &env);
        assert!(!out.record.contains_key("limit_hit"));
        let data = obj(json!({"session_id": "s", "hook_event_name": "StopFailure", "error": "rate_limit",
                              "last_assistant_message": "Claude USAGE LIMIT reached"}));
        let out = apply(base, "idle", &data, 1.0, &env);
        assert_eq!(out.record["limit_hit"]["message"], "Claude USAGE LIMIT reached");
    }

    #[test]
    fn prune_removes_old_dead_and_orphans() {
        let dir = std::env::temp_dir().join(format!("cb-prune-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(3 * 86400);
        let write = |name: &str, body: &str, aged: bool| {
            let p = dir.join(name);
            fs::write(&p, body).unwrap();
            if aged {
                fs::File::options().write(true).open(&p).unwrap().set_modified(old).unwrap();
            }
        };
        write("dead.json", r#"{"pid": 111}"#, true);
        write("nopid.json", r#"{"state": "ended"}"#, true);
        write("live.json", r#"{"pid": 222}"#, true);
        write("fresh.json", r#"{"pid": 111}"#, false);
        write("broken.json", "{", true);
        write(".live.lock", "", false);
        write(".gone.lock", "", false);
        write("gone.paused", "{}", false);
        write("gone.mute", "", false);
        write("fresh.mute", "", false);
        prune(&dir, crate::clock::now(), |rec| rec["pid"] == 222);
        let mut left: Vec<String> =
            fs::read_dir(&dir).unwrap().flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        left.sort();
        assert_eq!(left, [".live.lock", "broken.json", "fresh.json", "fresh.mute", "live.json"]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn run_writes_record_and_removes_paused() {
        let dir = std::env::temp_dir().join(format!("cb-run-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("s1.paused"), "{}").unwrap();
        let input = br#"{"session_id":"s1","hook_event_name":"UserPromptSubmit","cwd":"/w/proj","prompt":"hello"}"#;
        run(&dir, "working", input, &Fake { found: None, alive: true, meta: Map::new() });
        let rec = fileio::read_record(&dir.join("s1.json")).unwrap();
        assert_eq!(rec["state"], "working");
        assert_eq!(rec["topic"], "hello");
        assert!(!dir.join("s1.paused").exists());
        assert!(dir.join(".s1.lock").exists());
        // 깨진 입력·session_id 없음·경로가 든 id 는 아무것도 쓰지 않는다
        run(&dir, "working", b"not json", &fake());
        run(&dir, "working", br#"{"session_id":"../x"}"#, &fake());
        run(&dir, "working", br#"{"cwd":"/a"}"#, &fake());
        let n = fs::read_dir(&dir).unwrap().count();
        assert_eq!(n, 2);
        fs::remove_dir_all(&dir).unwrap();
    }
}
