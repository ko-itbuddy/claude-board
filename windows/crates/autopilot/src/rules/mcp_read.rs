//! (청크 MCPRead a/2 — 원본 1~145줄. PORTED.json 항목은 마지막 청크가 더한다)
//! MCP 읽기 세션 — 내장하지 않은 서비스의 링크는 사용자가 등록한 MCP 서버 하나로 읽는
//! 짧은 claude 세션이 메타 형식 JSON 하나로 답하게 한다. 이 파일은 그 준비 꼴만 둔다:
//! 서버 목록(apMCPServers), 그 서버 한 줄 고정 설정(apMCPConfigJSON), 허용·거부 도구 꼴,
//! 세션 설정 파일(apMCPSettingsJSON), claude 인자(apMCPReadArgs).
//! 원본: `Sources/AutopilotRules/MCPRead.swift`(1~145줄)

use crate::rules::connections::url_host;
use crate::rules::formats::grapheme_len;
use serde_json::{Map, Value};
use std::collections::HashSet;

/// 사용자 MCP 설정에서 옮겨 담은 서버 한 줄. 원본: `APMCPServer`
#[derive(Clone, Debug, PartialEq)]
pub struct McpServer {
    /// 도구 이름 `mcp__<이름>__<도구>` 와 파일 이름에 쓴다
    pub name: String,
    /// stdio·http·sse
    pub transport: String,
    /// local·project·user(Claude Code 의 우선 차례)
    pub scope: String,
    /// 사용자 설정의 그 서버 한 줄(JSON 객체 그대로, 원본은 키 순으로 정리해 둔 바이트)
    pub config: Value,
    /// 쓸 수 있는가 — 저장소 .mcp.json(project) 서버는 사용자가 그 프로젝트에서 승인했을 때만
    pub approved: bool,
    /// 승인되지 않은 까닭
    pub note: String,
    /// 화면에 보일 띄울 것: stdio 는 명령(앞 200 글자), http·sse 는 호스트
    pub target: String,
}

// 원본 init 의 기본값 — approved 만 true 이므로 유도(Derive)로 두지 않는다
impl Default for McpServer {
    fn default() -> Self {
        Self {
            name: String::new(),
            transport: String::new(),
            scope: String::new(),
            config: Value::Object(Map::new()),
            approved: true,
            note: String::new(),
            target: String::new(),
        }
    }
}

/// 서버 이름 꼴 — 도구 이름과 파일 이름에 쓰므로 좁게.
/// 원본: `apMCPServerNameOK`(정규식 `^[A-Za-z0-9_-]{1,64}$`, reading.rs 의 비공개 이식과 같은 잰)
pub fn mcp_server_name_ok(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// JSON 바이트를 객체로. 너무 크거나(32MiB 한도) 객체가 아니면 빈 객체 —
/// 원본 `count <= 32 << 20 ? (try? JSONSerialization…) as? [String:Any] : nil ?? [:]`
fn json_object(bytes: Option<&[u8]>) -> Map<String, Value> {
    let Some(b) = bytes else {
        return Map::new();
    };
    if b.len() > 32 << 20 {
        return Map::new();
    }
    serde_json::from_slice::<Value>(b)
        .ok()
        .and_then(|v| match v {
            Value::Object(m) => Some(m),
            _ => None,
        })
        .unwrap_or_default()
}

/// 배열 가운데 글자인 낱목만(원본 `compactMap { $0 as? String }`). 원본: 지역 도움 함수 없이 곧바로 Set
fn string_set(values: &[Value]) -> HashSet<String> {
    values.iter().filter_map(Value::as_str).map(str::to_string).collect()
}

/// JSON 을 키 순(바이트 순)으로 다시 심는다 — 원본이 `JSONSerialization` `.sortedKeys` 로 내보내는 것과 같은 모양.
/// 원본의 키 비교는 유니코드 글자 순서이고, 여기는 바이트 순서이나 테스트의 키는 ASCII 라 같다
fn sorted_value(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                out.insert(k.clone(), sorted_value(&m[k]));
            }
            Value::Object(out)
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted_value).collect()),
        other => other.clone(),
    }
}

