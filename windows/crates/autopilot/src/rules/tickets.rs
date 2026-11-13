//! 출발 때 한꺼번에 만드는 Jira 티켓 나무 — 계획(보드가 씀)·상태(엔진이 씀)·요청 꼴. 원본: `Sources/AutopilotRules/Tickets.swift`,
//! 이 청은 1–293 줄이다(계획 12–103 · 상태 105–203 · 요청 꼴 205–293). 원본 295–528(만들기와 행·실행 중 새 티켓)과 시험
//! make_all·retry_left·uncertain·blocked·waits_and_guards·types_and_keys·outbox·post_ticket 은 R-Tickets-b 청이다.
//! 4단계 ⑩-3(2026-10-30, 설계 2장 끝·5장 6절). 출발 확인의 "만들 티켓"(에픽 아래 새 스토리·새 작업, 옮긴 작업의 부모 바꾸기)은
//! 보드가 계획 파일(tickets-plan.json)로 쓰고, 출발을 누르면 보드가 앱 안 엔진의 tickets 명령을 불러 만든다(보드는 HTTP 를 하지 않는다, ⑩-1).
//! 만든 키는 엔진이 상태 파일(tickets.json)에 남기고 보드가 그 키를 대기열(queue.json 의 story·tickets)에 넣은 뒤 출발한다.
//! 같은 것을 두 번 만들지 않게: 요청을 보내기 전에 항목을 "만드는 중"(creating)으로 쓰고, 받은 키를 바로 적고, 잠금 안에서 creating 을 보면
//! 앞 만들기가 끝나지 못하고 죽은 것이라 "확인 필요"(uncertain)로 바꾸고 사람이 확인하기 전에는 다시 만들지 않는다(⑩-1 올릴 글과 같은 꼴).
//! 실행 중에 새로 필요해진 티켓은 출발 카드 답대로 임시 번호(TMP-<단위>-<차례>)로 진행하고, 단위 판정 뒤 엔진이 올릴 글(kind ticket)로 남겨
//! 검수 뒤에 사람이 만든다(게시 방식이 자동으로 올림이어도 늘 검수 뒤 — 실행 중 티켓 만들기 요청 0).
//!
//! 이식된 선언(원본 순서): `TicketKind`(APTicketKind) · `TicketEntry`(APTicketEntry) · `TicketPlan`(APTicketPlan)·`project` · 상수 넷 ·
//! `ticket_entry_id_ok`(apTicketEntryIDOK) · `ticket_plan_json`(apTicketPlanJSON) · `ticket_plan_parse`(apTicketPlanParse) ·
//! `TicketState`(APTicketState) · `Attempt`(APAttempt) · `TicketRecord`(APTicketRecord) · `TicketBook`(APTicketBook) ·
//! `ticket_book_json`(apTicketBookJSON) · `ticket_book_parse`(apTicketBookParse) · `ticket_keys`(apTicketKeys) ·
//! `ticket_placeholder`(`apTicketPlaceholderPattern` 규칙 + `apTicketPlaceholder`) · `is_ticket_placeholder`(apIsTicketPlaceholder) ·
//! `unmade_keys`(apUnmadeKeys) · `tickets_left`(apTicketsLeft) · `IssueType`(APIssueType) · `jira_project_request`(apJiraProjectRequest) ·
//! `jira_issue_types`(apJiraIssueTypes) · `pick_issue_type`(apPickIssueType) · `ticket_doc`(apTicketDoc) ·
//! `jira_create_issue_request`(apJiraCreateIssueRequest) · `jira_parent_request`(apJiraParentRequest) ·
//! `created_issue_key`(apCreatedIssueKey) · `ticket_fail_note`(apTicketFailNote).
//!
//! Swift ↔ Rust 로 달라진 점:
//! - 정규식(`apMatch`)은 판별자로 옮겼다: `issue_key_ok` 는 Outbox.swift 38줄 `apIssueKeyPattern`, `temp_ticket_ok` 는 이 원본의
//!   455줄 `apTempTicketPattern`(선언은 b 청이지만 a 청의 `unmade_keys` 가 쓴다)을 옮겼다. Outbox 는 아직 이식 전이라 `fail_note`
//!   (Outbox.swift 422–435 줄 `apFailNote`)도 여기에 둔다 — R-게시 청이 outbox.rs 를 심을 때 같은 것을 쓰게 한다.
//! - 제목·단위 제목은 모델·바깥이 쓴 글일 수 있다(`Notify` apNotifyClean 원칙) — 비밀 가리기·다듬기·멘션 무력화(`notify_clean_n`), 상한.
//! - Swift `prefix(40)` 는 글자(Character) 수다 — grapheme 으로 센 `take` 근사(`grapheme_len`). Swift `Int(...)` 파싱은 `i64`
//!   (부호 표도 받는다).
//! - 키 순서: serde_json 은 `preserve_order` 라 — Swift 사전은 순서가 없다. 요청 본문은 `body` 에서 키를 정렬한다
//!   (JSONSerialization `[.sortedKeys, .withoutEscapingSlashes]`, `/` 는 이스케이프하지 않는다).
//! - `trimmingCharacters(in: .whitespaces)` 는 Zs 범주와 탭(`trim_ws`)이다 — `char::is_whitespace` 가 아니다.
//! - 상태 파일의 수 칸은 `NSNumber` 규칙(불리언 1·0)으로 읽는다(`num_f64`·`int_value`·`int_opt`).
//! - `TicketBook.entries` 는 순서가 없는 Swift Dictionary — BTreeMap 으로 둔다(시험은 키를 정렬해 견준다).
//! - 기본 인자(`apTicketDoc` 의 temp·why·task)는 Rust 에 없다 — 모두 받는다.

