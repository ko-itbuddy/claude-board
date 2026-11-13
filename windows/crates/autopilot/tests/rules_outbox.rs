//! 올릴 글 앞부분(게시 설정·만들 글, `rules::outbox`) 시험 — 원본: `Tests/AutopilotRulesTests/PublishTests.swift` 의
//! `PublishPlanTests`(358–425 줄, 다섯 가운데 넷). `만든 글은 다시 만들지 않는다(같은 id)` 는 올리기(`apCreateItems`·메모리
//! 저장소)가 R-Outbox-b 에서 옮겨질 때 같이 옮긴다.
//! 실제 Atlassian·Notion 으로는 요청하지 않는다. 견본 기록은 `Tests/AutopilotRulesTests/Report/sample-run.json` 을 그 자리에서
//! 읽는다(tests/rules_report.rs 의 FixtureSource 와 같은 것). 시간대는 원본 `seoul` 대신 `Tz::Utc` — 이 네 시험의 판정은
//! 시간대에 안 탄다(만료일 여부·글 제목 때분을 보지 않는다).

use std::collections::HashSet;
use std::path::Path;

use claude_board_autopilot::rules::connections::{Connection, Service};
use claude_board_autopilot::rules::outbox::*;
use claude_board_autopilot::rules::report::{report_run, ReportOrigin, ReportRun, ReportSource};
use claude_board_autopilot::rules::stopline::Tz;
use serde_json::Value;

const T0: f64 = 1_793_340_000.0; // 2026-10-30 즈음(서울) — 원본 PublishTests.swift `t0`

// 가짜 읽을 곳 — 견본 기록의 사전을 그 자리에서. 원본: `FixtureSource`(tests/rules_report.rs 와 같은 것)
struct FixtureSource {
    j: Value,
}

impl FixtureSource {
    fn new(j: Value) -> Self {
        FixtureSource { j }
    }
}

impl ReportSource for FixtureSource {
    fn origin(&self) -> ReportOrigin {
        ReportOrigin::Folder
    }
    fn run(&self) -> Option<serde_json::Map<String, Value>> {
        self.j.get("run").and_then(Value::as_object).cloned()
    }
    fn queue(&self) -> Vec<serde_json::Map<String, Value>> {
        let Some(Value::Array(a)) = self.j.get("queue") else { return Vec::new() };
        a.iter().map(|v| v.as_object().cloned()).collect::<Option<Vec<_>>>().unwrap_or_default()
    }
    fn engine(&self, n: i64) -> Option<serde_json::Map<String, Value>> {
        self.j.get("engines")?.get(n.to_string())?.as_object().cloned()
    }
    fn result(&self, n: i64) -> Option<String> {
        match self.j.get("results")?.get(n.to_string())? {
            Value::String(s) => Some(s.clone()),
            o => Some(o.to_string()),
        }
    }
}

fn fixture(name: &str) -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Tests/AutopilotRulesTests/Report").join(name);
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// 견본 기록을 보고서 값으로. 원본 PublishTests.swift `run(_ name:)` — id 는 기록의 `id` 칸, 없으면 "?"
fn run(name: &str) -> ReportRun {
    let j = fixture(name);
    let id = j.get("id").and_then(Value::as_str).unwrap_or("?").to_string();
    report_run(&id, &FixtureSource::new(j)).unwrap_or_default()
}

/// 원본 `atl`(Atlassian 연결) — 만료일만 달리 다시 만든다(`var old = atl; old.expires = …`)
fn atlassian(expires: &str) -> Connection {
    Connection {
        id: "atlassian".to_string(),
        service: Service::Atlassian,
        site: "https://example.atlassian.net".to_string(),
        account: "kim@example.com".to_string(),
        scopes: vec!["jira".to_string(), "confluence".to_string()],
        expires: expires.to_string(),
        ..Connection::default()
    }
}

// ── 원본: 게시 설정 읽기(칸마다 한 줄) ─────────────────────────

struct PublishRow {
    note: &'static str,
    json: &'static str,
    want: &'static str,
}

const PUBLISH_ROWS: [PublishRow; 7] = [
    PublishRow { note: "없음 = 검수 뒤 올림, 받을 곳 없음", json: "null", want: "review - false | " },
    PublishRow {
        note: "자동·Confluence",
        json: r#"{"mode":"auto","report":{"service":"confluence","space":"TODO","parent":"123"},"jira_comments":true}"#,
        want: "auto confluence TODO 123 true | ",
    },
    PublishRow {
        note: "Confluence 상위 페이지 없음 = 스페이스 맨 위",
        json: r#"{"report":{"service":"confluence","space":"~abc"}}"#,
        want: "review confluence ~abc  false | ",
    },
    PublishRow {
        note: "Notion id 는 줄표 빼고 소문자",
        json: r#"{"report":{"service":"notion","parent":"0123ABCD-89ab-cdef-0123-456789ABCDEF"}}"#,
        want: "review notion  0123abcd89abcdef0123456789abcdef false | ",
    },
    PublishRow {
        note: "틀린 꼴은 그 칸만 버림",
        json: r#"{"mode":"later","report":{"service":"confluence","space":"a b"},"jira_comments":"yes"}"#,
        want: "review - false | publish.mode 는 review 또는 auto 입니다; publish.jira_comments 는 true/false 입니다; Confluence 스페이스 키 꼴이 아닙니다: a b",
    },
    PublishRow {
        note: "상위 페이지 id 숫자만",
        json: r#"{"report":{"service":"confluence","space":"TODO","parent":"../1"}}"#,
        want: "review - false | Confluence 상위 페이지 id 는 숫자입니다: ../1",
    },
    PublishRow {
        note: "모르는 서비스",
        json: r#"{"report":{"service":"slack"}}"#,
        want: "review - false | publish.report.service 는 confluence 또는 notion 입니다",
    },
];

