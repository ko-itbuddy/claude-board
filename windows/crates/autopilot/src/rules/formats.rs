//! (청크 Formats 1/2 — 앞쪽 절반만 이식됨. 이 첫 줄은 `tests/ported.rs` 가 「구현됐는데 PORTED.json 에 없는 규칙」으로 잡지 않게 하는 표시다. R-Formats-b 가 뒤쪽을 옮긴 뒤 이 줄을 지우고 PORTED.json 항목을 넣는다)
//! 바깥에 올릴 글의 형식 변환. 원본: `Sources/AutopilotRules/Formats.swift`(248줄).
//! 올릴 글은 받는 곳과 상관없는 작은 문서 꼴 `Doc`(제목·문단·목록·코드·가로줄, 글 조각은 굵게·https 링크)로 만들고,
//! 올릴 때 Jira 문서 형식(ADF)·Confluence 저장 형식(XHTML)·Notion 블록으로 바꾼다. 글은 모델이 쓴 기록에서 오므로 믿지 않는다:
//! 만들 때 다듬고(`doc_text`) 저장 형식은 모든 글을 이스케이프하며 링크는 https 만(`safe_link`) 건다.
//!
//! 이식된 선언(원본 순서, 원본 1~136줄): `Span`(APSpan) · `Block`(APBlock) · `Doc`(APDoc) · 상수 여섯(`DOC_MARKER` …
//! `NOTION_BATCH_MAX`) · `doc_text`(apDocText) · `doc_code`(apDocCode) · `doc_clamp`(apDocClamp) · `adf`(apADF) · `storage`(apStorage).
//! 남은 선언(R-Formats-b, 원본 「// MARK: - Notion 블록」 138줄부터): `apNotionRich` · 비공개 `notionSpans`·`notionBlock` ·
//! `apNotionBlocks` · `apBatches` · `apDocJSON` · `apDocParse` · `apDocPlainLines`. 상수 `NOTION_*` 은 그쪽이 쓴다.
//!
//! Swift ↔ Rust 로 달라진 점:
//! - 글자 수·자르기는 Swift 의 `Character`(확장 자소 덩어리) 기준이다. 이 크레이트에는 `unicode-segmentation` 이 없어(Cargo 는 통합자 몫)
//!   `grapheme_len` 이 UAX #29 의 주요 규칙(CRLF·제어·결합 표시·ZWJ 이모지·국기 쌍·한글 자모)만 옮겼다. 그 밖의 드문 덩어리(Prepend·인도계 결합 등)는 다르게 셀 수 있다.
//! - 가장자리 빈칸(`.whitespaces`)은 Zs 범주와 탭이다(`ws`). `char::is_whitespace` 가 아니다.
//! - ADF 는 `serde_json::Value`(Swift `[String: Any]`)다. 이 크레이트는 `preserve_order` 라 키 순서가 만든 순서다 — Swift 사전은 순서가 없어 시험이 키를 정렬해 견준다.
//! - 제목 수준은 `i64` 다(Swift `Int`). 기본 인자는 Rust 에 없어 `doc_text`·`doc_code` 는 기본 상한, `doc_text_n`·`doc_code_n` 은 상한을 받는다.
//! - `doc_code` 는 `\n` 으로만 줄을 가른다. Swift `components(separatedBy:)` 가 `\r\n` 을 한 글자로 보고 안 가를 수 있는지는 이 머신(Windows)에서 못 쟀다.

use crate::rules::prose::{escape_html, plain, safe_link};
use serde_json::{json, Map, Value};
use unicode_normalization::char::is_combining_mark;

/// 글 조각 — `Text(글, 굵게)` 또는 `Link(글, 주소)`. 주소는 `safe_link` 를 지난 https 주소. 원본: `APSpan`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Span {
    Text(String, bool),
    Link(String, String),
}

/// 문서 블록. 원본: `APBlock`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    /// 수준 1~3
    Heading(i64, String),
    Paragraph(Vec<Span>),
    /// 줄마다 조각들
    Bullets(Vec<Vec<Span>>),
    Code(String),
    Rule,
}

/// 작은 문서 꼴. 원본: `APDoc`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Doc {
    pub blocks: Vec<Block>,
}

impl Doc {
    pub fn new(blocks: Vec<Block>) -> Doc {
        Doc { blocks }
    }
}