use std::collections::{BTreeMap, HashSet};

use crate::rules::connections::{Auth, HttpRequest, HttpResponse};
use crate::rules::formats::{adf, doc_clamp, doc_text_n, grapheme_len, Block, Doc, DOC_MARKER, Span};
use crate::rules::notify_rules::notify_clean_n;
use serde_json::{json, Map, Value};

// MARK: - 계획(보드가 씀)

/// 티켓 종류. 원본: `APTicketKind`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TicketKind {
    Story,
    Task,
    Move,
}

impl TicketKind {
    /// Swift rawValue
    pub fn raw_value(&self) -> &'static str {
        match self {
            TicketKind::Story => "story",
            TicketKind::Task => "task",
            TicketKind::Move => "move",
        }
    }

    /// Swift `APTicketKind(rawValue:)`
    pub fn from_raw(s: &str) -> Option<TicketKind> {
        match s {
            "story" => Some(TicketKind::Story),
            "task" => Some(TicketKind::Task),
            "move" => Some(TicketKind::Move),
            _ => None,
        }
    }
}

/// 만들 티켓 하나 — id 는 출발 준비 안에서 바뀌지 않는 이름(단위 위치가 아니라):
/// story:<보드 스토리 id>·task:<단위 id>·move:<작업 키>. 원본: `APTicketEntry`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TicketEntry {
    pub id: String,
    pub kind: TicketKind,
    pub title: String,
    /// 에픽 키·이미 있는 스토리 키, 또는 같은 계획의 story:… 항목
    pub parent: String,
    /// move: 부모를 바꿀 이미 있는 작업 키
    pub key: String,
    /// 본문에 넣을 단위 제목(스토리는 그 스토리의 단위들, 작업은 그 단위 하나)
    pub units: Vec<String>,
}

impl Default for TicketEntry {
    /// Swift 기본값(kind 는 .task)
    fn default() -> Self {
        TicketEntry { id: String::new(), kind: TicketKind::Task, title: String::new(), parent: String::new(), key: String::new(), units: Vec::new() }
    }
}

/// 만들 티켓 계획(보드가 씀). 원본: `APTicketPlan`
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TicketPlan {
    /// 출발 준비 id(출발하면 실행 id)
    pub run: String,
    pub epic: String,
    /// 프로젝트 설정 tickets.issue_types.story(비우면 응답의 이슈 종류에서 고름)
    pub story_type: String,
    /// 프로젝트 설정 tickets.issue_types.task(비우면 하위 작업 종류에서 고름)
    pub task_type: String,
    pub entries: Vec<TicketEntry>,
}

impl TicketPlan {
    /// 새 티켓의 프로젝트 — 에픽 키 앞(SHOP-10 → SHOP). Swift `split(separator:)` 는 빈 조각을 버린다
    pub fn project(&self) -> String {
        self.epic.split('-').find(|x| !x.is_empty()).unwrap_or("").to_string()
    }
}

/// 원본: `apTicketPlanFormat`
pub const TICKET_PLAN_FORMAT: &str = "claude-board-tickets-plan";
/// 원본: `apTicketBookFormat`
pub const TICKET_BOOK_FORMAT: &str = "claude-board-tickets";
/// Jira 요약 칸 255자 안. 원본: `apTicketTitleMax`
pub const TICKET_TITLE_MAX: usize = 240;
/// 한 번에 만들 티켓 수 상한. 원본: `apTicketEntryMax`
pub const TICKET_ENTRY_MAX: usize = 200;

/// 티켓 항목 id 꼴 — `^(story:<…>|task:<…>|move:<이슈 키>)$` 를 판별자로 옮긴 것. 원본: `apTicketEntryIDOK`
pub fn ticket_entry_id_ok(id: &str) -> bool {
    let Some((head, tail)) = id.split_once(':') else { return false };
    match head {
        "story" | "task" => {
            (1..=64).contains(&tail.chars().count()) && tail.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        }
        "move" => issue_key_ok(tail),
        _ => false,
    }
}

/// 계획 파일 값(보드가 쓴다). Swift 사전은 순서가 없다 — 요청 본문은 `body` 에서 키를 정렬해 내보낸다. 원본: `apTicketPlanJSON`
pub fn ticket_plan_json(p: &TicketPlan) -> Value {
    let entries: Vec<Value> = p
        .entries
        .iter()
        .map(|e| {
            let mut o = json!({"id": e.id, "kind": e.kind.raw_value(), "title": e.title, "parent": e.parent, "units": e.units});
            if !e.key.is_empty() {
                o["key"] = json!(e.key);
            }
            o
        })
        .collect();
    let mut d = json!({"format": TICKET_PLAN_FORMAT, "version": 1, "run": p.run, "epic": p.epic, "entries": entries});
    if !p.story_type.is_empty() || !p.task_type.is_empty() {
        d["issue_types"] = json!({"story": p.story_type, "task": p.task_type});
    }
    d
}

