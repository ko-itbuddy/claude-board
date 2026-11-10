//! (청크 Inbox 1/2 — 전반 시험만) 방향을 바꿀 말 규칙(`rules::inbox`) 시험. 원본: `Tests/AutopilotRulesTests/InboxTests.swift`.
//! Swift 시험 파일의 시험은 15개다. 이 파일이 옮긴 것은 `읽기`·`되읽기`·`이름`(스위트 「글 꼴」)·`단위`·`까닭`(「어느 단위에」)·`고르기`(「넘기기와 닫기」의 첫 시험)이다.
//! 남은 것은 R-Inbox-b 가 옮긴다: `판정뒤`·`턴뒤`·`닫기`·`글`·`줄`(「넘기기와 닫기」)·`받기`·`보내기`·`기록`(「받기와 보내기」)과
//! 시험 파일 끝의 `RestartTests`(`rules::control` 규칙, 시험 5개: `계획`·`글`·`멈춤`·`가르기`·`다시열기`).
//! `parseRows`·`bindRows` 표는 Swift 에서 기계로 옮겼다(`#"…"#` 없이 보통 글자 값이라 그대로 복사, 손으로 다시 치지 않았다).
//! 글자 값을 `String(repeating:)` 으로 만든 두 줄(본문 2001 자·2000 자)은 같은 식으로 `"가".repeat(n)` 을 쓴다.
//! 끝의 `추가_` 시험은 Swift 시험에 없는 것(경계·직렬화·Foundation 과의 차이를 지키는 것)이다.

use claude_board_autopilot::rules::inbox::{
    bind, compose, name_ok, next_for, now_for, parse, InboxEntry, InboxNote, InboxParse, InboxTo, INBOX_MAX_BYTES, INBOX_MAX_CHARS, INBOX_STATUSES,
};

// ── 글 꼴 ────────────────────────────────────────────

struct ParseRow {
    note: String,
    raw: String,
    /// "to unit at | 본문" 또는 "bad: 까닭"
    want: String,
}

// 원본 `show` — Swift 는 `at` 을 `"\($0)"`(Double 설명)으로 쓰고 여기는 `{}` 로 쓴다. 표의 값(1761800000.5)에서는 같다
fn show(p: &InboxParse) -> String {
    match p {
        InboxParse::Note(n) => format!(
            "{} {} {} | {}",
            n.to.as_str(),
            n.unit.map(|u| u.to_string()).unwrap_or_else(|| "-".to_string()),
            n.at.map(|a| format!("{a}")).unwrap_or_else(|| "-".to_string()),
            n.text
        ),
        InboxParse::Bad(w) => format!("bad: {w}"),
    }
}

