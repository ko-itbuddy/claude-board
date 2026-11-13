//! Tickets 청 a(원본 1–293)의 시험 — `Tests/AutopilotRulesTests/TicketsTests.swift` 의 여섯 가지
//! (계획 읽기·이슈 종류 고르기·요청 꼴·제목 다듬기·키 자리표·상태 파일).
//! make_all·retry_left·uncertain·blocked·waits_and_guards·types_and_keys 시험과 outbox·post_ticket 은 R-Tickets-b.
//! 요청 꼴 골든(Tests/AutopilotRulesTests/Publish/ticket-*.json)은 만들기·부모바꾸기 요청 본문 그대로다

use std::path::Path;

use claude_board_autopilot::rules::connections::{Auth, HttpResponse};
use claude_board_autopilot::rules::formats::grapheme_len;
use claude_board_autopilot::rules::tickets::*;
use serde_json::{json, Value};

const SITE: &str = "https://zz-toy.atlassian.net";
const RUN: &str = "20261030-1500-zz";

// ── 골든·견본 도우미(원본의 golden·pretty) ─────────────────────────────────

/// 골든과 비교한다 — `AP_REPORT_GOLDEN=write` 면 다시 쓴다. 윈도우 체크아웃은 줄 끝이 CRLF 일 수 있다(글: LF 만)
fn golden(name: &str, text: &str) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Tests/AutopilotRulesTests/Publish").join(name);
    if std::env::var("AP_REPORT_GOLDEN").as_deref() == Ok("write") {
        std::fs::write(p, text).expect("골든 쓰기");
        return;
    }
    let want = std::fs::read_to_string(p).map(|s| s.replace("\r\n", "\n")).unwrap_or(format!("(골든 {name} 없음)"));
    assert_eq!(text, want, "골든 {name} 와 다름");
}

/// Swift JSONSerialization 의 [.prettyPrinted, .sortedKeys, .withoutEscapingSlashes] 꼴 — 2칸 들여쓰기, 키 뒤 " : "
fn pretty(v: &Value) -> String {
    fn out(v: &Value, depth: usize, buf: &mut String) {
        match v {
            Value::Object(o) if o.is_empty() => buf.push_str("{}"),
            Value::Object(o) => {
                let mut kv: Vec<(&String, &Value)> = o.iter().collect();
                kv.sort_by(|a, b| a.0.cmp(b.0));
                buf.push_str("{\n");
                for (i, (k, val)) in kv.iter().enumerate() {
                    buf.push_str(&" ".repeat(depth * 2 + 2));
                    out(&Value::String((*k).clone()), depth + 1, buf);
                    buf.push_str(" : ");
                    out(val, depth + 1, buf);
                    if i + 1 < kv.len() {
                        buf.push(',');
                    }
                    buf.push('\n');
                }
                buf.push_str(&" ".repeat(depth * 2));
                buf.push('}');
            }
            Value::Array(a) if a.is_empty() => buf.push_str("[]"),
            Value::Array(a) => {
                buf.push_str("[\n");
                for (i, val) in a.iter().enumerate() {
                    buf.push_str(&" ".repeat(depth * 2 + 2));
                    out(val, depth + 1, buf);
                    if i + 1 < a.len() {
                        buf.push(',');
                    }
                    buf.push('\n');
                }
                buf.push_str(&" ".repeat(depth * 2));
                buf.push(']');
            }
            other => buf.push_str(&other.to_string()),
        }
    }
    let mut s = String::new();
    out(v, 0, &mut s);
    s.push('\n');
    s
}

/// 요청 본문(Option) — 없으면 "(본문 없음)"
fn pretty_body(b: Option<&[u8]>) -> String {
    match b {
        None => "(본문 없음)\n".to_string(),
        Some(d) => {
            let v: Value = serde_json::from_slice(d).expect("요청 본문 JSON");
            pretty(&v)
        }
    }
}

/// Swift `APAuth.basic(email:)`
fn auth() -> Auth {
    Auth::Basic { user: "kim@example.com".to_string() }
}

// ── 견본(Swift 시험과 같은 계획) ───────────────────────────────────────────