/// 바깥 글 첫 줄(사용자 확정). 원본: `apDocMarker`
pub const DOC_MARKER: &str = "오토파일럿 작성";
/// 글 조각 하나의 글자 수 상한. 원본: `apDocTextMax`
pub const DOC_TEXT_MAX: usize = 4000;
/// 문서 하나의 블록 수 상한. 원본: `apDocBlockMax`
pub const DOC_BLOCK_MAX: usize = 400;
/// 목록 하나의 줄 수 상한. 원본: `apDocListMax`
pub const DOC_LIST_MAX: usize = 200;
/// Notion 글 한 칸(rich_text 하나)의 글자 수 상한. 원본: `apNotionTextMax`
pub const NOTION_TEXT_MAX: usize = 2000;
/// Notion 에 한 번에 보내는 블록 수 상한. 원본: `apNotionBatchMax`
pub const NOTION_BATCH_MAX: usize = 100;

// ---- 확장 자소 덩어리(Swift Character) 세기 ----

fn is_control(c: char) -> bool {
    let v = c as u32;
    v < 0x20 || (0x7F..=0x9F).contains(&v) || v == 0x2028 || v == 0x2029
}

fn is_extend(c: char) -> bool {
    let v = c as u32;
    is_combining_mark(c) || v == 0x200C || v == 0x200D || (0x1F3FB..=0x1F3FF).contains(&v) || (0xE0020..=0xE007F).contains(&v)
}

fn is_pict(c: char) -> bool {
    let v = c as u32;
    (0x1F000..=0x1FAFF).contains(&v)
        || (0x2600..=0x27BF).contains(&v)
        || (0x2300..=0x23FF).contains(&v)
        || (0x2B00..=0x2BFF).contains(&v)
        || matches!(v, 0xA9 | 0xAE | 0x203C | 0x2049 | 0x2122 | 0x2139)
}

fn is_ri(c: char) -> bool {
    (0x1F1E6..=0x1F1FF).contains(&(c as u32))
}

// 1 L, 2 V, 3 T, 4 LV, 5 LVT, 0 그 밖
fn hangul(c: char) -> u8 {
    let v = c as u32;
    match v {
        0x1100..=0x115F | 0xA960..=0xA97C => 1,
        0x1160..=0x11A7 | 0xD7B0..=0xD7C6 => 2,
        0x11A8..=0x11FF | 0xD7CB..=0xD7FB => 3,
        0xAC00..=0xD7A3 => {
            if (v - 0xAC00).is_multiple_of(28) {
                4
            } else {
                5
            }
        }
        _ => 0,
    }
}

// 글 맨 앞 덩어리의 바이트 길이(글은 비어 있지 않다)
fn first_cluster_len(s: &str) -> usize {
    let mut it = s.char_indices();
    let Some((_, first)) = it.next() else { return 0 };
    let (mut prev, mut ri) = (first, usize::from(is_ri(first)));
    let (mut pict_open, mut zwj_pending) = (is_pict(first), false);
    for (i, c) in it {
        let (p, q) = (hangul(prev), hangul(c));
        let free = !is_control(prev) && !is_control(c); // 제어 글자 앞뒤는 늘 끊긴다(CR LF 만 예외)
        let join = (prev == '\r' && c == '\n')
            || (free
                && (is_extend(c)
                    || (prev == '\u{200D}' && zwj_pending && is_pict(c))
                    || (is_ri(prev) && is_ri(c) && ri % 2 == 1)
                    || (p == 1 && matches!(q, 1 | 2 | 4 | 5))
                    || (matches!(p, 4 | 2) && matches!(q, 2 | 3))
                    || (matches!(p, 5 | 3) && q == 3)));
        if !join {
            return i;
        }
        zwj_pending = if c == '\u{200D}' { pict_open } else { zwj_pending && is_extend(c) };
        if !is_extend(c) && !(prev == '\u{200D}' && is_pict(c)) {
            pict_open = is_pict(c);
        }
        ri = if is_ri(c) { ri + 1 } else { 0 };
        prev = c;
    }
    s.len()
}