/// 앞 `n` 글자(그래핀 군집, meta.rs clamp_graphemes 와 같은 잰)만 자른다. 원본 `String(s.prefix(n))`
fn grapheme_prefix(t: &str, n: usize) -> String {
    if grapheme_len(t) <= n {
        return t.to_string();
    }
    let bounds: Vec<usize> = t.char_indices().map(|(i, _)| i).chain(std::iter::once(t.len())).collect();
    let (mut lo, mut hi) = (0, bounds.len() - 1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if grapheme_len(&t[..bounds[mid]]) <= n {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    t[..bounds[lo]].to_string()
}

/// 사용자 MCP 설정에서 서버 목록 — ~/.claude.json 의 `projects[<작업 경로>].mcpServers`(local),
/// `<작업 경로>/.mcp.json`(project), ~/.claude.json 의 `mcpServers`(user) 차례로, 같은 이름은 앞의 것.
/// 이름·꼴이 틀린 줄은 버린다. 파일은 읽기만 한다(어댑터).
/// 저장소 .mcp.json 의 서버는 저장소가 정한 명령이라, 그 프로젝트 항목의 `enableAllProjectMcpServers` 가
/// true 이거나 `enabledMcpjsonServers` 에 이름이 있을 때만 approved(disabledMcpjsonServers 에 있으면 아님).
/// 승인되지 않은 서버는 목록에 까닭과 함께 남고 쓰지 않는다. 원본: `apMCPServers`
pub fn mcp_servers(
    claude_json: Option<&[u8]>,
    project_mcp: Option<&[u8]>,
    cwd: Option<&str>,
) -> Vec<McpServer> {
    let cj = json_object(claude_json);
    let pm = json_object(project_mcp);
    let proj: Map<String, Value> = match cwd {
        Some(path) => cj
            .get("projects")
            .and_then(Value::as_object)
            .and_then(|m| m.get(path))
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default(),
        None => Map::new(),
    };
    let all_project = proj.get("enableAllProjectMcpServers").and_then(Value::as_bool) == Some(true);
    let enabled = proj
        .get("enabledMcpjsonServers")
        .and_then(Value::as_array)
        .map(|a| string_set(a))
        .unwrap_or_default();
    let disabled = proj
        .get("disabledMcpjsonServers")
        .and_then(Value::as_array)
        .map(|a| string_set(a))
        .unwrap_or_default();
    let mut groups: Vec<(&str, Map<String, Value>)> = Vec::new();
    if let Some(m) = proj.get("mcpServers").and_then(Value::as_object) {
        groups.push(("local", m.clone()));
    }
    if let Some(m) = pm.get("mcpServers").and_then(Value::as_object) {
        groups.push(("project", m.clone()));
    }
    if let Some(m) = cj.get("mcpServers").and_then(Value::as_object) {
        groups.push(("user", m.clone()));
    }
    let mut out: Vec<McpServer> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (scope, m) in groups {
        let mut names: Vec<&String> = m.keys().collect();
        names.sort(); // 원본도 이름 순(유니코드 글자 순)이고, 테스트의 이름은 ASCII 라 같다
        for name in names {
            let name = name.as_str();
            if !mcp_server_name_ok(name) {
                continue;
            }
            if seen.contains(name) {
                continue;
            }
            let Some(c) = m.get(name).and_then(Value::as_object) else {
                continue;
            };
            // 꼴 글자(type) 가 없으면 command 가 있을 때 stdio 로 본다
            let kind = c
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_lowercase)
                .unwrap_or_else(|| {
                    if c.contains_key("command") {
                        "stdio".to_string()
                    } else {
                        String::new()
                    }
                });
            let (command, url): (Option<&str>, Option<&str>) = match kind.as_str() {
                "stdio" => {
                    let Some(cmd) = c.get("command").and_then(Value::as_str).filter(|s| !s.is_empty())
                    else {
                        continue;
                    };
                    (Some(cmd), None)
                }
                "http" | "sse" => {
                    let Some(u) = c.get("url").and_then(Value::as_str) else {
                        continue;
                    };
                    if !(u.to_lowercase().starts_with("https://")
                        || u.starts_with("http://127.0.0.1")
                        || u.starts_with("http://localhost"))
                    {
                        continue;
                    }
                    (None, Some(u))
                }
                _ => continue,
            };
            let mut server = McpServer {
                name: name.to_string(),
                transport: kind,
                scope: scope.to_string(),
                config: sorted_value(&Value::Object(c.clone())),
                approved: true,
                note: String::new(),
                target: match (command, url) {
                    (Some(cmd), _) => grapheme_prefix(cmd, 200),
                    (None, Some(u)) => url_host(u),
                    (None, None) => String::new(), // 꼴을 걸러 이미 도달하지 않는다
                },
            };
            if scope == "project" {
                if disabled.contains(name) {
                    server.approved = false;
                    server.note = "이 프로젝트에서 사용자가 끈 서버입니다(disabledMcpjsonServers)".to_string();
                } else if !(all_project || enabled.contains(name)) {
                    server.approved = false;
                    server.note = "저장소 .mcp.json 의 서버라 Claude Code 에서 이 프로젝트의 서버로 승인한 뒤에만 씁니다(저장소가 정한 명령을 띄우므로)".to_string();
                }
            }
            seen.insert(name.to_string());
            out.push(server);
        }
    }
    out
}

/// 세션의 고정 MCP 설정 — 그 서버 한 줄만(사용자 설정의 값 그대로 옮겨 적는다. env·머리에 비밀이
/// 있을 수 있어 엔진 전용 폴더 0600 에 쓰고 끝나면 지운다). 원본: `apMCPConfigJSON`
pub fn mcp_config_json(s: &McpServer) -> Value {
    let mut inner = Map::new();
    inner.insert(s.name.clone(), s.config.clone());
    let mut root = Map::new();
    root.insert("mcpServers".to_string(), Value::Object(inner));
    Value::Object(root)
}

/// 읽기 꼴 동사와 쓰기 꼴 동사 — 이름 맨 앞, `_`·`-`·`.` 뒤, 낙타 꼴(첫 글자 대문자) 자리에 올 때 그 동사로 본다
pub const MCP_READ_VERBS: [&str; 12] = [
    "get", "list", "search", "read", "fetch", "query", "find", "view", "describe", "lookup", "retrieve", "show",
];
pub const MCP_WRITE_VERBS: [&str; 72] = [
    "create", "update", "delete", "remove", "write", "edit", "post", "put", "patch", "add", "set", "send", "move", "archive",
    "execute", "run", "transition", "upload", "insert", "publish", "merge", "assign", "invite", "share", "rename", "replace",
    "import", "restore", "trash", "close", "reopen", "approve", "submit", "revoke", "grant", "install", "deploy", "push",
    "save", "modify", "drop", "clear", "reset", "trigger", "dispatch", "invoke", "upsert", "append", "attach", "complete",
    "resolve", "cancel", "enable", "disable", "apply", "commit", "duplicate", "copy", "fork", "lock", "unlock", "subscribe",
    "unsubscribe", "transfer", "schedule", "start", "stop", "sync", "generate", "mark", "react", "vote",
];
/// 내장 도구 이름(--tools "" 로 이미 꺼지지만 거부 목록에도 둔다)
pub const MCP_BUILTIN_DENY: [&str; 22] = [
    "Bash", "Edit", "Write", "MultiEdit", "NotebookEdit", "Read", "Glob", "Grep", "WebFetch", "WebSearch", "Task", "Agent",
    "Skill", "TodoWrite", "BashOutput", "KillShell", "TaskStop", "Monitor", "SendMessage", "ToolSearch", "SlashCommand", "LSP",
];

/// 쓰기 꼴 — 이름 어디에 와도(맨 앞, `_`·`-`·`.` 뒤, 낙타 꼴 대문자 자리) 거부한다. 원본: `shapes`
fn shapes(prefix: &str, verb: &str) -> Vec<String> {
    let mut cap = String::with_capacity(verb.len());
    let mut chars = verb.chars();
    if let Some(first) = chars.next() {
        cap.extend(first.to_uppercase());
        cap.push_str(chars.as_str());
    }
    vec![
        format!("{prefix}{verb}*"),
        format!("{prefix}*_{verb}*"),
        format!("{prefix}*-{verb}*"),
        format!("{prefix}*.{verb}*"),
        format!("{prefix}*{cap}*"),
    ]
}

/// 읽기 꼴 앞에 와도 되는 정해 둔 접두어 — 없음, Notion 공식 서버의 API-·API_, 서버 이름 자신(wiki_get… · wiki-get…).
/// 원본: `apMCPAllowPrefixes`
pub fn mcp_allow_prefixes(server: &str) -> Vec<String> {
    vec![
        String::new(),
        "API-".to_string(),
        "API_".to_string(),
        format!("{server}_"),
        format!("{server}-"),
    ]
}

/// 허용은 읽기 동사가 이름의 첫 낱말(정해 둔 접두어 하나 뒤까지)일 때만 — 가운데에 get·read 가 낀 쓰기 도구(delete_and_get_x)는
/// 허용 꼴에 맞지 않는다(거부 꼴은 그대로 이긴다). 원본: `apMCPAllowRules`
pub fn mcp_allow_rules(server: &str) -> Vec<String> {
    MCP_READ_VERBS
        .iter()
        .flat_map(|v| {
            mcp_allow_prefixes(server)
                .iter()
                .map(move |p| format!("mcp__{server}__{p}{v}*"))
                .collect::<Vec<String>>()
        })
        .collect()
}

/// 화면에 보일 허용 꼴 한 줄(준비 화면의 MCP 읽기 줄). 원본: `apMCPAllowSummary`
pub fn mcp_allow_summary(server: &str) -> String {
    "허용 도구: 이름이 ".to_string()
        + &MCP_READ_VERBS.join("·")
        + " 로 시작하는 것만(앞에 API- 나 "
        + server
        + "_ 가 붙어도 됨). 이름 어디에든 create·update·delete·edit·add 같은 쓰기 꼴이 있으면 거부. 내장 도구(Bash·파일·웹)는 없음"
}

/// 거부 꼴 — 내장 도구, 쓰기 꼴 동사 전부, 그리고 대문자 꼴 DELETE·CREATE·UPDATE·WRITE 이름. 원본: `apMCPDenyRules`
pub fn mcp_deny_rules(server: &str) -> Vec<String> {
    MCP_BUILTIN_DENY
        .iter()
        .map(|t| t.to_string())
        .chain(MCP_WRITE_VERBS.iter().flat_map(|v| shapes(&format!("mcp__{server}__"), v)))
        .chain([
            format!("mcp__{server}__*DELETE*"),
            format!("mcp__{server}__*CREATE*"),
            format!("mcp__{server}__*UPDATE*"),
            format!("mcp__{server}__*WRITE*"),
        ])
        .collect()
}

/// 글롭 맞추기(시험과 보드 미리 보기용) — `*` 는 아무 글자(빈 글자 포함).
/// 원본 주석은 "대소문자를 가린다" 라 썼지만 Foundation `.regularExpression` 이 실제로는 대소문자를 구별하므로
/// 여기도 문자 그대로 비교해 구별한다(내부 근거: 꼴이 CamelCase 형 `*Create*` 과 지우기 꼴 `*DELETE*` 를 나눠 둔다).
/// 원본은 정규식으로 푼 것과 뜻이 같다(메타 문자도 낱글자로 비교). 원본: `apGlob`
pub fn mcp_glob(pattern: &str, s: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return s == pattern; // * 가 없으면 글자 그대로 (원본은 정규식 이스케이프로 같은 뜻)
    }
    let Some(body) = s.strip_prefix(parts[0]) else {
        return false;
    };
    let Some(body) = body.strip_suffix(parts[parts.len() - 1]) else {
        return false;
    };
    let mut rest = body;
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    true
}