/// 출발 확인에서 승인한 만들 티켓 계획 — 에픽 아래 새 스토리·작업 둘, 이미 있는 스토리로 옮길 작업 하나
fn plan() -> TicketPlan {
    TicketPlan {
        run: RUN.to_string(),
        epic: "SHOP-10".to_string(),
        story_type: String::new(),
        task_type: String::new(),
        entries: vec![
            TicketEntry {
                id: "story:new-1".to_string(),
                kind: TicketKind::Story,
                title: "쿠폰 만료 묶음".to_string(),
                parent: "SHOP-10".to_string(),
                key: String::new(),
                units: vec!["쿠폰 만료일 검사".to_string(), "만료 쿠폰 안내".to_string()],
            },
            TicketEntry {
                id: "task:u2".to_string(),
                kind: TicketKind::Task,
                title: "쿠폰 만료일 검사".to_string(),
                parent: "story:new-1".to_string(),
                key: String::new(),
                units: vec!["쿠폰 만료일 검사".to_string()],
            },
            TicketEntry {
                id: "task:u4".to_string(),
                kind: TicketKind::Task,
                title: "장바구니 합계 고침".to_string(),
                parent: "SHOP-12".to_string(),
                key: String::new(),
                units: vec!["장바구니 합계 고침".to_string()],
            },
            TicketEntry {
                id: "move:SHOP-13".to_string(),
                kind: TicketKind::Move,
                title: String::new(),
                parent: "story:new-1".to_string(),
                key: "SHOP-13".to_string(),
                units: Vec::new(),
            },
        ],
    }
}

// ── 시험 ───────────────────────────────────────────────────────────────────

/// 계획을 읽는다 — 꼴이 틀린 곳의 까닭을 준다. 고칠 것이 하나라도 있으면 만들기를 막는다(요청 0)
#[test]
fn plan_parse() {
    // 맞음
    let bytes = serde_json::to_vec(&ticket_plan_json(&plan())).expect("계획 JSON");
    let (p, broken) = ticket_plan_parse(Some(&bytes));
    assert_eq!(broken, Vec::<String>::new());
    assert_eq!(p, plan());
    assert_eq!(p.project(), "SHOP");

    // 꼴이 틀린 견본 여섯+하나(곤란이 있는 자리) — 각각 대답에 까닭이 있다
    let broken_of = |patch: &dyn Fn(&mut Value)| -> Vec<String> {
        let mut o = ticket_plan_json(&plan());
        patch(&mut o);
        ticket_plan_parse(Some(o.to_string().as_bytes())).1
    };

    // 에픽 번호 꼴
    let broken = broken_of(&|o| o["epic"] = json!("shop 10"));
    assert!(broken.iter().any(|s| s.contains("에픽 번호 꼴이 아닙니다: shop 10")));
    // 모르는 부모
    let broken = broken_of(&|o| o["entries"][1]["parent"] = json!("story:new-9"));
    assert!(broken.iter().any(|s| s.contains("항목 task:u2 의 부모 story:new-9 가 계획에 없습니다")));
    // 스토리는 에픽 아래
    let broken = broken_of(&|o| o["entries"][0]["parent"] = json!("SHOP-11"));
    assert!(broken.iter().any(|s| s.contains("스토리 story:new-1 의 부모가 에픽(SHOP-10)이 아닙니다")));
    // 옮길 키와 이름이 다름
    let broken = broken_of(&|o| o["entries"][3]["key"] = json!("SHOP-14"));
    assert!(broken.iter().any(|s| s.contains("옮길 작업 키 꼴이 아닙니다: SHOP-14")));
    // 같은 항목이 둘
    let broken = broken_of(&|o| {
        let e = o["entries"][1].clone();
        o["entries"].as_array_mut().expect("entries").push(e);
    });
    assert!(broken.iter().any(|s| s.contains("같은 항목이 둘입니다: task:u2")));
    // 빈 제목(비인쇄 뿐)
    let broken = broken_of(&|o| o["entries"][1]["title"] = json!(" \u{202E} "));
    assert!(broken.iter().any(|s| s.contains("항목 task:u2 의 제목이 비었습니다")));
    // 종류와 이름이 다름
    let broken = broken_of(&|o| o["entries"][1]["kind"] = json!("story"));
    assert!(broken.iter().any(|s| s.contains("항목 task:u2 의 종류가 이름과 다릅니다")));
}