/// 계획 파일 → 계획과 고칠 것. 고칠 것이 하나라도 있으면 아무것도 만들지 않는다(요청 0). 원본: `apTicketPlanParse`
pub fn ticket_plan_parse(data: Option<&[u8]>) -> (TicketPlan, Vec<String>) {
    let mut p = TicketPlan::default();
    let mut probs: Vec<String> = Vec::new();
    // 읽는다: JSONSerialization, 1MiB, 그리고 format 표지. 어느 하나라도 아니면 "읽지 못했다" 하나만
    let o: Value = match data {
        Some(d) if d.len() <= 1 << 20 => serde_json::from_slice(d).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    if o.get("format").and_then(Value::as_str) != Some(TICKET_PLAN_FORMAT) {
        return (p, vec!["만들 티켓 계획 파일을 읽지 못했습니다".to_string()]);
    }
    p.run = o.get("run").and_then(Value::as_str).unwrap_or("").to_string();
    p.epic = trim_ws(o.get("epic").and_then(Value::as_str).unwrap_or("")).to_string();
    if let Some(t) = o.get("issue_types").and_then(Value::as_object) {
        p.story_type = doc_text_n(t.get("story").and_then(Value::as_str).unwrap_or(""), 80, true);
        p.task_type = doc_text_n(t.get("task").and_then(Value::as_str).unwrap_or(""), 80, true);
    }
    if !issue_key_ok(&p.epic) {
        probs.push(format!("에픽 번호 꼴이 아닙니다: {}", take(&p.epic, 40)));
    }
    let list: Vec<Value> = o.get("entries").and_then(Value::as_array).cloned().unwrap_or_default();
    if list.len() > TICKET_ENTRY_MAX {
        probs.push(format!("한 번에 만들 티켓은 {}개까지입니다({}개)", TICKET_ENTRY_MAX, list.len()));
    }
    let mut seen: HashSet<String> = HashSet::new();
    for x in list.iter().take(TICKET_ENTRY_MAX) {
        let Some(e) = x.as_object() else {
            probs.push("꼴이 틀린 항목이 있습니다".to_string());
            continue;
        };
        let Some(id) = e.get("id").and_then(Value::as_str).filter(|s| ticket_entry_id_ok(s)) else {
            probs.push("꼴이 틀린 항목이 있습니다".to_string());
            continue;
        };
        let Some(k) = e.get("kind").and_then(Value::as_str).and_then(TicketKind::from_raw) else {
            probs.push("꼴이 틀린 항목이 있습니다".to_string());
            continue;
        };
        if !seen.insert(id.to_string()) {
            probs.push(format!("같은 항목이 둘입니다: {id}"));
            continue;
        }
        let t = TicketEntry {
            id: id.to_string(),
            kind: k,
            title: notify_clean_n(e.get("title").and_then(Value::as_str).unwrap_or(""), TICKET_TITLE_MAX),
            parent: trim_ws(e.get("parent").and_then(Value::as_str).unwrap_or("")).to_string(),
            key: trim_ws(e.get("key").and_then(Value::as_str).unwrap_or("")).to_string(),
            units: e
                .get("units")
                .and_then(Value::as_array)
                .map(|a| a.iter().take(50).filter_map(Value::as_str).map(|s| notify_clean_n(s, 200)).filter(|s| !s.is_empty()).collect())
                .unwrap_or_default(),
        };
        if id.split(':').find(|x| !x.is_empty()).unwrap_or("") != k.raw_value() {
            probs.push(format!("항목 {id} 의 종류가 이름과 다릅니다"));
        }
        if k != TicketKind::Move && t.title.is_empty() {
            probs.push(format!("항목 {id} 의 제목이 비었습니다"));
        }
        if k == TicketKind::Move && (!issue_key_ok(&t.key) || id != format!("move:{}", t.key)) {
            probs.push(format!("옮길 작업 키 꼴이 아닙니다: {}", take(&t.key, 40)));
        }
        p.entries.push(t);
    }
    // 부모: 스토리는 에픽 아래, 작업·옮기기는 이미 있는 스토리 키 또는 같은 계획의 새 스토리
    let stories: HashSet<String> = p.entries.iter().filter(|e| e.kind == TicketKind::Story).map(|e| e.id.clone()).collect();
    for e in &p.entries {
        match e.kind {
            TicketKind::Story => {
                if e.parent != p.epic {
                    probs.push(format!("스토리 {} 의 부모가 에픽({})이 아닙니다", e.id, p.epic));
                }
            }
            TicketKind::Task | TicketKind::Move => {
                if e.parent.starts_with("story:") {
                    if !stories.contains(&e.parent) {
                        probs.push(format!("항목 {} 의 부모 {} 가 계획에 없습니다", e.id, e.parent));
                    }
                } else if !issue_key_ok(&e.parent) {
                    probs.push(format!("항목 {} 의 부모 꼴이 아닙니다: {}", e.id, take(&e.parent, 40)));
                }
            }
        }
    }
    (p, probs)
}

// MARK: - 상태(엔진이 쓴다)

/// 티켓 하나의 상태. 원본: `APTicketState`
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TicketState {
    /// 아직 시작 전(waits: 항목이 있지만 만들기 요청이 아직 없다)
    #[default]
    Waiting,
    /// 요청을 보내는 중(보내기 전 상태, 답을 받으면 바꾼다)
    Creating,
    Made,
    Failed,
    /// 잠금에서 creating 이 남아 앞 만들기가 끝났는지 알 수 없다 — 사람이 확인해야 다시 만든다
    Uncertain,
}

impl TicketState {
    /// Swift rawValue
    pub fn raw_value(&self) -> &'static str {
        match self {
            TicketState::Waiting => "waiting",
            TicketState::Creating => "creating",
            TicketState::Made => "made",
            TicketState::Failed => "failed",
            TicketState::Uncertain => "uncertain",
        }
    }

    /// Swift `APTicketState(rawValue:)`
    pub fn from_raw(s: &str) -> Option<TicketState> {
        match s {
            "waiting" => Some(TicketState::Waiting),
            "creating" => Some(TicketState::Creating),
            "made" => Some(TicketState::Made),
            "failed" => Some(TicketState::Failed),
            "uncertain" => Some(TicketState::Uncertain),
            _ => None,
        }
    }
}

