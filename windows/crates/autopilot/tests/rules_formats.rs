//! (청크 Formats 1/2 — 앞쪽 절반의 시험만) 올릴 글의 형식 변환(`rules::formats`) 시험.
//! 원본: `Tests/AutopilotRulesTests/PublishTests.swift` 의 `FormatTests`(`@Suite("자율주행 올릴 글: 형식 변환")`, Connections·Outbox 시험과 한 파일).
//! `FormatTests` 의 시험 8개 가운데 이 파일이 옮긴 것은 `다듬기`(글 다듬기, 표 5줄)와 `상한`(문서 상한)이다. 같은 이름의 Rust 시험이다.
//! 남은 여섯은 R-Formats-b(또는 Report·Outbox 가 먼저 와야 하는 것)가 옮긴다:
//!   `노션글`(apNotionRich) · `묶음`(apBatches) — Formats 뒤쪽 절반 선언이 필요하다.
//!   `골든`·`표지`·`악성`·`왕복` — `item(…)`·`apReportDoc`·`run(…)`·`apOutboxParse` 와 골든 파일(`Tests/AutopilotRulesTests/Publish/*`)이 필요해
//!   Report·Outbox 이식이 끝난 뒤에야 옮길 수 있다(`악성` 의 `apStorage`·`apADF` 단언은 이 파일의 `extra_` 시험이 손으로 만든 문서로 미리 덮는다).
//! 표(`DOC_TEXT_ROWS`)는 Swift 시험 파일에서 기계로 옮긴 것이다(손으로 다시 치지 않았다): 문자열 이스케이프(`\n`·`\t`·`\u{…}`)는 Rust 와 같아 글자 그대로.
//! 끝의 `extra_` 시험은 Swift 시험에 없는 것(덩어리 세기·링크 걸러내기·ADF·저장 형식 직접 호출)이다.

use claude_board_autopilot::rules::formats::{
    adf, doc_clamp, doc_code, doc_code_n, doc_text, doc_text_n, grapheme_len, storage, Block, Doc, Span, DOC_MARKER, NOTION_BATCH_MAX,
    NOTION_TEXT_MAX,
};
use serde_json::json;

const DOC_TEXT_ROWS: [(&str, &str); 5] = [
    ("A — B", "A, B"),
    ("줄\n바꿈\t탭", "줄 바꿈 탭"),
    ("거\u{202E}꾸로", "거꾸로"),
    ("  빈칸   여럿  ", "빈칸 여럿"),
    ("\u{0007}종", "종"),
];

fn text(s: &str) -> Span {
    Span::Text(s.to_string(), false)
}

#[test]
fn 다듬기() {
    for (input, want) in DOC_TEXT_ROWS {
        assert_eq!(doc_text(input), want, "{input:?}");
    }
}

#[test]
fn 상한() {
    let mut blocks: Vec<Block> = (0..450).map(|_| Block::Paragraph(vec![Span::Text("x".repeat(5000), false)])).collect();
    blocks.push(Block::Bullets((0..300).map(|_| vec![text("줄")]).collect()));
    let c = doc_clamp(&Doc::new(blocks));
    assert_eq!(c.blocks.len(), 400);
    match &c.blocks[0] {
        Block::Paragraph(s) => match &s[0] {
            Span::Text(t, _) => assert_eq!(t.chars().count(), 4001),
            other => panic!("글 조각: {other:?}"),
        },
        other => panic!("문단: {other:?}"),
    }
    let b = doc_clamp(&Doc::new(vec![Block::Bullets((0..300).map(|_| vec![text("줄")]).collect())]));
    match &b.blocks[0] {
        Block::Bullets(rows) => assert_eq!(rows.len(), 200),
        other => panic!("목록: {other:?}"),
    }
}

// ---- 추가: Swift 시험에 없는 것 ----

#[test]
fn extra_상수() {
    assert_eq!(DOC_MARKER, "오토파일럿 작성");
    assert_eq!((NOTION_TEXT_MAX, NOTION_BATCH_MAX), (2000, 100));
}

#[test]
fn extra_덩어리_세기() {
    for (s, n) in [
        ("", 0),
        ("abc", 3),
        ("가나다", 3),
        ("e\u{301}", 1),
        ("a\u{301}b", 2),
        ("\r\n", 1),
        ("\n\n", 2),
        ("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}", 1),
        ("\u{1F1F0}\u{1F1F7}\u{1F1FA}\u{1F1F8}", 2),
        ("\u{1F1F0}\u{1F1F7}\u{1F1FA}", 2),
        ("\u{1100}\u{1161}\u{11A8}", 1),
        ("\u{1F44D}\u{1F3FD}", 1),
    ] {
        assert_eq!(grapheme_len(s), n, "{s:?}");
    }
}

#[test]
fn extra_상한에서_자르기() {
    assert_eq!(doc_text(&"x".repeat(4000)), "x".repeat(4000));
    assert_eq!(doc_text(&"x".repeat(4001)), format!("{}…", "x".repeat(4000)));
    assert_eq!(doc_text_n("abc", 2, true), "ab…");
    assert_eq!(doc_text_n("abc", 3, true), "abc");
    assert_eq!(doc_text_n("가나다", 2, true), "가나…");
    // 결합 글자는 앞 글자와 한 덩어리 — 잘리지 않는다
    assert_eq!(doc_text_n("e\u{301}e\u{301}", 1, true), "e\u{301}…");
    assert_eq!(doc_text_n("\u{1F468}\u{200D}\u{1F469}x", 1, true), "\u{1F468}\u{200D}\u{1F469}…");
}