/// 이슈 종류 고르기 — 프로젝트 설정 이름이 있으면 그 이름(대소문자 무시·없으면 없다), 없으면 종류 무리에서 고른다
#[test]
fn pick_type() {
    let it = |id: &str, name: &str, subtask: bool, level: i32| IssueType { id: id.to_string(), name: name.to_string(), subtask, level: Some(level) };

    // 영문 기본 — Story·Task·Subtask 이름
    let t = vec![it("1", "Story", false, 0), it("3", "Subtask", true, -1)];
    assert_eq!(pick_issue_type(&t, TicketKind::Story, "").as_ref().map(|x| x.id.as_str()), Some("1"));
    assert_eq!(pick_issue_type(&t, TicketKind::Task, "").as_ref().map(|x| x.id.as_str()), Some("3"));
    // 한국어 — 프로젝트 설정 이름이 없어도 종류 이름으로 고른다
    let t = vec![it("1", "스토리", false, 0), it("3", "하위 작업", true, -1)];
    assert_eq!(pick_issue_type(&t, TicketKind::Story, "").as_ref().map(|x| x.id.as_str()), Some("1"));
    assert_eq!(pick_issue_type(&t, TicketKind::Task, "").as_ref().map(|x| x.id.as_str()), Some("3"));
    // 옛 하위 작업 이름(Sub-task)
    let t = vec![it("1", "Story", false, 0), it("3", "Sub-task", true, -1)];
    assert_eq!(pick_issue_type(&t, TicketKind::Task, "").as_ref().map(|x| x.id.as_str()), Some("3"));
    // 이름이 없어도 수준(에픽 1, 글 0, 하위 작업 -1)으로 고른다
    let t = vec![it("1", "Feature", false, 0), it("3", "Child", true, -1)];
    assert_eq!(pick_issue_type(&t, TicketKind::Story, "").as_ref().map(|x| x.id.as_str()), Some("1"));
    assert_eq!(pick_issue_type(&t, TicketKind::Task, "").as_ref().map(|x| x.id.as_str()), Some("3"));
    // 프로젝트 설정 이름(대소문자 무시) — 설정에 있으면 그 종류
    let t = vec![it("1", "Story", false, 0), it("2", "User Story", false, 0), it("3", "개발", false, 0), it("4", "개발 하위", true, -1)];
    assert_eq!(pick_issue_type(&t, TicketKind::Story, "user story").as_ref().map(|x| x.id.as_str()), Some("2"));
    assert_eq!(pick_issue_type(&t, TicketKind::Task, "개발 하위").as_ref().map(|x| x.id.as_str()), Some("4"));
    // 설정 이름이 무리에 없으면 고르지 않는다(만들기를 막는다)
    let t = vec![it("1", "Story", false, 0), it("2", "Task", false, 0), it("3", "Subtask", true, -1)];
    assert!(pick_issue_type(&t, TicketKind::Story, "Feature").is_none());
    // 임의 이름이 아니라 정한 이름만
    assert!(pick_issue_type(&t, TicketKind::Story, "스토리").is_none());
    // 하위 작업 종류가 없으면 없다 — 보통 작업으로 만들지 않는다
    let t = vec![it("1", "Story", false, 0), it("2", "Task", false, 0)];
    assert_eq!(pick_issue_type(&t, TicketKind::Story, "").as_ref().map(|x| x.id.as_str()), Some("1"));
    assert!(pick_issue_type(&t, TicketKind::Task, "").is_none());
}

/// 만들기·부모바꾸기 요청 꼴 — 골든(Publish/ticket-*.json). 요약 칸 상한·제어문자가 다듬어졌는지 함께 견준다
#[test]
fn request_golden() {
    // 제목: 긴 줄표·제어문자·한글 300자 — Jira 요약 255자 안으로 다듬는다
    let dirty = format!("쿠폰 — 만료\u{202E}묶음 {}", "가".repeat(300));
    let e0 = plan().entries[0].clone();
    let doc = ticket_doc(&e0, RUN, "", "", "");
    let create = jira_create_issue_request("SHOP", "10001", "SHOP-10", &dirty, &doc, SITE, auth());
    golden("ticket-create-story.json", &pretty_body(create.body.as_deref()));
    let fields = &serde_json::from_slice::<Value>(create.body.as_ref().expect("본문 있음")).expect("본문 JSON")["fields"];
    let summary = fields["summary"].as_str().expect("요약 있음");
    assert!(grapheme_len(summary) <= TICKET_TITLE_MAX + 1);
    assert!(!summary.contains('—'));
    assert!(!summary.contains('\u{202E}'));
    assert_eq!(create.method, "POST");
    assert_eq!(create.url, format!("{SITE}/rest/api/3/issue"));

    let task = jira_create_issue_request("SHOP", "10003", "SHOP-101", "쿠폰 만료일 검사", &ticket_doc(&plan().entries[1], RUN, "", "", ""), SITE, auth());
    golden("ticket-create-task.json", &pretty_body(task.body.as_deref()));

    let move_req = jira_parent_request("SHOP-13", "SHOP-101", SITE, auth());
    golden("ticket-move.json", &pretty_body(move_req.body.as_deref()));
    assert_eq!(move_req.method, "PUT");
    assert_eq!(move_req.url, format!("{SITE}/rest/api/3/issue/SHOP-13"));
}