// 덩어리가 n 개보다 많으면 n 개째 끝의 바이트 위치, 아니면 None
fn cut_after(s: &str, n: usize) -> Option<usize> {
    if s.len() <= n {
        return None; // 덩어리는 바이트보다 많을 수 없다
    }
    let mut at = 0;
    for _ in 0..n {
        at += first_cluster_len(&s[at..]);
    }
    (at < s.len()).then_some(at)
}

/// Swift `Character` 수 근사. 시험용으로 공개한다(상한 판정과 같은 규칙).
pub fn grapheme_len(s: &str) -> usize {
    let (mut at, mut n) = (0, 0);
    while at < s.len() {
        at += first_cluster_len(&s[at..]);
        n += 1;
    }
    n
}

// CharacterSet.whitespaces — Zs 범주와 탭
fn ws(c: char) -> bool {
    matches!(c, '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}')
}

/// 글 다듬기(기본 상한 4000, 가장자리 빈칸 지움). 원본: `apDocText`
pub fn doc_text(s: &str) -> String {
    doc_text_n(s, DOC_TEXT_MAX, true)
}

/// 글 다듬기 — 긴 줄표는 쉼표로(`plain`), 줄바꿈·탭은 빈칸으로, 화면을 속이는 글자(제어·방향 바꾸기·줄 구분자)는 지우고,
/// 상한 `n` 글자에서 자르고 `…` 를 붙인다. `trim` 이 거짓이면 앞뒤 빈칸을 남긴다(글 조각 사이 띄어쓰기). 원본: `apDocText`
pub fn doc_text_n(s: &str, n: usize, trim: bool) -> String {
    let mut o = String::new();
    for u in plain(s).chars() {
        let v = u as u32;
        if matches!(u, '\n' | '\t' | '\r') || v == 0x2028 || v == 0x2029 {
            o.push(' ');
            continue;
        }
        if v < 0x20 || (0x7F..=0x9F).contains(&v) || (0x202A..=0x202E).contains(&v) || (0x2066..=0x2069).contains(&v) || v == 0xFEFF {
            continue;
        }
        o.push(u);
    }
    // ` {2,}` → ` `
    let mut c = String::with_capacity(o.len());
    let mut prev_space = false;
    for ch in o.chars() {
        if ch == ' ' && prev_space {
            continue;
        }
        prev_space = ch == ' ';
        c.push(ch);
    }
    let t = if trim { c.trim_matches(ws) } else { c.as_str() };
    match cut_after(t, n) {
        Some(at) => format!("{}…", &t[..at]),
        None => t.to_string(),
    }
}

/// 코드 칸(기본 상한). 원본: `apDocCode`
pub fn doc_code(s: &str) -> String {
    doc_code_n(s, DOC_TEXT_MAX)
}

/// 코드 칸 — 줄바꿈은 남기고 나머지는 `doc_text` 와 같게, 전체를 `n` 글자에서 자른다(`…` 없이). 원본: `apDocCode`
pub fn doc_code_n(s: &str, n: usize) -> String {
    let joined = s.split('\n').map(|l| doc_text_n(l, n, true)).collect::<Vec<_>>().join("\n");
    match cut_after(&joined, n) {
        Some(at) => joined[..at].to_string(),
        None => joined,
    }
}

fn clamp_spans(a: &[Span]) -> Vec<Span> {
    a.iter()
        .filter_map(|s| match s {
            Span::Text(t, b) => {
                let x = doc_text_n(t, DOC_TEXT_MAX, false);
                if x.trim_matches(ws).is_empty() {
                    None
                } else {
                    Some(Span::Text(x, *b))
                }
            }
            Span::Link(t, u) => {
                let x = doc_text_n(t, 300, false);
                match safe_link(u) {
                    None => (!x.is_empty()).then_some(Span::Text(x, false)),
                    Some(ok) => Some(Span::Link(if x.is_empty() { ok.clone() } else { x }, ok)),
                }
            }
        })
        .collect()
}