/// 그 서버의 도구 이름 하나가 이 세션에서 쓰일 수 있는가(허용 꼴에 맞고 거부 꼴에 맞지 않음, 다른 서버 도구는 늘 아님).
/// 원본: `apMCPToolAllowed`
pub fn mcp_tool_allowed(tool: &str, server: &str) -> bool {
    let full = if tool.starts_with("mcp__") {
        tool.to_string()
    } else {
        format!("mcp__{server}__{tool}")
    };
    if !full.starts_with(&format!("mcp__{server}__")) {
        return false;
    }
    if mcp_deny_rules(server).iter().any(|r| mcp_glob(r, &full)) {
        return false;
    }
    mcp_allow_rules(server).iter().any(|r| mcp_glob(r, &full))
}

/// 세션 설정 파일(--settings) — hook 을 모두 끄고, 같은 허용·거부 목록을 한 번 더(명령줄과 겹쳐도 거부가 이긴다).
/// 원본: `apMCPSettingsJSON`
pub fn mcp_settings_json(server: &str) -> Value {
    let mut permissions = Map::new();
    permissions.insert("defaultMode".to_string(), Value::String("dontAsk".to_string()));
    permissions.insert(
        "allow".to_string(),
        Value::Array(mcp_allow_rules(server).into_iter().map(Value::String).collect()),
    );
    permissions.insert(
        "deny".to_string(),
        Value::Array(mcp_deny_rules(server).into_iter().map(Value::String).collect()),
    );
    let mut root = Map::new();
    root.insert("disableAllHooks".to_string(), Value::Bool(true));
    root.insert("permissions".to_string(), Value::Object(permissions));
    Value::Object(root)
}