/// 게시 방식의 raw 값(시험이 원본 `rawValue` 처럼 표시하려고)
fn mode_text(m: &PublishMode) -> &'static str {
    match m {
        PublishMode::Review => "review",
        PublishMode::Auto => "auto",
    }
}

/// 검수 예시 글 — 원본 PublishTests.swift `item(_:)`(jira 갈래는 R-Outbox-b 의 `한번` 시험이 쓴다)
fn item(s: TargetService) -> OutboxItem {
    let r = run("sample-run.json");
    match s {
        TargetService::Jira => unit_outbox(&r, 1, Some("TODO-41"), &Publish { jira_comments: true, ..Publish::default() }, T0)
            .into_iter()
            .next()
            .unwrap(),
        TargetService::Confluence => run_outbox(
            &r,
            &Publish { report: Some(OutboxTarget::new(s, "", "TODO", "12345")), ..Publish::default() },
            "",
            true,
            T0,
            Tz::Utc,
        )
        .into_iter()
        .next()
        .unwrap(),
        TargetService::Notion => run_outbox(
            &r,
            &Publish { report: Some(OutboxTarget::new(s, "", "", "0123456789abcdef0123456789abcdef")), ..Publish::default() },
            "김요청",
            true,
            T0,
            Tz::Utc,
        )
        .into_iter()
        .next()
        .unwrap(),
    }
}

/// 게시 설정 읽기 — 칸마다 한 줄. 원본: PublishTests.swift `설정`
#[test]
fn 설정() {
    for r in &PUBLISH_ROWS {
        let v: Value = serde_json::from_str(r.json).unwrap_or(Value::Null);
        let (p, probs) = publish_parse(if v.is_null() { None } else { Some(&v) });
        let t = p
            .report
            .as_ref()
            .map(|x| format!("{} {} {}", x.service.raw_value(), x.space, x.parent))
            .unwrap_or_else(|| "-".to_string());
        let got = format!("{} {} {} | {}", mode_text(&p.mode), t, p.jira_comments, probs.join("; "));
        assert_eq!(got, r.want, "{}", r.note);
        let (back, _) = publish_parse(Some(&publish_json(&p)));
        assert_eq!(back, p, "{} — 온전히 읽은 설정은 다시 읽어도 같다", r.note);
    }
}

/// 단위 댓글 — 켰고 티켓 키가 있고 끝난 단위만. 원본: PublishTests.swift `댓글`
#[test]
fn 댓글() {
    let r = run("sample-run.json");
    let on = Publish { jira_comments: true, ..Publish::default() };
    let ids = |unit: i64, ticket: Option<&str>, p: &Publish| unit_outbox(&r, unit, ticket, p, T0).iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(1, Some("TODO-41"), &on), ["u1-jira-TODO-41"]);
    assert!(ids(1, Some("TODO-41"), &Publish::default()).is_empty(), "끄면 댓글 없음");
    assert!(ids(1, None, &on).is_empty(), "티켓 키 없으면 없음");
    assert!(ids(1, Some("../x"), &on).is_empty(), "티켓 키 꼴이 틀리면 없음");
    assert_eq!(ids(4, Some("TODO-44"), &on).len(), 1, "건너뜀도 댓글");
    assert!(outbox_id_ok("u1-jira-TODO-41") && outbox_id_ok("run-confluence") && !outbox_id_ok("run-x/../y"));
}

/// 보고서 글 — 받을 곳이 있을 때 하나, Confluence 만 첨부. 원본: PublishTests.swift `보고서`
#[test]
fn 보고서() {
    let r = run("sample-run.json");
    assert!(run_outbox(&r, &Publish::default(), "", true, T0, Tz::Utc).is_empty(), "받을 곳 없으면 글 없음");
    let c = item(TargetService::Confluence);
    let n = item(TargetService::Notion);
    assert_eq!(c.id, "run-confluence");
    assert_eq!(c.attachment.as_deref(), Some("report.html"));
    assert_eq!(c.connection, "atlassian");
    assert_eq!(n.id, "run-notion");
    assert_eq!(n.attachment, None);
    assert_eq!(n.connection, "notion");
    let no_file = run_outbox(
        &r,
        &Publish { report: Some(OutboxTarget::new(TargetService::Confluence, "", "T", "")), ..Publish::default() },
        "",
        false,
        T0,
        Tz::Utc,
    )
    .into_iter()
    .next()
    .unwrap();
    assert_eq!(no_file.attachment, None, "보고서 파일이 없으면 첨부 없음");
}

/// 출발 확인 알림 — 연결 없음·범위·토큰·만료. 원본: PublishTests.swift `알림`
#[test]
fn 알림() {
    let p = Publish { report: Some(OutboxTarget::new(TargetService::Confluence, "", "T", "")), jira_comments: true, ..Publish::default() };
    let none = HashSet::<String>::new();
    assert_eq!(publish_notes(&p, &[], &none, false, T0, Tz::Utc).len(), 2, "연결 없음 — 보고서·댓글 받을 곳 각각");
    let secret = HashSet::from(["atlassian".to_string()]);
    assert!(publish_notes(&p, &[atlassian("2027-01-31")], &secret, false, T0, Tz::Utc).is_empty());
    let old = [atlassian("2026-10-01")];
    let notes = publish_notes(&p, &old, &secret, false, T0, Tz::Utc);
    assert_eq!(notes.iter().filter(|n| n.contains("만료일이 지났습니다")).count(), 2, "{notes:?}");
    assert!(publish_notes(&Publish::default(), &[], &none, false, T0, Tz::Utc).is_empty());
}
