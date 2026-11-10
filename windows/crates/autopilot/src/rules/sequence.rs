//! (청크 Sequence 1/2 전반만 이식됨 — 이 첫 줄은 `tests/ported.rs` 가 「구현됐는데 PORTED.json 에 없는 규칙」으로 잡지 않게 하는 표시다. R-Sequence-b 가 후반을 옮긴 뒤 이 줄을 지우고 PORTED.json 항목을 넣는다)
//! 결과 기록의 흐름 그림(result.json 의 `sequence[].mermaid`)을 읽는 규칙. 원본: `Sources/AutopilotRules/Sequence.swift`.
//! mermaid 의 `sequenceDiagram` 작은 부분집합만 읽고, 그 밖의 줄이 하나라도 있으면 그리지 않고(`Unsupported`) 보고서가 원문을 그대로 보인다.
//! 순수 함수만 있다(파일·시각·환경 없음).
//!
//! 이식된 선언(원본 순서, 원본 1~197 줄): `SeqHead`(APSeqHead) · `SeqSide`(APSeqSide) · `SeqParticipant`(APSeqParticipant) · `SeqItem`(APSeqItem) ·
//! `SeqDiagram`(APSeqDiagram) · `SeqParse`(APSeqParse) · `SEQ_MAX_PARTICIPANTS`·`SEQ_MAX_ITEMS`·`SEQ_MAX_LABEL`(apSeqMax…) ·
//! 비공개 `ARROWS`·`BEGIN_KINDS`·`divide_of`·`first_word` · `parse_sequence`(apParseSequence) · `text_width`(apTextWidth).
//! 남은 선언(R-Sequence-b, 원본 「// 줄바꿈 표지 <br>·<br/>·<br /> 로 나눈 줄」 주석(199 줄)부터 끝까지): `apSeqLines` · 비공개 `n` ·
//! `apSequenceSVG` · 비공개 `arrowHead`. SVG 쪽은 `rules::prose` 의 `plain`·`escape_html` 을 쓴다(복사하지 않는다).
//!
//! Foundation 과 다른 점(원본 Swift 는 `Character`=자소 묶음 단위, 여기는 유니코드 글자(`char`) 단위):
//! - 길이 한도(`count` ≤ 80·300)와 `prefix(300)` 은 `chars()` 로 센다. 결합 글자·ZWJ 그림 글자·옛 한글 자모열은 원본이 하나로 세는 것을 여기서는 여럿으로 센다.
//! - `trimmingCharacters(in: .whitespaces)` 는 Zs 와 탭만 깎는다(`is_ws`). Rust 의 `char::is_whitespace` 와 다르다(줄바꿈 글자 제외).
//! - 원본의 `hasPrefix`·`range(of:)`·`firstIndex(of:)` 는 자소 묶음·정준 동치 기준이다. 여기는 바이트 열 그대로 견준다
//!   (화살표·` as `·`:` 바로 뒤에 결합 글자가 붙은 줄에서만 갈린다).

use std::collections::HashMap;

/// 메시지 화살표 머리. 원본: `APSeqHead`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqHead {
    None,
    Arrow,
    Cross,
    Open,
}

impl SeqHead {
    /// 원본 `rawValue`
    pub fn as_str(self) -> &'static str {
        match self {
            SeqHead::None => "none",
            SeqHead::Arrow => "arrow",
            SeqHead::Cross => "cross",
            SeqHead::Open => "open",
        }
    }
}

/// Note 의 자리. 원본: `APSeqSide`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqSide {
    Left,
    Right,
    Over,
}

impl SeqSide {
    /// 원본 `rawValue`
    pub fn as_str(self) -> &'static str {
        match self {
            SeqSide::Left => "left",
            SeqSide::Right => "right",
            SeqSide::Over => "over",
        }
    }
}

/// 참여자 한 명. 원본: `APSeqParticipant`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeqParticipant {
    pub id: String,
    pub label: String,
    pub actor: bool,
}