fn parse_rows() -> Vec<ParseRow> {
    vec![
        ParseRow { note: "보드가 쓰는 꼴".to_string(), raw: "to: now\nunit: 3\nat: 1761800000.500\n\n정렬은 화면 순서와 같게\n".to_string(), want: "now 3 1761800000.5 | 정렬은 화면 순서와 같게".to_string() },
        ParseRow { note: "다음 단위부터, unit 없음, 여러 줄 본문".to_string(), raw: "to: next\n\n첫 줄\n둘째 줄\n".to_string(), want: "next - - | 첫 줄\n둘째 줄".to_string() },
        ParseRow { note: "머리 이름은 대소문자·앞뒤 빈칸을 가리지 않는다, 모르는 칸은 넘김".to_string(), raw: " To : now \nfrom: board\n\n말\n".to_string(), want: "now - - | 말".to_string() },
        ParseRow { note: "CRLF 도 읽는다".to_string(), raw: "to: now\r\n\r\n말\r\n".to_string(), want: "now - - | 말".to_string() },
        ParseRow { note: "to 없음".to_string(), raw: "unit: 3\n\n말\n".to_string(), want: "bad: 머리에 to: now 또는 to: next 가 없습니다".to_string() },
        ParseRow { note: "to 가 다른 값".to_string(), raw: "to: later\n\n말\n".to_string(), want: "bad: 머리 to 는 now 또는 next 입니다(later)".to_string() },
        ParseRow { note: "unit 이 0".to_string(), raw: "to: now\nunit: 0\n\n말\n".to_string(), want: "bad: 머리 unit 은 1 이상의 정수입니다(0)".to_string() },
        ParseRow { note: "at 이 수가 아님".to_string(), raw: "to: now\nat: 어제\n\n말\n".to_string(), want: "bad: 머리 at 은 수입니다(어제)".to_string() },
        ParseRow { note: "빈 줄이 없음".to_string(), raw: "to: now".to_string(), want: "bad: 머리와 본문 사이 빈 줄이 없습니다".to_string() },
        ParseRow { note: "머리 줄 꼴이 아님".to_string(), raw: "그냥 글\n\n말\n".to_string(), want: "bad: 머리 줄 꼴이 아닙니다: 그냥 글".to_string() },
        ParseRow { note: "본문이 빈칸뿐".to_string(), raw: "to: next\n\n   \n\n".to_string(), want: "bad: 본문이 비었습니다".to_string() },
        ParseRow { note: "본문이 상한보다 김".to_string(), raw: ["to: now\n\n", "가".repeat(2001).as_str()].concat(), want: "bad: 본문이 2000자보다 깁니다(2001자)".to_string() },
        ParseRow { note: "본문이 상한과 같으면 받는다".to_string(), raw: ["to: now\n\n", "가".repeat(2000).as_str()].concat(), want: ["now - - | ", "가".repeat(2000).as_str()].concat() },
    ]
}
const PARSE_ROWS_LEN: usize = 13;

fn bind_rows() -> Vec<BindRow> {
    vec![
        BindRow { note: "지금 도는 단위".to_string(), to: InboxTo::Now, seen: None, statuses: "d r p".to_string(), want: "waiting unit:2".to_string() },
        BindRow { note: "보낼 때 본 단위가 아직 돈다".to_string(), to: InboxTo::Now, seen: Some(2), statuses: "d r p".to_string(), want: "waiting unit:2".to_string() },
        BindRow { note: "보낼 때 본 단위가 끝났으면 다른 단위에 넘기지 않는다".to_string(), to: InboxTo::Now, seen: Some(1), statuses: "d r p".to_string(), want: "late".to_string() },
        BindRow { note: "보낼 때 본 단위가 아직 시작 전이면 그 단위".to_string(), to: InboxTo::Now, seen: Some(3), statuses: "d d p".to_string(), want: "waiting unit:3".to_string() },
        BindRow { note: "없는 단위".to_string(), to: InboxTo::Now, seen: Some(9), statuses: "d r".to_string(), want: "late".to_string() },
        BindRow { note: "도는 단위가 없으면(단위 사이·멈출 선 대기) 다음에 시작할 단위".to_string(), to: InboxTo::Now, seen: None, statuses: "d f p p".to_string(), want: "waiting unit:3".to_string() },
        BindRow { note: "남은 단위가 없음".to_string(), to: InboxTo::Now, seen: None, statuses: "d d".to_string(), want: "late".to_string() },
        BindRow { note: "다음 단위부터: 도는 단위 뒤의 첫 대기".to_string(), to: InboxTo::Next, seen: None, statuses: "d r p p".to_string(), want: "waiting from:3".to_string() },
        BindRow { note: "다음 단위부터: 뒤 단위가 건너뜀이면 그 뒤 대기".to_string(), to: InboxTo::Next, seen: Some(2), statuses: "d r s p".to_string(), want: "waiting from:4".to_string() },
        BindRow { note: "다음 단위부터: 도는 단위가 없으면 첫 대기".to_string(), to: InboxTo::Next, seen: None, statuses: "d d p".to_string(), want: "waiting from:3".to_string() },
        BindRow { note: "다음 단위부터: 뒤에 남은 단위가 없음".to_string(), to: InboxTo::Next, seen: None, statuses: "d r".to_string(), want: "late".to_string() },
    ]
}
const BIND_ROWS_LEN: usize = 11;