/// 요청 한 번의 기록. 원본 Outbox.swift 141–145 줄 `APAttempt`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Attempt {
    pub at: f64,
    pub what: String,
    pub code: i32,
    pub note: String,
}

/// 티켓 하나의 상태 기록. 원본: `APTicketRecord`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TicketRecord {
    pub state: TicketState,
    /// 만든 키(move 는 옮긴 작업 키)
    pub key: String,
    pub url: String,
    pub last: Option<Attempt>,
    pub tries: i32,
    pub updated: f64,
}

/// 상태 파일(tickets.json, 엔진이 쓴다 — 보드는 읽기만). 원본: `APTicketBook`
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TicketBook {
    /// 만들기 시작한 사이트(만든 키는 이 사이트의 것)
    pub site: String,
    /// Swift Dictionary 는 순서가 없다 — BTreeMap 으로 둔다(시험은 키를 정렬해 견준다)
    pub entries: BTreeMap<String, TicketRecord>,
}

/// Swift `NSNumber` 수 꺼내기 — JSONSerialization 은 true·false 도 NSNumber(1·0)로 준다
fn num_f64(v: &Value) -> f64 {
    match v {
        Value::Bool(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::Number(n) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

/// Swift `(x as? NSNumber)?.intValue`
fn int_value(v: &Value) -> i32 {
    match v {
        Value::Bool(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        Value::Number(n) => n.as_i64().map(|x| x as i32).or_else(|| n.as_f64().map(|f| f as i32)).unwrap_or(0),
        _ => 0,
    }
}

/// Swift `(x as? NSNumber)` — 칸이 없거나 문자·사전이면 None(불리언·수만)
fn int_opt(v: Option<&Value>) -> Option<i32> {
    match v {
        Some(Value::Bool(_)) | Some(Value::Number(_)) => Some(int_value(v.unwrap())),
        _ => None,
    }
}

/// 상태 파일 값(엔진이 쓴다). 원본: `apTicketBookJSON`
pub fn ticket_book_json(b: &TicketBook) -> Value {
    let mut es = Map::new();
    for (k, r) in &b.entries {
        let mut d = json!({"state": r.state.raw_value(), "tries": r.tries, "updated": r.updated});
        if !r.key.is_empty() {
            d["key"] = json!(r.key);
        }
        if !r.url.is_empty() {
            d["url"] = json!(r.url);
        }
        if let Some(a) = &r.last {
            d["last"] = json!({"at": a.at, "what": a.what, "code": a.code, "note": a.note});
        }
        es.insert(k.clone(), d);
    }
    let mut o = json!({"format": TICKET_BOOK_FORMAT, "version": 1, "entries": Value::Object(es)});
    if !b.site.is_empty() {
        o["site"] = json!(b.site);
    }
    o
}

/// 상태 파일 → 책. 꼴이 틀린 칸은 버린다(상태 없음으로). 원본: `apTicketBookParse`
pub fn ticket_book_parse(data: Option<&[u8]>) -> TicketBook {
    let mut b = TicketBook::default();
    let o: Value = match data {
        Some(d) if d.len() <= 1 << 20 => serde_json::from_slice(d).unwrap_or(Value::Null),
        _ => Value::Null,
    };
    if o.get("format").and_then(Value::as_str) != Some(TICKET_BOOK_FORMAT) {
        return b;
    }
    b.site = o.get("site").and_then(Value::as_str).unwrap_or("").to_string();
    if let Some(es) = o.get("entries").and_then(Value::as_object) {
        for (k, v) in es {
            if !ticket_entry_id_ok(k) {
                continue;
            }
            let Some(d) = v.as_object() else { continue };
            let Some(st) = d.get("state").and_then(Value::as_str).and_then(TicketState::from_raw) else { continue };
            let mut r = TicketRecord { state: st, ..Default::default() };
            let key = d.get("key").and_then(Value::as_str).unwrap_or("");
            r.key = if issue_key_ok(key) { key.to_string() } else { String::new() };
            r.url = doc_text_n(d.get("url").and_then(Value::as_str).unwrap_or(""), 500, true);
            if let Some(a) = d.get("last").and_then(Value::as_object) {
                r.last = Some(Attempt {
                    at: num_f64(a.get("at").unwrap_or(&Value::Null)),
                    what: a.get("what").and_then(Value::as_str).unwrap_or("").to_string(),
                    code: int_value(a.get("code").unwrap_or(&Value::Null)),
                    note: doc_text_n(a.get("note").and_then(Value::as_str).unwrap_or(""), 500, true),
                });
            }
            r.tries = int_value(d.get("tries").unwrap_or(&Value::Null));
            r.updated = num_f64(d.get("updated").unwrap_or(&Value::Null));
            if r.state == TicketState::Made && r.key.is_empty() {
                r.state = TicketState::Uncertain;
            }
            b.entries.insert(k.clone(), r);
        }
    }
    b
}

/// 만든 키 — 보드가 대기열에 넣는다: 보드 스토리 id → 스토리 키, 단위 id → 작업 키. 원본: `apTicketKeys`
pub fn ticket_keys(p: &TicketPlan, b: &TicketBook) -> (BTreeMap<String, String>, BTreeMap<String, String>) {
    let mut s: BTreeMap<String, String> = BTreeMap::new();
    let mut t: BTreeMap<String, String> = BTreeMap::new();
    for e in &p.entries {
        let Some(r) = b.entries.get(&e.id) else { continue };
        if r.state != TicketState::Made || r.key.is_empty() {
            continue;
        }
        let tail = e.id.split_once(':').map(|x| x.1).unwrap_or("");
        match e.kind {
            TicketKind::Story => {
                s.insert(tail.to_string(), r.key.clone());
            }
            TicketKind::Task => {
                t.insert(tail.to_string(), r.key.clone());
            }
            TicketKind::Move => {}
        }
    }
    (s, t)
}

// MARK: - 키 가림막·임시 번호(이 심부름이 쓰는 것)

/// 이슈 키 꼴 — `^[A-Z][A-Z0-9_]*-[0-9]{1,9}$` 를 판별자로 옮긴 것. 원본: Outbox.swift 38줄 `apIssueKeyPattern`
/// (Outbox 는 아직 이식 전이라 여기에 두고, R-게시 청이 outbox.rs 를 심으면 그쪽에서 같은 것을 쓰게 한다)
pub fn issue_key_ok(s: &str) -> bool {
    let Some((head, digits)) = s.split_once('-') else { return false };
    let mut hs = head.chars();
    if !matches!(hs.next(), Some(c) if c.is_ascii_uppercase()) {
        return false;
    }
    if !hs.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        return false;
    }
    (1..=9).contains(&digits.chars().count()) && digits.chars().all(|c| c.is_ascii_digit())
}

/// 임시 티켓 번호 꼴 — `^TMP-[0-9]{1,4}-[0-9]{1,2}$` 를 판별자로 옮긴 것. 원본: Tickets.swift 455줄 `apTempTicketPattern`
/// (선언은 R-Tickets-b 자리지만 a 청의 `unmade_keys` 가 쓴다)
pub fn temp_ticket_ok(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("TMP-") else { return false };
    let Some((unit, seq)) = rest.split_once('-') else { return false };
    (1..=4).contains(&unit.chars().count()) && unit.chars().all(|c| c.is_ascii_digit()) && (1..=2).contains(&seq.chars().count()) && seq.chars().all(|c| c.is_ascii_digit())
}

/// 자리표가 될 수 없는 이슈 키 꼭 꼴 — `^[A-Z][A-Z0-9_]*-NEW([0-9]{1,4}|-[A-Za-z0-9._]{1,64})$` 를 판별자로 옮긴 것.
/// (이슈 키 꼴은 숫자 부분이 있어 NEW 로 시작할 수 없다) 원본: `apIsTicketPlaceholder`
pub fn is_ticket_placeholder(s: &str) -> bool {
    let Some((head, tail)) = s.split_once('-') else { return false };
    let mut hs = head.chars();
    if !matches!(hs.next(), Some(c) if c.is_ascii_uppercase()) {
        return false;
    }
    if !hs.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        return false;
    }
    let Some(rest) = tail.strip_prefix("NEW") else { return false };
    let Some(rest) = rest.strip_prefix('-') else {
        return (1..=4).contains(&rest.chars().count()) && rest.chars().all(|c| c.is_ascii_digit());
    };
    (1..=64).contains(&rest.chars().count()) && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

/// 대기열에 넣을 키 자리표 — story:new-N 은 SHOP-NEW{N}, 그 밖은 SHOP-NEW-<id 꼬리>. Swift 기본 인자(`project=""`)는 없다.
/// 이슈 키 꼭 꼴(숫자 부분)과 임시 번호 꼴은 출발을 막는다(엔진 start 는 자리표가 남은 대기열로 출발하지 않는다).
/// 원본: `apTicketPlaceholderPattern`(규칙) · `apTicketPlaceholder`(값)
pub fn ticket_placeholder(project: &str, entry: &str) -> String {
    let tail = entry.split_once(':').map(|x| x.1).unwrap_or("");
    if entry.starts_with("story:") && tail.starts_with("new-") {
        if let Ok(k) = tail[4..].parse::<i64>() {
            if (0..=9999).contains(&k) {
                return format!("{project}-NEW{k}");
            }
        }
    }
    let safe: String = tail
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '_' { c } else { '_' })
        .take(64)
        .collect();
    format!("{project}-NEW-{safe}")
}

/// 출발하면 안 되는 키 — 대기열 단위의 story·tickets 에 자리표나 임시 번호(TMP-…)가 남았으면 그 값들(처음 본 순서). 원본: `apUnmadeKeys`
pub fn unmade_keys(units: &[Value]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for u in units {
        let mut vals: Vec<String> = Vec::new();
        if let Some(x) = u.get("story").and_then(Value::as_str) {
            vals.push(x.to_string());
        }
        if let Some(a) = u.get("tickets").and_then(Value::as_array) {
            vals.extend(a.iter().filter_map(Value::as_str).map(str::to_string));
        }
        for v in vals {
            if (is_ticket_placeholder(&v) || temp_ticket_ok(&v)) && !out.contains(&v) {
                out.push(v);
            }
        }
    }
    out
}

/// 남은 항목(만들지 않았거나 실패·확인 필요) — 출발은 이것이 없을 때만. 원본: `apTicketsLeft`
pub fn tickets_left(p: &TicketPlan, b: &TicketBook) -> Vec<TicketEntry> {
    p.entries
        .iter()
        .filter(|e| !b.entries.get(&e.id).map(|r| r.state == TicketState::Made).unwrap_or(false))
        .cloned()
        .collect()
}

// MARK: - 이 파일 안에서만 쓰는 것들

/// `trimmingCharacters(in: .whitespaces)` — Zs 범주와 탭(formats.rs `ws` 와 같은 판별)
fn trim_ws(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'))
}

/// Swift `String.prefix(n)` 근사 — grapheme 으로 센 `n` 개 덩어리까지의 접두어. (formats.rs 의 cluster 함수는 비공개라
/// 접두어마다 grapheme_len 을 다시 센다 — 여기는 짧은 까닭 문장만 자른다) `len_utf8` 으로 자리를 옮겨가며 다음 덩어리가
/// 시작하는 자리(그 앞이 n 개)를 찾는다
fn take(s: &str, n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    for (i, c) in s.char_indices() {
        if grapheme_len(&s[..i + c.len_utf8()]) == n + 1 {
            return s[..i].to_string();
        }
    }
    s.to_string()
}

// MARK: - 요청 꼴(보드가 HTTP 를 하지 않으므로 꼴만 준다)

/// Jira 이슈 종류 응답 한 줄. 원본: `APIssueType`
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssueType {
    pub id: String,
    pub name: String,
    pub subtask: bool,
    /// hierarchyLevel: 에픽 1, 스토리·작업 0, 하위 작업 -1
    pub level: Option<i32>,
}

/// Swift JSONSerialization `[.sortedKeys, .withoutEscapingSlashes]` — 키를 자순(deep)으로 정렬해 내보낸다(`/` 는 이스케이프하지
/// 않는다, serde_json 도 같다). 순서 있는 Map(preserve_order)이므로 BTreeMap 순서로 다시 싣는다
fn sort_keys(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let keys: BTreeMap<String, &Value> = m.iter().map(|(k, val)| (k.clone(), val)).collect();
            let mut sorted = Map::new();
            for (k, val) in keys {
                sorted.insert(k, sort_keys(val));
            }
            Value::Object(sorted)
        }
        Value::Array(a) => Value::Array(a.iter().map(sort_keys).collect()),
        other => other.clone(),
    }
}