/// 그림의 한 줄. 원본: `APSeqItem`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeqItem {
    Message { from: usize, to: usize, text: String, dotted: bool, head: SeqHead, both: bool },
    Note { at: Vec<usize>, side: SeqSide, text: String },
    /// loop·alt·opt·par·critical·break·rect·box
    Begin { kind: String, label: String },
    /// else·and·option
    Divide { kind: String, label: String },
    End,
}

/// 그림 값. 원본: `APSeqDiagram`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SeqDiagram {
    pub title: String,
    pub participants: Vec<SeqParticipant>,
    pub items: Vec<SeqItem>,
    pub autonumber: bool,
}

/// 읽은 결과. 원본: `APSeqParse`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeqParse {
    Ok(SeqDiagram),
    /// 그리지 못한 첫 줄(1부터)과 까닭
    Unsupported { line: usize, why: String },
}

pub const SEQ_MAX_PARTICIPANTS: usize = 24;
pub const SEQ_MAX_ITEMS: usize = 300;
pub const SEQ_MAX_LABEL: usize = 300;

// 글자, 점선, 머리, 양쪽 — 긴 것부터
const ARROWS: [(&str, bool, SeqHead, bool); 10] = [
    ("<<-->>", true, SeqHead::Arrow, true),
    ("<<->>", false, SeqHead::Arrow, true),
    ("-->>", true, SeqHead::Arrow, false),
    ("--x", true, SeqHead::Cross, false),
    ("--)", true, SeqHead::Open, false),
    ("->>", false, SeqHead::Arrow, false),
    ("-->", true, SeqHead::None, false),
    ("-x", false, SeqHead::Cross, false),
    ("-)", false, SeqHead::Open, false),
    ("->", false, SeqHead::None, false),
];
const BEGIN_KINDS: [&str; 8] = ["loop", "alt", "opt", "par", "critical", "break", "rect", "box"];

fn divide_of(w: &str) -> Option<&'static str> {
    match w {
        "else" => Some("alt"),
        "and" => Some("par"),
        "option" => Some("critical"),
        _ => None,
    }
}