#[test]
fn 읽기() {
    let rows = parse_rows();
    assert_eq!(rows.len(), PARSE_ROWS_LEN);
    for r in &rows {
        assert_eq!(show(&parse(&r.raw)), r.want, "{}", r.note);
    }
}

#[test]
fn 되읽기() {
    let mut a = InboxNote::new(InboxTo::Now, "  한 줄  \n");
    a.unit = Some(2);
    a.at = Some(1761800000.25);
    let b = InboxNote::new(InboxTo::Next, "여러\r\n줄");
    for n in [a, b] {
        let InboxParse::Note(back) = parse(&compose(&n)) else {
            panic!("읽지 못함");
        };
        assert!(back.to == n.to && back.unit == n.unit && back.at == n.at);
        assert_eq!(back.text, n.text.replace("\r\n", "\n").trim_matches(char::is_whitespace));
    }
}

#[test]
fn 이름() {
    let cases = [
        ("20261030-140512-123.md", true),
        ("20261030-140512-123-2.md", true),
        ("a.md", true),
        (".tmp-20261030-140512-123.md", false),
        ("note.txt", false),
        ("../x.md", false),
        ("한글.md", false),
        (".md", false),
    ];
    for (name, want) in cases {
        assert_eq!(name_ok(name), want, "{name}");
    }
}

// ── 어느 단위에 붙일지 ───────────────────────────────

struct BindRow {
    note: String,
    to: InboxTo,
    seen: Option<i64>,
    /// 단위 상태를 빈칸으로(d done, r running, p pending, f failed, s skipped)
    statuses: String,
    /// "waiting unit:n" · "waiting from:n" · "late"
    want: String,
}

fn sts(s: &str) -> Vec<String> {
    s.split(' ')
        .filter(|x| !x.is_empty())
        .map(|x| match x {
            "d" => "done",
            "r" => "running",
            "p" => "pending",
            "f" => "failed",
            "s" => "skipped",
            "c" => "capped",
            other => other,
        })
        .map(|x| x.to_string())
        .collect()
}

#[test]
fn 단위() {
    let rows = bind_rows();
    assert_eq!(rows.len(), BIND_ROWS_LEN);
    for r in &rows {
        let mut note = InboxNote::new(r.to, "말");
        note.unit = r.seen;
        let b = bind(&note, &sts(&r.statuses));
        let got = if b.status == "late" {
            "late".to_string()
        } else if let Some(u) = b.unit {
            format!("waiting unit:{u}")
        } else if let Some(f) = b.from {
            format!("waiting from:{f}")
        } else {
            "?".to_string()
        };
        assert_eq!(got, r.want, "{}", r.note);
        if b.status == "late" {
            assert!(!b.why.clone().unwrap_or_default().is_empty(), "{}", r.note);
        }
    }
}

#[test]
fn 까닭() {
    let mut note = InboxNote::new(InboxTo::Now, "말");
    note.unit = Some(1);
    let b = bind(&note, &["capped", "running"]);
    assert_eq!(
        b.why.as_deref(),
        Some("보낼 때 본 단위(단위 1)가 이미 끝나(상한에서 멈춤) 넘기지 않았습니다. 다음 단위부터로 다시 보내거나 멈추고 다시 출발합니다")
    );
}

// ── 언제 넘길지 ──────────────────────────────────────

fn e(to: &str, status: &str, unit: Option<i64>, from: Option<i64>, units: &[i64], file: &str) -> InboxEntry {
    let mut x = InboxEntry::new(file, to, &format!("말 {file}"), 0.0, status);
    x.unit = unit;
    x.from = from;
    x.units = units.to_vec();
    x
}