/// 문서 다듬기 — 블록 수·목록 줄 수 상한, 빈 조각 빼기, 링크 다시 확인(만드는 쪽이 이미 했어도 한 번 더). 원본: `apDocClamp`
pub fn doc_clamp(d: &Doc) -> Doc {
    let mut out = Vec::new();
    for b in d.blocks.iter().take(DOC_BLOCK_MAX) {
        match b {
            Block::Heading(l, t) => {
                let x = doc_text_n(t, 300, true);
                if !x.is_empty() {
                    out.push(Block::Heading((*l).clamp(1, 3), x));
                }
            }
            Block::Paragraph(a) => {
                let s = clamp_spans(a);
                if !s.is_empty() {
                    out.push(Block::Paragraph(s));
                }
            }
            Block::Bullets(rows) => {
                let r: Vec<Vec<Span>> = rows.iter().take(DOC_LIST_MAX).map(|r| clamp_spans(r)).filter(|r| !r.is_empty()).collect();
                if !r.is_empty() {
                    out.push(Block::Bullets(r));
                }
            }
            Block::Code(t) => {
                let x = doc_code(t);
                if !x.is_empty() {
                    out.push(Block::Code(x));
                }
            }
            Block::Rule => out.push(Block::Rule),
        }
    }
    Doc::new(out)
}

// ---- Jira 문서 형식(ADF) ----

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    let mut m = Map::new();
    for (k, v) in pairs {
        m.insert(k.to_string(), v);
    }
    Value::Object(m)
}

fn adf_spans(a: &[Span]) -> Vec<Value> {
    a.iter()
        .filter_map(|s| match s {
            // ADF 는 빈 글 마디를 받지 않는다
            Span::Text(t, b) => {
                if t.is_empty() {
                    return None;
                }
                let mut n = obj(vec![("type", json!("text")), ("text", json!(t))]);
                if *b {
                    n["marks"] = json!([{"type": "strong"}]);
                }
                Some(n)
            }
            Span::Link(t, u) => {
                if t.is_empty() {
                    return None;
                }
                Some(json!({"type": "text", "text": t, "marks": [{"type": "link", "attrs": {"href": u}}]}))
            }
        })
        .collect()
}

/// Jira 문서 형식(ADF) 으로. 먼저 `doc_clamp` 를 지난다. 원본: `apADF`
pub fn adf(d0: &Doc) -> Value {
    let d = doc_clamp(d0);
    let mut content: Vec<Value> = Vec::new();
    for b in &d.blocks {
        match b {
            Block::Heading(l, t) => content.push(json!({"type": "heading", "attrs": {"level": l}, "content": [{"type": "text", "text": t}]})),
            Block::Paragraph(a) => content.push(json!({"type": "paragraph", "content": adf_spans(a)})),
            Block::Bullets(rows) => {
                let items: Vec<Value> = rows
                    .iter()
                    .map(|r| json!({"type": "listItem", "content": [{"type": "paragraph", "content": adf_spans(r)}]}))
                    .collect();
                content.push(json!({"type": "bulletList", "content": items}));
            }
            Block::Code(t) => content.push(json!({"type": "codeBlock", "content": [{"type": "text", "text": t}]})),
            Block::Rule => content.push(json!({"type": "rule"})),
        }
    }
    json!({"type": "doc", "version": 1, "content": content})
}

// ---- Confluence 저장 형식(XHTML) ----

fn storage_spans(a: &[Span]) -> String {
    a.iter()
        .map(|s| match s {
            Span::Text(t, b) => {
                if *b {
                    format!("<strong>{}</strong>", escape_html(t))
                } else {
                    escape_html(t)
                }
            }
            Span::Link(t, u) => format!("<a href=\"{}\">{}</a>", escape_html(u), escape_html(t)),
        })
        .collect()
}

/// Confluence 저장 형식으로. 매크로(`ac:…`)·CDATA 를 쓰지 않는다 — 글에 든 `]]>` 나 매크로 꼴이 그대로 글로 남게. 코드는 `<pre>`.
/// 먼저 `doc_clamp` 를 지난다. 원본: `apStorage`
pub fn storage(d0: &Doc) -> String {
    let d = doc_clamp(d0);
    let mut o = String::new();
    for b in &d.blocks {
        match b {
            Block::Heading(l, t) => o += &format!("<h{l}>{}</h{l}>", escape_html(t)),
            Block::Paragraph(a) => o += &format!("<p>{}</p>", storage_spans(a)),
            Block::Bullets(rows) => {
                o += "<ul>";
                for r in rows {
                    o += &format!("<li>{}</li>", storage_spans(r));
                }
                o += "</ul>";
            }
            Block::Code(t) => o += &format!("<pre>{}</pre>", escape_html(t)),
            Block::Rule => o += "<hr />",
        }
    }
    o
}