/// 본문 인코딩(Swift `body` 와 같다 — 실패는 0바이트). 요청 본문의 키는 항상 정렬해 답이 견주기 쉽게
fn body(o: &Value) -> Vec<u8> {
    serde_json::to_vec(&sort_keys(o)).unwrap_or_default()
}

fn json_headers() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("Content-Type".to_string(), "application/json".to_string()),
        ("Accept".to_string(), "application/json".to_string()),
    ])
}

/// 프로젝트의 이슈 종류 — 종류 이름은 사이트·언어마다 달라(Story·스토리, Subtask·Sub-task·하위 작업) 이름을 박지 않고 응답에서 고른다.
/// 원본: `apJiraProjectRequest`
pub fn jira_project_request(project: &str, base: &str, auth: Auth) -> HttpRequest {
    let mut headers = BTreeMap::new();
    headers.insert("Accept".to_string(), "application/json".to_string());
    HttpRequest { method: "GET".to_string(), url: format!("{base}/rest/api/3/project/{project}"), headers, body: None, auth }
}

/// GET /rest/api/3/project/<키> 의 응답에서 이슈 종류를 읽는다(믿지 않는 값). 원본: `apJiraIssueTypes`
pub fn jira_issue_types(r: &HttpResponse) -> Vec<IssueType> {
    if r.error.is_some() || r.status != 200 || r.body.len() > 4 << 20 {
        return Vec::new();
    }
    let Ok(o) = serde_json::from_slice::<Value>(&r.body) else { return Vec::new() };
    let list: Vec<Value> = o.get("issueTypes").and_then(Value::as_array).cloned().unwrap_or_default();
    list.iter()
        .take(100)
        .filter_map(|x| {
            let t = x.as_object()?;
            let id = t.get("id").and_then(Value::as_str)?;
            if !((1..=20).contains(&id.chars().count()) && id.chars().all(|c| c.is_ascii_digit())) {
                return None;
            }
            Some(IssueType {
                id: id.to_string(),
                name: doc_text_n(t.get("name").and_then(Value::as_str).unwrap_or(""), 80, true),
                subtask: t.get("subtask").and_then(Value::as_bool).unwrap_or(false),
                level: int_opt(t.get("hierarchyLevel")),
            })
        })
        .collect()
}