/// 세션 모델과 비용 상한·시간 상한(시간은 엔진이 끊는다). 원본: `apMCPReadModel`·`apMCPReadBudgetUSD`·`apMCPReadMaxSec`
pub const MCP_READ_MODEL: &str = "haiku";
pub const MCP_READ_BUDGET_USD: f64 = 0.30;
pub const MCP_READ_MAX_SEC: f64 = 180.0;

/// claude 인자 — 질문은 표준 입력으로(--disallowedTools 가 뒤 글을 삼키므로, 4장 실측).
/// 원본의 기본 인수(model·budgetUSD)는 호출한 곳에서 위 상수를 넣는다. 원본: `apMCPReadArgs`
pub fn mcp_read_args(server: &str, mcp_config: &str, settings: &str, model: &str, budget_usd: f64) -> Vec<String> {
    let allow = mcp_allow_rules(server).join(" ");
    let deny = mcp_deny_rules(server).join(" ");
    vec![
        "-p".to_string(),
        "--output-format".to_string(),
        "json".to_string(),
        "--model".to_string(),
        model.to_string(),
        "--max-budget-usd".to_string(),
        format!("{budget_usd:.2}"),
        "--no-session-persistence".to_string(),
        "--disable-slash-commands".to_string(),
        "--restricted".to_string(),
        "--strict-mcp-config".to_string(),
        "--mcp-config".to_string(),
        mcp_config.to_string(),
        "--tools".to_string(),
        String::new(),
        "--permission-mode".to_string(),
        "dontAsk".to_string(),
        "--permission-prompts".to_string(),
        "none".to_string(),
        "--settings".to_string(),
        settings.to_string(),
        "--allowedTools".to_string(),
        allow,
        "--disallowedTools".to_string(),
        deny,
    ]
}

// R-MCPRead-b 가 여기서 이어 쓴다