#[test]
fn extra_다듬기_가장자리() {
    assert_eq!(doc_text_n("  a  b  ", 10, false), " a b ");
    assert_eq!(doc_text_n("  a  ", 10, true), "a");
    assert_eq!(doc_text("\u{A0}\u{3000}a\u{A0}"), "a");
    assert_eq!(doc_text("a\u{85}b\u{FEFF}c\u{2066}d\u{2069}e"), "abcde");
    assert_eq!(doc_text("줄\u{2028}둘\u{2029}셋\r끝"), "줄 둘 셋 끝");
    assert_eq!(doc_text(""), "");
}

#[test]
fn extra_코드_칸() {
    assert_eq!(doc_code("a\n\n b  \tc"), "a\n\nb c");
    assert_eq!(doc_code_n("abcdef\nxyz", 8), "abcdef\nx");
    // 줄마다 `…` 까지 붙여 자른 뒤 전체를 다시 n 글자로 자르므로 `…` 는 남지 않는다
    assert_eq!(doc_code_n("abcdef", 3), "abc");
    assert_eq!(doc_code_n("abcdef\nxy", 4), "abcd");
    assert_eq!(doc_code(""), "");
}

#[test]
fn extra_다듬기_문서() {
    let d = Doc::new(vec![
        Block::Heading(7, "큰 제목".into()),
        Block::Heading(-2, "작은 제목".into()),
        Block::Heading(2, "  ".into()),
        Block::Paragraph(vec![text("   "), Span::Text("굵게".into(), true), text(" 일반")]),
        Block::Paragraph(vec![text(" ")]),
        Block::Paragraph(vec![
            Span::Link("나쁜".into(), "javascript:alert(1)".into()),
            Span::Link("".into(), "javascript:alert(1)".into()),
            Span::Link("".into(), "https://example.com/a".into()),
            Span::Link("좋은".into(), "https://example.com/b".into()),
        ]),
        Block::Bullets(vec![vec![text("  ")], vec![text("줄")]]),
        Block::Bullets(vec![vec![text("")]]),
        Block::Code(String::new()),
        Block::Code(" x ".into()),
        Block::Rule,
    ]);
    let want = Doc::new(vec![
        Block::Heading(3, "큰 제목".into()),
        Block::Heading(1, "작은 제목".into()),
        Block::Paragraph(vec![Span::Text("굵게".into(), true), text(" 일반")]),
        Block::Paragraph(vec![
            text("나쁜"),
            Span::Link("https://example.com/a".into(), "https://example.com/a".into()),
            Span::Link("좋은".into(), "https://example.com/b".into()),
        ]),
        Block::Bullets(vec![vec![text("줄")]]),
        Block::Code("x".into()),
        Block::Rule,
    ]);
    assert_eq!(doc_clamp(&d), want);
    assert_eq!(doc_clamp(&want), want, "두 번 다듬어도 같다");
}

#[test]
fn extra_adf_꼴() {
    let d = Doc::new(vec![
        Block::Heading(2, "제목".into()),
        Block::Paragraph(vec![Span::Text("굵게".into(), true), text(" 일반"), Span::Link("링크".into(), "https://example.com/a?b=1&c=2".into())]),
        Block::Bullets(vec![vec![text("줄")]]),
        Block::Code("x".into()),
        Block::Rule,
    ]);
    let want = json!({"type": "doc", "version": 1, "content": [
        {"type": "heading", "attrs": {"level": 2}, "content": [{"type": "text", "text": "제목"}]},
        {"type": "paragraph", "content": [
            {"type": "text", "text": "굵게", "marks": [{"type": "strong"}]},
            {"type": "text", "text": " 일반"},
            {"type": "text", "text": "링크", "marks": [{"type": "link", "attrs": {"href": "https://example.com/a?b=1&c=2"}}]}]},
        {"type": "bulletList", "content": [{"type": "listItem", "content": [{"type": "paragraph", "content": [{"type": "text", "text": "줄"}]}]}]},
        {"type": "codeBlock", "content": [{"type": "text", "text": "x"}]},
        {"type": "rule"}]});
    assert_eq!(adf(&d), want);
    assert_eq!(adf(&Doc::default()), json!({"type": "doc", "version": 1, "content": []}));
    // 위험한 주소는 링크 표시가 아니라 글로만 남는다
    let evil = adf(&Doc::new(vec![Block::Paragraph(vec![Span::Link("나쁜".into(), "javascript:alert(1)".into())])]));
    assert!(!evil.to_string().contains("javascript:") && !evil.to_string().contains("\"link\""));
}

#[test]
fn extra_저장_형식_꼴() {
    let d = Doc::new(vec![
        Block::Heading(1, "<b>t</b>".into()),
        Block::Paragraph(vec![Span::Text("a&b".into(), true), Span::Link("L".into(), "https://example.com/x?y=1&z=2".into())]),
        Block::Bullets(vec![vec![text("1")], vec![text("2")]]),
        Block::Code("]]><ac:x>".into()),
        Block::Rule,
    ]);
    assert_eq!(
        storage(&d),
        "<h1>&lt;b&gt;t&lt;/b&gt;</h1><p><strong>a&amp;b</strong><a href=\"https://example.com/x?y=1&amp;z=2\">L</a></p><ul><li>1</li><li>2</li></ul><pre>]]&gt;&lt;ac:x&gt;</pre><hr />"
    );
    assert_eq!(storage(&Doc::default()), "");
    let evil = storage(&Doc::new(vec![Block::Paragraph(vec![
        Span::Link("나쁜".into(), "javascript:alert(1)".into()),
        Span::Link("데이터".into(), "data:text/html,x".into()),
    ])]));
    assert_eq!(evil, "<p>나쁜데이터</p>");
}