/// 이슈 종류 고르기 — 프로젝트 설정에 이름이 있으면 그 이름(대소문자 무시), 없으면 스토리는 하위 작업이 아닌 수준 0 의
/// Story·스토리, 작업·옮기기는 하위 작업 종류(subtask 또는 수준 -1)의 Subtask·Sub-task·하위 작업, 그것도 없으면 그 무리의
/// 첫 종류. 프로젝트 설정 이름이 없는 무리이면 없다. 원본: `apPickIssueType`
pub fn pick_issue_type(types: &[IssueType], kind: TicketKind, name: &str) -> Option<IssueType> {
    let want = trim_ws(name).to_lowercase();
    if !want.is_empty() {
        return types.iter().find(|t| t.name.to_lowercase() == want).cloned();
    }
    match kind {
        TicketKind::Story => {
            let pool: Vec<&IssueType> = types.iter().filter(|t| !t.subtask && t.level.unwrap_or(0) == 0).collect();
            pool.iter()
                .find(|t| t.name.to_lowercase() == "story" || t.name == "스토리")
                .or_else(|| {
                    pool.iter().find(|t| {
                        let n = t.name.to_lowercase();
                        !(n == "task" || n == "bug" || t.name == "작업" || t.name == "버그")
                    })
                })
                .or_else(|| pool.first())
                .map(|t| (*t).clone())
        }
        TicketKind::Task | TicketKind::Move => {
            let pool: Vec<&IssueType> = types.iter().filter(|t| t.subtask || t.level == Some(-1)).collect();
            pool.iter()
                .find(|t| {
                    let n = t.name.to_lowercase();
                    n == "subtask" || n == "sub-task" || t.name == "하위 작업"
                })
                .or_else(|| pool.first())
                .map(|t| (*t).clone())
        }
    }
}