#[test]
fn 고르기() {
    let es = vec![
        e("now", "waiting", Some(2), None, &[], "a"),
        e("now", "delivered", Some(2), None, &[2], "b"),
        e("now", "waiting", Some(3), None, &[], "c"),
        e("next", "waiting", None, Some(3), &[], "d"),
        e("next", "applied", None, Some(2), &[2], "e"),
        e("next", "late", None, None, &[], "f"),
        e("next", "unused", None, Some(2), &[], "g"),
    ];
    assert_eq!(now_for(&es, 2), vec![0]);
    assert_eq!(now_for(&es, 3), vec![2]);
    assert_eq!(next_for(&es, 2), Vec::<usize>::new()); // e 는 이미 단위 2 에 붙였다, d 는 3 부터
    assert_eq!(next_for(&es, 3), vec![3, 4]); // 붙인 방향은 뒤 단위에도
    assert_eq!(next_for(&es, 4), vec![3, 4]);
}

// ── 추가: Swift 시험에 없는 것 ───────────────────────

#[test]
fn 추가_상수와_상태_이름() {
    assert_eq!(INBOX_MAX_CHARS, 2000);
    assert_eq!(INBOX_MAX_BYTES, 16_384);
    assert_eq!(INBOX_STATUSES, ["waiting", "delivered", "applied", "late", "dropped", "unused", "bad"]);
}

#[test]
fn 추가_compose_글_꼴() {
    let mut n = InboxNote::new(InboxTo::Now, "  말  \r\n둘째\n");
    n.unit = Some(2);
    n.at = Some(5.0);
    assert_eq!(compose(&n), "to: now\nunit: 2\nat: 5.000\n\n말  \n둘째\n");
    assert_eq!(compose(&InboxNote::new(InboxTo::Next, "x")), "to: next\n\nx\n");
    n.at = Some(1.0005);
    assert!(compose(&n).contains("at: 1.000\n")); // 1.0005 는 이진수로 1.000499… 라 printf 와 같이 내림
}

#[test]
fn 추가_이름_길이_경계() {
    let stem81 = format!("a{}", "b".repeat(80));
    assert!(name_ok(&format!("{stem81}.md")));
    assert!(!name_ok(&format!("{stem81}b.md")));
    assert!(name_ok("a.b_c-d.md"));
    assert!(!name_ok("-a.md"));
    assert!(!name_ok("_a.md"));
    assert!(!name_ok("a b.md"));
    assert!(!name_ok("a.md\n"));
    assert!(!name_ok("a.MD"));
    assert!(!name_ok(""));
}

#[test]
fn 추가_읽기_경계() {
    let bad = |raw: &str| show(&parse(raw));
    assert_eq!(bad("to: now\nunit: +3\n\n말"), "now 3 - | 말");
    assert_eq!(bad("to: now\nunit: 1.5\n\n말"), "bad: 머리 unit 은 1 이상의 정수입니다(1.5)");
    assert_eq!(bad("to: now\nunit: -1\n\n말"), "bad: 머리 unit 은 1 이상의 정수입니다(-1)");
    assert_eq!(bad("to: NOW\n\n말"), "bad: 머리 to 는 now 또는 next 입니다(NOW)");
    assert_eq!(bad("to: now\nat: 1e999\n\n말"), "bad: 머리 at 은 수입니다(1e999)");
    assert_eq!(bad("to: now\nat: nan\n\n말"), "bad: 머리 at 은 수입니다(nan)");
    assert_eq!(bad("\n말"), "bad: 머리에 to: now 또는 to: next 가 없습니다");
    assert_eq!(bad("to: now\n \t\n 안 쪽  \n\n줄 "), "now - - | 안 쪽  \n\n줄");
    assert_eq!(bad("to: now\n\n"), "bad: 본문이 비었습니다");
    // 머리 줄 오류 글은 40 글자까지
    let long = "가".repeat(60);
    assert_eq!(bad(&format!("{long}\n\n말")), format!("bad: 머리 줄 꼴이 아닙니다: {}", "가".repeat(40)));
    // 같은 머리 칸이 둘이면 뒤가 이긴다
    assert_eq!(bad("to: now\nto: next\n\n말"), "next - - | 말");
}