/// 계획 제목·단위 제목은 모델이 쓴 글일 수 있다 — 비밀·멘션·줄표를 다듬는다(요청 꼴). 실패 까닭도 사람 말
#[test]
fn plan_clean() {
    let mut p = plan();
    p.entries[0].title = format!("쿠폰 — @here <!channel> https://hooks.example.com/services/T0/B0/{}", "z".repeat(24));
    p.entries[0].units = vec!["@everyone 단위".to_string()];
    let parsed = ticket_plan_parse(Some(serde_json::to_vec(&ticket_plan_json(&p)).expect("계획 JSON").as_slice())).0;
    let title = &parsed.entries[0].title;
    assert!(!title.contains('@'));
    assert!(!title.contains("—"));
    assert!(!title.contains("hooks.example.com/services"));
    assert_eq!(parsed.entries[0].units, vec!["＠everyone 단위".to_string()]);

    // 실패 까닭 — 받는 쪽 본문은 믿지 않아 상태 번호로만(Jira 티켓에 맞춘 말)
    let bad = |code: i32| HttpResponse { status: code, ..Default::default() };
    assert!(ticket_fail_note(&bad(400)).contains("이슈 종류·부모·필수 칸"));
    assert!(ticket_fail_note(&bad(404)).contains("프로젝트나 부모 이슈를 찾지 못했습니다"));
    assert!(ticket_fail_note(&bad(302)).contains("따라가지 않았습니다"));
}

/// 키 자리표 — story:new-N 과 단위 id. 자리표는 이슈 키 꼭 꼴이지만 이슈 키 꼴이 아니라, 출발을 막는 값이다
#[test]
fn placeholders() {
    assert_eq!(ticket_placeholder("SHOP", "story:new-3"), "SHOP-NEW3");
    assert_eq!(ticket_placeholder("SHOP", "task:u2"), "SHOP-NEW-u2");
    assert_eq!(ticket_placeholder("SHOP", "task:a/b c"), "SHOP-NEW-a_b_c");
    assert!(is_ticket_placeholder("SHOP-NEW3"));
    assert!(!issue_key_ok("SHOP-NEW3"));
    // 이슈 키 꼭 꼴의 NEW 뒤는 1–4자리 수 또는 이름 한 조각
    assert!(!is_ticket_placeholder("SHOP-NEW3.5"));
    assert!(!is_ticket_placeholder("SHOP-12"));
    assert!(!is_ticket_placeholder("NEW-1"));

    // 출발하면 안 되는 키 — 자리표·임시 번호(TMP-…)가 남은 대기열 단위
    let units = vec![
        json!({"story": "SHOP-NEW1", "tickets": ["SHOP-NEW-u2"]}),
        json!({"story": "SHOP-12", "tickets": ["SHOP-13"]}),
        json!({"story": "SHOP-12", "tickets": ["TMP-3-1"]}),
        json!({"title": "티켓 없는 단위"}),
    ];
    assert_eq!(unmade_keys(&units), vec!["SHOP-NEW1".to_string(), "SHOP-NEW-u2".to_string(), "TMP-3-1".to_string()]);
    // 모두 만들었고 임시 번호가 없다 — 아무것도 막지 않는다
    assert!(unmade_keys(&[units[1].clone(), units[3].clone()]).is_empty());
}

/// 상태 파일을 읽는다 — 만듦·믿지 않음(만들었다는데 키가 없으면)·꼴이 틀린 칸은 버린다. 그리고 돌려 쓸 수 있나
#[test]
fn book_parse() {
    let o = json!({
        "format": TICKET_BOOK_FORMAT,
        "version": 1,
        "site": SITE,
        "entries": {
            "story:new-1": {"state": "made", "key": "SHOP-101", "tries": 1, "updated": 1},
            "task:u2": {"state": "made", "key": "<b>x</b>"},
            "../evil": {"state": "made", "key": "SHOP-1"},
            "task:u4": {"state": "zz"}
        }
    });
    let bytes = serde_json::to_vec(&o).expect("상태 JSON");
    let b = ticket_book_parse(Some(&bytes));
    let keys: Vec<String> = b.entries.keys().cloned().collect();
    assert_eq!(keys, vec!["story:new-1".to_string(), "task:u2".to_string()]);
    assert_eq!(b.site, SITE);
    let ok = &b.entries["story:new-1"];
    assert_eq!(ok.state, TicketState::Made);
    assert_eq!(ok.key, "SHOP-101");
    assert_eq!(ok.tries, 1);
    // 만들었다는데 키가 없으면 믿지 않는다 — 사람이 확인해야 다시 만든다
    assert_eq!(b.entries["task:u2"].state, TicketState::Uncertain);

    // 돌려 써도 같다
    let back = ticket_book_parse(Some(serde_json::to_vec(&ticket_book_json(&b)).expect("상태 JSON").as_slice()));
    assert_eq!(back, b);
}