/// 티켓 본문 — 첫 줄 표지, 무엇을 위한 티켓인지, 단위 제목, 보드 티켓 번호(사람이 Jira 에서 찾게).
/// Swift 기본 인자(`temp=""`·`why=""`·`task=""`)는 Rust 에 없어 모두 받는다. 원본: `apTicketDoc`
pub fn ticket_doc(e: &TicketEntry, run: &str, temp: &str, why: &str, task: &str) -> Doc {
    let mut b: Vec<Block> = vec![Block::Paragraph(vec![Span::Text(DOC_MARKER.to_string(), true)])];
    if !temp.is_empty() {
        // 실행 중에 새로 필요해진 티켓 — 가지는 이 번호로 만들지 않았다(임시 번호로 진행)
        let tail_m = if task.is_empty() { String::new() } else { format!("(작업 {task})") };
        b.push(Block::Paragraph(vec![Span::Text(
            format!("자율주행 실행 {run} 의 단위{tail_m}가 일하다가 새로 필요해진 티켓입니다. 실행 중에는 임시 번호 {temp} 로 진행했고, 사람이 검수한 뒤 만든 티켓입니다."),
            false,
        )]));
        if !why.is_empty() {
            b.push(Block::Paragraph(vec![Span::Text(format!("까닭: {why}"), false)]));
        }
    } else {
        match e.kind {
            TicketKind::Story => b.push(Block::Paragraph(vec![Span::Text(
                format!("자율주행 실행 {run} 의 출발 확인에서 승인한 스토리입니다. 아래 작업 티켓의 단위를 묶습니다."),
                false,
            )])),
            TicketKind::Task | TicketKind::Move => b.push(Block::Paragraph(vec![Span::Text(
                format!("자율주행 실행 {run} 의 단위 하나(작업 티켓)입니다. 엔진이 이 번호로 작업 가지를 만듭니다."),
                false,
            )])),
        }
    }
    if !e.units.is_empty() {
        b.push(Block::Bullets(e.units.iter().take(50).map(|u| vec![Span::Text(u.clone(), false)]).collect()));
    }
    b.push(Block::Paragraph(vec![Span::Text(format!("보드 티켓 번호 {run}/{}", e.id), false)]));
    doc_clamp(&Doc::new(b))
}