#[test]
fn 추가_bind_범위_밖_단위() {
    let mut n = InboxNote::new(InboxTo::Now, "말");
    n.unit = Some(0);
    let b = bind(&n, &["running"]);
    assert_eq!(b.status, "late");
    assert_eq!(b.why.as_deref(), Some("보낼 때 본 단위(단위 0)가 이 실행에 없어 넘기지 않았습니다"));
    n.unit = Some(2);
    assert_eq!(bind(&n, &["running"]).why.as_deref(), Some("보낼 때 본 단위(단위 2)가 이 실행에 없어 넘기지 않았습니다"));
    // 끝난 꼴의 쉬운 말 다섯과 모르는 꼴
    for (st, word) in [("done", "통과"), ("failed", "미통과"), ("capped", "상한에서 멈춤"), ("stopped", "중단"), ("skipped", "건너뜀"), ("weird", "weird")] {
        n.unit = Some(1);
        let why = bind(&n, &[st, "running"]).why.unwrap();
        assert!(why.contains(&format!("가 이미 끝나({word}) 넘기지 않았습니다")), "{st}");
    }
    // next 는 도는 단위 앞의 대기를 건너뛴다(도는 단위 뒤의 첫 대기부터)
    let next = InboxNote::new(InboxTo::Next, "말");
    assert_eq!(bind(&next, &["pending", "running", "pending"]).from, Some(3));
    assert_eq!(bind(&next, &["pending", "running"]).status, "late");
    assert_eq!(bind(&next, &["pending", "pending"]).from, Some(1));
    // now 는 단위가 없으면 도는 단위, 없으면 첫 대기
    assert_eq!(bind(&InboxNote::new(InboxTo::Now, "말"), &["pending", "running", "pending"]).unit, Some(2));
    assert_eq!(bind(&InboxNote::new(InboxTo::Now, "말"), &["done", "pending", "pending"]).unit, Some(2));
    // 상태가 빈 실행
    let none: [&str; 0] = [];
    assert_eq!(bind(&InboxNote::new(InboxTo::Next, "말"), &none).why.as_deref(), Some("뒤에 남은 단위가 없어 붙일 곳이 없습니다"));
    assert_eq!(bind(&InboxNote::new(InboxTo::Now, "말"), &none).why.as_deref(), Some("남은 단위가 없어 넘기지 않았습니다"));
}

#[test]
fn 추가_next_for_에서_from_이_없으면_붙이지_않는다() {
    let es = vec![e("next", "waiting", None, None, &[], "a"), e("now", "waiting", Some(1), None, &[], "b")];
    assert_eq!(next_for(&es, 99), Vec::<usize>::new());
    assert_eq!(now_for(&es, 99), Vec::<usize>::new());
    assert_eq!(now_for(&es, 1), vec![1]);
}

#[test]
fn 추가_entry_직렬화는_빈_칸을_쓰지_않고_정수_시각은_정수로() {
    let mut x = InboxEntry::new("f.md", "now", "말", 50.0, "waiting");
    x.unit = Some(2);
    x.sent = Some(1761800000.5);
    assert_eq!(
        serde_json::to_string(&x).unwrap(),
        r#"{"file":"f.md","to":"now","text":"말","sent":1761800000.5,"received":50,"status":"waiting","unit":2,"units":[]}"#
    );
    // 옛 꼴(source·done 칸 없음)도 읽힌다, units 는 꼭 있어야 한다(Swift 합성 Codable 과 같다)
    let old: InboxEntry = serde_json::from_str(r#"{"file":"f.md","to":"next","text":"t","received":1.5,"status":"bad","units":[],"why":"w"}"#).unwrap();
    assert_eq!(old.source, None);
    assert_eq!(old.done, None);
    assert_eq!(old.why.as_deref(), Some("w"));
    assert_eq!(old.received, 1.5);
    assert!(serde_json::from_str::<InboxEntry>(r#"{"file":"f","to":"","text":"","received":1,"status":"bad"}"#).is_err());
    let back: InboxEntry = serde_json::from_str(&serde_json::to_string(&x).unwrap()).unwrap();
    assert_eq!(back, x);
}