// Foundation `CharacterSet.whitespaces`: 일반 범주 Zs 와 탭(U+0009)
fn is_ws(c: char) -> bool {
    matches!(c, '\t' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

fn trim_ws(s: &str) -> &str {
    s.trim_matches(is_ws)
}

fn char_count(s: &str) -> usize {
    s.chars().count()
}

// 첫 빈칸·탭에서 둘로 나눈다. 빈칸이 없으면 (전부, "")
fn first_word(s: &str) -> (&str, &str) {
    match s.find([' ', '\t']) {
        None => (s, ""),
        Some(i) => (&s[..i], trim_ws(&s[i..])),
    }
}

fn valid_id(s: &str) -> bool {
    !s.is_empty() && char_count(s) <= 80 && !s.contains([' ', '\t', ':', ',', ';', '<', '>', '"'])
}

// `id as 이름` → (id, 이름). ` as ` 가 없으면 (전부, None)
fn split_as(s: &str) -> (&str, Option<&str>) {
    match s.find(" as ") {
        Some(p) => (trim_ws(&s[..p]), Some(trim_ws(&s[p + 4..]))),
        None => (s, None),
    }
}

fn participant(d: &mut SeqDiagram, index: &mut HashMap<String, usize>, id: &str, label: Option<&str>, actor: bool) -> Option<usize> {
    if let Some(&i) = index.get(id) {
        if let Some(l) = label {
            d.participants[i].label = l.to_string();
        }
        if actor {
            d.participants[i].actor = true;
        }
        return Some(i);
    }
    if d.participants.len() >= SEQ_MAX_PARTICIPANTS {
        return None;
    }
    d.participants.push(SeqParticipant { id: id.to_string(), label: label.unwrap_or(id).to_string(), actor });
    index.insert(id.to_string(), d.participants.len() - 1);
    Some(d.participants.len() - 1)
}

fn unsupported(line: usize, why: impl Into<String>) -> SeqParse {
    SeqParse::Unsupported { line, why: why.into() }
}

fn too_many() -> String {
    format!("참여자가 너무 많습니다({SEQ_MAX_PARTICIPANTS}까지)")
}

/// sequenceDiagram 원문 → 그림 값. 첫 의미 있는 줄은 `sequenceDiagram` 이어야 한다. 원본: `apParseSequence`
pub fn parse_sequence(src: &str) -> SeqParse {
    let mut d = SeqDiagram::default();
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut stack: Vec<String> = Vec::new();
    let mut started = false;
    let norm = src.replace("\r\n", "\n").replace('\r', "\n");
    for (k, raw) in norm.split('\n').enumerate() {
        let no = k + 1;
        let mut line = trim_ws(raw);
        if let Some(h) = line.strip_suffix(';') {
            line = trim_ws(h);
        }
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if !started {
            if line != "sequenceDiagram" {
                return unsupported(no, "sequenceDiagram 으로 시작하지 않습니다");
            }
            started = true;
            continue;
        }
        if d.items.len() >= SEQ_MAX_ITEMS {
            return unsupported(no, format!("줄이 너무 많습니다({SEQ_MAX_ITEMS}줄까지 그립니다)"));
        }
        let (w0, rest) = first_word(line);
        let w = w0.to_lowercase();
        match w.as_str() {
            "participant" | "actor" => {
                let (id, label) = split_as(rest);
                if !valid_id(id) || label.map_or(0, char_count) > SEQ_MAX_LABEL {
                    return unsupported(no, "참여자 이름을 읽지 못했습니다");
                }
                if participant(&mut d, &mut index, id, label, w == "actor").is_none() {
                    return unsupported(no, too_many());
                }
                continue;
            }
            "create" => {
                let (w1, r1) = first_word(rest);
                let w1 = w1.to_lowercase();
                if w1 != "participant" && w1 != "actor" {
                    return unsupported(no, "읽지 못한 줄입니다");
                }
                let (id, label) = split_as(r1);
                if !valid_id(id) || participant(&mut d, &mut index, id, label, w1 == "actor").is_none() {
                    return unsupported(no, "참여자 이름을 읽지 못했습니다");
                }
                continue;
            }
            "autonumber" => {
                d.autonumber = true;
                continue;
            }
            "activate" | "deactivate" | "destroy" => continue,
            "title" | "title:" => {
                let t = trim_ws(rest.strip_prefix(':').unwrap_or(rest));
                d.title = t.chars().take(SEQ_MAX_LABEL).collect();
                continue;
            }
            "acctitle:" | "accdescr:" => continue,
            "end" => {
                if rest.is_empty() && stack.is_empty() {
                    return unsupported(no, "여는 묶음 없이 end 가 있습니다");
                }
                if rest.is_empty() {
                    stack.pop();
                    d.items.push(SeqItem::End);
                    continue;
                }
                // 뒤에 글이 있으면 묶음 끝이 아니라 메시지 줄로 읽어 본다
            }
            "note" => {
                let (side0, r1) = first_word(rest);
                let side = side0.to_lowercase();
                let (s, targets) = if side == "over" {
                    (SeqSide::Over, r1)
                } else if side == "left" || side == "right" {
                    let (of, r2) = first_word(r1);
                    if of.to_lowercase() != "of" {
                        return unsupported(no, "Note 줄을 읽지 못했습니다");
                    }
                    (if side == "left" { SeqSide::Left } else { SeqSide::Right }, r2)
                } else {
                    return unsupported(no, "Note 줄을 읽지 못했습니다");
                };
                let Some(colon) = targets.find(':') else {
                    return unsupported(no, "Note 줄에 쌍점(:)이 없습니다");
                };
                // Swift `split(separator:)` 는 빈 조각을 버린다(공백뿐인 조각은 남아 다듬은 뒤 valid_id 가 거른다)
                let names: Vec<&str> = targets[..colon].split(',').filter(|p| !p.is_empty()).map(trim_ws).collect();
                let text = trim_ws(&targets[colon + 1..]);
                if !(1..=2).contains(&names.len())
                    || !names.iter().all(|n| valid_id(n))
                    || char_count(text) > SEQ_MAX_LABEL
                    || !(s == SeqSide::Over || names.len() == 1)
                {
                    return unsupported(no, "Note 줄을 읽지 못했습니다");
                }
                let mut at = Vec::new();
                for n in names {
                    let Some(i) = participant(&mut d, &mut index, n, None, false) else {
                        return unsupported(no, too_many());
                    };
                    at.push(i);
                }
                d.items.push(SeqItem::Note { at, side: s, text: text.to_string() });
                continue;
            }
            _ => {
                if BEGIN_KINDS.contains(&w.as_str()) {
                    if char_count(rest) > SEQ_MAX_LABEL {
                        return unsupported(no, "묶음 이름이 너무 깁니다");
                    }
                    // rect·box 의 색 글자는 그리지 않는다
                    let label = if w == "rect" || w == "box" { String::new() } else { rest.to_string() };
                    d.items.push(SeqItem::Begin { kind: w.clone(), label });
                    stack.push(w);
                    continue;
                }
                if let Some(parent) = divide_of(&w) {
                    if stack.last().map(String::as_str) != Some(parent) {
                        return unsupported(no, format!("{w0} 가 맞는 묶음 안에 있지 않습니다"));
                    }
                    if char_count(rest) > SEQ_MAX_LABEL {
                        return unsupported(no, "묶음 이름이 너무 깁니다");
                    }
                    d.items.push(SeqItem::Divide { kind: w, label: rest.to_string() });
                    continue;
                }
            }
        }
        // 메시지: 보내는 쪽 화살표 받는 쪽: 글
        let mut hit: Option<(usize, usize, bool, SeqHead, bool)> = None;
        for (i, c) in line.char_indices() {
            for (tok, dotted, head, both) in ARROWS {
                if line[i..].starts_with(tok) {
                    hit = Some((i, i + tok.len(), dotted, head, both));
                    break;
                }
            }
            if hit.is_some() || c == ':' {
                break;
            }
        }
        let Some((hs, he, dotted, head, both)) = hit else {
            return unsupported(no, "읽지 못한 줄입니다");
        };
        let from = trim_ws(&line[..hs]);
        let mut after = trim_ws(&line[he..]);
        if after.starts_with('+') || after.starts_with('-') {
            after = trim_ws(&after[1..]);
        }
        let (to, text) = match after.find(':') {
            Some(c) => (trim_ws(&after[..c]), trim_ws(&after[c + 1..])),
            None => (after, ""),
        };
        if !valid_id(from) || !valid_id(to) || char_count(text) > SEQ_MAX_LABEL {
            return unsupported(no, "메시지 줄을 읽지 못했습니다");
        }
        let (Some(a), Some(b)) = (participant(&mut d, &mut index, from, None, false), participant(&mut d, &mut index, to, None, false)) else {
            return unsupported(no, too_many());
        };
        d.items.push(SeqItem::Message { from: a, to: b, text: text.to_string(), dotted, head, both });
    }
    if !started {
        return unsupported(1, "비어 있습니다");
    }
    if d.participants.is_empty() {
        return unsupported(1, "참여자가 없습니다");
    }
    // 닫지 않은 묶음은 끝에서 닫는다
    while stack.pop().is_some() {
        d.items.push(SeqItem::End);
    }
    SeqParse::Ok(d)
}

/// 글자 폭 어림(px) — 한글·한자·전각·그림 글자는 1em, 그 밖은 0.6em. 원본: `apTextWidth`
pub fn text_width(s: &str, size: f64) -> f64 {
    let mut w = 0.0;
    for c in s.chars() {
        let v = c as u32;
        let wide = (0x1100..=0x11FF).contains(&v)
            || (0x2E80..=0xA4CF).contains(&v)
            || (0xAC00..=0xD7A3).contains(&v)
            || (0xF900..=0xFAFF).contains(&v)
            || (0xFE30..=0xFE4F).contains(&v)
            || (0xFF00..=0xFF60).contains(&v)
            || (0xFFE0..=0xFFE6).contains(&v)
            || (0x1F300..=0x1FAFF).contains(&v);
        w += size * if wide { 1.0 } else { 0.6 };
    }
    w
}