/// 만들기 — POST /rest/api/3/issue. 스토리는 parent 에 에픽, 작업(하위 작업 종류)은 parent 에 스토리.
/// 제목은 모델·바깥이 쓴 글일 수 있다 — 상한(TICKET_TITLE_MAX)·다듬기, 본문은 문서 모형(계획 꼴). 원본: `apJiraCreateIssueRequest`
pub fn jira_create_issue_request(project: &str, type_id: &str, parent: &str, summary: &str, doc: &Doc, base: &str, auth: Auth) -> HttpRequest {
    let fields = json!({
        "project": {"key": project},
        "issuetype": {"id": type_id},
        "parent": {"key": parent},
        "summary": doc_text_n(summary, TICKET_TITLE_MAX, true),
        "description": adf(doc),
    });
    HttpRequest { method: "POST".to_string(), url: format!("{base}/rest/api/3/issue"), headers: json_headers(), body: Some(body(&json!({"fields": fields}))), auth }
}

/// 부모 바꾸기 — PUT /rest/api/3/issue/<키>(다른 스토리로 옮긴 이미 있는 작업). 하위 작업의 부모 바꾸기는 사이트 설정에 따라
/// 거부될 수 있다(그때 실패 줄). 원본: `apJiraParentRequest`
pub fn jira_parent_request(key: &str, parent: &str, base: &str, auth: Auth) -> HttpRequest {
    let o = json!({"fields": {"parent": {"key": parent}}});
    HttpRequest { method: "PUT".to_string(), url: format!("{base}/rest/api/3/issue/{key}"), headers: json_headers(), body: Some(body(&o)), auth }
}

/// 만든 키 — 응답(믿지 않는 값)의 key 칸이 이슈 키 꼴이고 그 프로젝트의 것일 때만. 원본: `apCreatedIssueKey`
pub fn created_issue_key(r: &HttpResponse, project: &str) -> Option<String> {
    if r.error.is_some() || !(200..=299).contains(&r.status) || r.body.len() > 1 << 20 {
        return None;
    }
    let o: Value = serde_json::from_slice(&r.body).ok()?;
    let k = o.get("key").and_then(Value::as_str)?;
    if !issue_key_ok(k) || !k.starts_with(&format!("{project}-")) {
        return None;
    }
    Some(k.to_string())
}

/// 실패 까닭(사람 말) — 받는 쪽 본문은 믿지 않아 상태 번호로만. 원본: Outbox.swift 422–435 줄 `apFailNote`
/// (Outbox 는 아직 이식 전이라 여기에 둔다 — R-게시 청이 outbox.rs 를 심을 때 같은 것을 쓰게 한다)
pub fn fail_note(r: &HttpResponse) -> String {
    if let Some(e) = &r.error {
        return e.clone();
    }
    match r.status {
        401 | 403 => format!("거부됨({}): 토큰이 만료·취소됐거나 그 자리에 쓸 권한이 없습니다", r.status),
        404 => "받을 곳을 찾지 못했습니다(404): 이슈·스페이스·페이지 id 를 확인합니다".to_string(),
        400 => "받는 쪽이 글 꼴을 받지 않았습니다(400): 같은 제목의 페이지가 있거나 받을 곳 값이 틀렸을 수 있습니다".to_string(),
        409 => "받는 쪽에서 부딪혔습니다(409)".to_string(),
        413 => "글이 너무 큽니다(413)".to_string(),
        429 => "요청이 많다며 거절했습니다(429). 잠시 뒤 다시 올립니다".to_string(),
        300..=399 => format!("다른 주소로 보내려 해 따라가지 않았습니다({})", r.status),
        500..=599 => format!("받는 쪽 오류({})", r.status),
        _ => format!("받는 쪽이 {} 로 답했습니다", r.status),
    }
}

/// 만들기 실패 까닭 — 받는 쪽 본문은 믿지 않아 상태 번호로만(Jira 티켓에 맞춘 말, 그 밖은 `fail_note`). 원본: `apTicketFailNote`
pub fn ticket_fail_note(r: &HttpResponse) -> String {
    if r.error.is_some() {
        return fail_note(r);
    }
    match r.status {
        400 => "Jira 가 요청 꼴을 받지 않았습니다(400): 이슈 종류·부모·필수 칸을 확인합니다(프로젝트 설정 tickets.issue_types)".to_string(),
        401 | 403 => format!("거부됨({}): 토큰이 만료·취소됐거나 그 프로젝트에 티켓을 만들 권한이 없습니다", r.status),
        404 => "프로젝트나 부모 이슈를 찾지 못했습니다(404)".to_string(),
        _ => fail_note(r),
    }
}

// ── R-Tickets-b (원본 295–528: 만들기와 행·임시 티켓 + 시험 make_all·retry_left·uncertain·blocked·waits_and_guards·types_and_keys·outbox·post_ticket) ──
