//! (청크 Import a/3 — 원본 1~199줄. PORTED.json 항목은 마지막 청크가 더한다)
//! 자율주행 가져오기(7단계 ⑫-7a, 2026-10-30, 설계 2장 "목록으로 이어 가기: 원천과 차례"). 원본: `Sources/AutopilotRules/Import.swift`.
//! 원천 셋(저장소 할 일 문서·바깥 목록·옛 자동화 옮기기)을 같은 꼴의 항목(원천 항목 키·제목·본문·상태·원천 자리)으로 모으고,
//! 원천 항목 키로 이미 끝난 것·진행 중인 것을 기록 저장소와 바깥 상태로 가린 뒤(apImportDecide), 남은 항목을 가져오기 세션(⑪-2 초안 세션 꼴)이
//! 단위 초안으로 쓰고, 그 결과를 기계로 본다(apImportCheck). 같은 항목이 다시 오면 새 단위를 만들지 않고 대기열의 그 줄(정의)을 잇는다.
//! 바깥(파일 읽기·HTTP·기록 저장소·시각)은 어댑터 몫이고, 여기는 값만 받는다. 이 청크는 원천과 항목·저장소 할 일 문서(MARK 두 절)다.
//! 이어 쓰기: 바깥 목록은 200줄(MARK `바깥 목록`)부터 R-Import-b, 가리기와 세션은 598줄부터 R-Import-c.

use std::collections::HashMap;

use crate::rules::formats::grapheme_len;
use crate::rules::harness::fnv64_data;
use crate::rules::meta::MetaField;
use crate::rules::scenarios::scenario_path_ok;

// MARK: - 원천과 항목

/// 원본: `APImportSource`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImportSource {
    Todo,
    List,
    Legacy,
}

impl ImportSource {
    /// 원본 `CaseIterable.allCases`(선언 차례)
    pub const ALL: [ImportSource; 3] = [ImportSource::Todo, ImportSource::List, ImportSource::Legacy];

    /// 원본 `rawValue`
    pub fn raw(self) -> &'static str {
        match self {
            ImportSource::Todo => "todo",
            ImportSource::List => "list",
            ImportSource::Legacy => "legacy",
        }
    }

    /// 원본 `init?(rawValue:)`
    pub fn from_raw(s: &str) -> Option<ImportSource> {
        ImportSource::ALL.into_iter().find(|v| v.raw() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            ImportSource::Todo => "저장소 할 일 문서",
            ImportSource::List => "바깥 목록",
            ImportSource::Legacy => "옛 자동화 옮기기",
        }
    }
}

/// 원본: `APItemState`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ItemState {
    #[default]
    Open,
    Done,
    Progress,
}

impl ItemState {
    /// 원본 `rawValue`
    pub fn raw(self) -> &'static str {
        match self {
            ItemState::Open => "open",
            ItemState::Done => "done",
            ItemState::Progress => "progress",
        }
    }

    /// 원본 `init?(rawValue:)`
    pub fn from_raw(s: &str) -> Option<ItemState> {
        match s {
            "open" => Some(ItemState::Open),
            "done" => Some(ItemState::Done),
            "progress" => Some(ItemState::Progress),
            _ => None,
        }
    }
}

/// 가져오기 항목 하나. 원본: `APImportItem`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImportItem {
    /// 원천 항목 키: todo:<경로>#<제목 꼴> · jira:<키> · notion:<id> · legacy:<폴더>/<id 또는 경로#꼴>
    pub key: String,
    pub title: String,
    /// 본문(근거 묶음의 절 글이 됨)
    pub body: String,
    pub state: ItemState,
    /// 원천의 상태 글([x]·Done·완료 등)
    pub state_note: String,
    /// 원천 자리(파일:줄, Jira 주소 등) — 사람이 보는 글
    pub place: String,
    /// http(s) 주소가 있을 때만
    pub url: String,
    /// todo·jira·notion·legacy(근거 묶음 메타의 source)
    pub source: String,
    /// 이슈 종류 등
    pub kind: String,
    pub tickets: Vec<String>,
    /// Jira 하위 작업의 상위 스토리 키
    pub story: Option<String>,
    pub fields: Vec<MetaField>,
}

impl ImportItem {
    pub fn new() -> Self {
        Self::default()
    }

    /// 제목·본문의 지문(기록 저장소 queue_items.digest) — 같은 키에 본문이 바뀌었는지 본다
    pub fn digest(&self) -> String {
        fnv(&format!("{}\n{}", norm(&self.title), norm(&self.body)))
    }

    /// 본문 요약(기록 저장소 queue_items.summary, 바뀐 본문을 줄 단위로 견줄 때) — 다듬은 줄 30개·2,000자까지
    pub fn body_lines(&self) -> Vec<String> {
        body_lines(&self.body)
    }
}

/// CharacterSet.whitespaces — Zs 범주와 탭(formats.rs·misses.rs 의 것과 같다, 거긴 비공개라 여기도 둔다 — 세 셋쨰, 나중에 묶는다)
fn swift_ws(c: char) -> bool {
    matches!(c, '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}')
}

/// `whitespacesAndNewlines`(swift_ws + 줄바꿈·줄 나누기 류)
fn is_ws_or_nl(c: char) -> bool {
    swift_ws(c) || matches!(c, '\n' | '\u{000B}' | '\u{000C}' | '\r' | '\u{0085}' | '\u{2028}' | '\u{2029}')
}

fn trim_ws(s: &str) -> &str {
    s.trim_matches(swift_ws)
}

/// 앞 `n` 묶음을 자른다(Swift `String.prefix`). takeover.rs 의 grapheme_prefix 와 같은 규칙(거긴 비공개라 여기도 둔다)
fn grapheme_prefix(t: &str, n: usize) -> &str {
    t.char_indices().nth(n).map_or(t, |(i, _)| &t[..i])
}

/// 본문 요약의 다듬은 줄들 — 빈 줄은 버리고, 200 묶음을 넘는 줄은 앞 199 묶음과 `…` 로, 줄 30·2,000 묶음까지 담는다. 원본: `apBodyLines`
pub fn body_lines(s: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut total = 0usize;
    for raw in s.split('\n') {
        let l = trim_ws(raw);
        if l.is_empty() {
            continue;
        }
        let t = if grapheme_len(l) > 200 {
            format!("{}…", grapheme_prefix(l, 199))
        } else {
            l.to_string()
        };
        let n = grapheme_len(&t);
        if total + n > 2000 || out.len() >= 30 {
            break;
        }
        out.push(t);
        total += n;
    }
    out
}

/// FNV-1a 64비트(16진수 16자) — 견주기에만 쓴다(암호 해시가 아니다; 같은 글이면 같은 값). 원본: `apFNV64` — 같은 알고리즘·같은 출력의
/// `harness::fnv64_data` 를 재사용한다(같은 글자열을 UTF-8 바이트로 돌린다)
pub fn fnv(s: &str) -> String {
    fnv64_data(s.as_bytes())
}

/// 견줄 꼴 — 앞뒤 빈칸·여러 빈칸·대소문자를 접는다. 원본: `apNorm`
pub fn norm(s: &str) -> String {
    let mut out = String::new();
    let mut sep = false;
    for c in s.to_lowercase().chars() {
        if is_ws_or_nl(c) {
            sep = true;
        } else {
            if sep && !out.is_empty() {
                out.push(' ');
            }
            sep = false;
            out.push(c);
        }
    }
    out
}

/// 제목 → 키에 쓸 꼴(소문자, 꾸밈 글자 빼고 빈칸은 -, 글자·숫자·한글·-·. 만, 60묶음까지).
/// 줄 번호가 아니라 제목이라 줄이 밀려도 같은 키. 원본: `apSlug`
pub fn slug(s: &str) -> String {
    let stripped = s.replace("~~", "").replace("**", "").replace("__", "").replace('`', "");
    let t = norm(&stripped);
    let mut out = String::new();
    for ch in t.chars() {
        if ch == ' ' {
            if !out.ends_with('-') {
                out.push('-');
            }
        } else if ch.is_alphabetic() || ch.is_numeric() || ch == '-' || ch == '.' {
            out.push(ch);
        }
    }
    let x = out.trim_matches(|c| c == '-' || c == '.');
    if x.is_empty() {
        return "item".to_string();
    }
    grapheme_prefix(x, 60).to_string()
}

// MARK: - 저장소 할 일 문서(체크박스·목록 항목)

/// 머리 하나(level·제목·줄 번호, 줄은 1부터). 원본: `APTodoHeading`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TodoHeading {
    pub level: i64,
    pub title: String,
    pub line: i64,
}

impl TodoHeading {
    pub fn new(level: i64, title: &str, line: i64) -> Self {
        Self { level, title: title.to_string(), line }
    }
}

/// 할 일 문서 크기 상한(바이트). 원본: `apTodoMaxBytes`
pub const TODO_MAX_BYTES: usize = 512 << 10;
/// 항목 수 상한. 원본: `apTodoMaxItems`
pub const TODO_MAX_ITEMS: usize = 200;
/// 글 문서 확장자. 원본: `apTodoExtensions`
pub const TODO_EXTENSIONS: [&str; 4] = ["md", "markdown", "txt", "org"];

/// NSString.pathExtension — 마지막 경로 조각의 마지막 온점 뒤 소문자(온점 앞에 다른 글자가 없으면 빈 글자). 원본 지역 쓰임의 손판
fn path_extension(rel: &str) -> String {
    let last = rel.rsplit('/').next().unwrap_or("");
    match last.rfind('.') {
        Some(i) if i > 0 => last[i + 1..].to_lowercase(),
        _ => String::new(),
    }
}

/// 고를 수 있는 파일 — 저장소 안 상대 경로, 숨은 자리·.. 없음, 글 문서 확장자. 원본: `apTodoPathOK`
pub fn todo_path_ok(rel: &str) -> bool {
    if !scenario_path_ok(rel) || rel.split('/').any(|part| part.starts_with('.')) {
        return false;
    }
    TODO_EXTENSIONS.contains(&path_extension(rel).as_str())
}

/// 할 일 문서 후보(파일 목록에서 고를 때 위로 올릴 이름). 원본: `apTodoLikely`
pub fn todo_likely(rel: &str) -> bool {
    let n = rel.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_lowercase();
    ["todo", "backlog", "roadmap", "tasks", "할 일", "할일", "원장", "ledger", "plan"]
        .iter()
        .any(|word| n.contains(word))
}

/// 문서의 머리 차례(울타리 ```·~~~ 안은 세지 않는다). 원본: `apTodoHeadings`
pub fn todo_headings(text: &str) -> Vec<TodoHeading> {
    let mut out: Vec<TodoHeading> = vec![];
    let mut fence = false;
    for (i, raw) in text.split('\n').enumerate() {
        let l = trim_ws(raw);
        if l.starts_with("```") || l.starts_with("~~~") {
            fence = !fence;
            continue;
        }
        if fence {
            continue;
        }
        if let Some((level, title)) = heading(l) {
            out.push(TodoHeading::new(level, &title, i as i64 + 1));
        }
    }
    out
}

/// 머리 줄 꼴 손판 — `#` 1~6개 + 빈칸 하나 뒤 제목(끝의 `#`·빈칸은 벗긴다). 원본 지역 함수 `heading`
fn heading(l: &str) -> Option<(i64, String)> {
    if !l.starts_with('#') {
        return None;
    }
    let n = l.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&n) || grapheme_len(l) <= n {
        return None;
    }
    if l.chars().nth(n) != Some(' ') {
        return None;
    }
    let rest: String = l.chars().skip(n).collect();
    let t = rest.trim_matches(|c| c == '#' || swift_ws(c)).to_string();
    if t.is_empty() {
        None
    } else {
        Some((n as i64, t))
    }
}

/// 원본 지역 정규식 `^([ \t]*)([-*+]|[0-9]{1,3}[.)])[ \t]+(\[([ xX~/>-])\][ \t]+)?(.*)$` 의 손판 —
/// (들여쓰기 폭, 상자 글자, 제목). 들여쓰기 폭은 탭이 4. regex 룩어라운드 부재 원칙에 따라 손으로 훑는다
fn list_line(raw: &str) -> Option<(usize, Option<char>, &str)> {
    let indent = raw.len() - raw.trim_start_matches([' ', '\t']).len();
    let rest = &raw[indent..];
    // 마커: - * + 또는 숫자 1~3개 뒤 . )
    let marker = match rest.chars().next() {
        Some('-') | Some('*') | Some('+') => 1,
        Some(c) if c.is_ascii_digit() => {
            let digits = rest.chars().take_while(|c| c.is_ascii_digit()).count();
            match rest[digits..].chars().next() {
                Some('.') | Some(')') if digits <= 3 => digits + 1,
                _ => return None,
            }
        }
        _ => return None,
    };
    let after_marker = &rest[marker..];
    // [ \t]+ — 마커 뒤 빈칸 하나 이상은 필수
    let gap = after_marker.len() - after_marker.trim_start_matches([' ', '\t']).len();
    if gap == 0 {
        return None;
    }
    let rest = &after_marker[gap..];
    // 선택 상자 [x]·[X]·[-]·[~]·[/]·[>]·[ ] + 빈칸 하나 이상
    let r_bytes = rest.as_bytes();
    let box_ok = |c: u8| matches!(c, b' ' | b'x' | b'X' | b'~' | b'/' | b'>' | b'-');
    if r_bytes.first() == Some(&b'[')
        && r_bytes.len() > 2
        && box_ok(r_bytes[1])
        && r_bytes[2] == b']'
    {
        let after_close = &rest[3..];
        let gap2 = after_close.len() - after_close.trim_start_matches([' ', '\t']).len();
        if gap2 > 0 {
            return Some((indent, Some(r_bytes[1] as char), &after_close[gap2..]));
        }
    }
    Some((indent, None, rest))
}

/// 들여쓰기 폭 — 탭은 4. 원본 지역 함수 `indentWidth`
fn indent_width(s: &str) -> usize {
    s.chars().take_while(|c| *c == ' ' || *c == '\t').map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

/// 끝난 표시: [x]·[-](그만둠)·제목 전체 취소선·✅·✔·제목 끝 (완료)·(끝남)·(done). 진행 중: [~]·[/]·[>]. 원본: `apTodoMarkState`
pub fn todo_mark_state(r#box: Option<&str>, title: &str) -> (ItemState, String) {
    if let Some(b) = r#box {
        match b {
            "x" | "X" => return (ItemState::Done, "[x]".to_string()),
            "-" => return (ItemState::Done, "[-]".to_string()),
            "~" | "/" | ">" => return (ItemState::Progress, format!("[{b}]")),
            _ => {}
        }
    }
    let t = trim_ws(title);
    if t.starts_with("~~") && t.ends_with("~~") && grapheme_len(t) > 4 {
        return (ItemState::Done, "취소선".to_string());
    }
    if t.starts_with('✅') || t.starts_with('✔') {
        return (ItemState::Done, grapheme_prefix(t, 1).to_string());
    }
    let low = t.to_lowercase();
    for m in ["(완료)", "(끝남)", "(done)"] {
        if low.ends_with(m) {
            return (ItemState::Done, m.to_string());
        }
    }
    (ItemState::Open, if r#box.is_none() { "목록 항목".to_string() } else { "[ ]".to_string() })
}

/// 진행 중 항목을 본문까지 합쳐 내보낸다
fn flush_cur(cur: &mut Option<(ImportItem, Vec<String>)>, out: &mut Vec<ImportItem>) {
    if let Some((item, lines)) = cur.take() {
        let mut done = item;
        done.body = lines.join("\n");
        out.push(done);
    }
}

/// 체크박스·불릿·번호 목록을 항목으로. `section` 은 그 절(머리)의 항목만 걷는다.
/// 본문은 두 칸 이상 들인 잇는 줄(`indentWidth` 이 2 이상, 탭은 4)이고, 울타리 안 줄은 항목이 안 되지만 진행 중 항목의 본문엔 들어간다.
/// 같은 제목은 `~2`·`~3` 을 붙여 서로 다른 키를 가진다. 원본: `apListItems`
pub fn list_items(
    text: &str,
    path: &str,
    section: Option<&str>,
    key_prefix: &str,
    source: &str,
) -> Vec<ImportItem> {
    let mut out: Vec<ImportItem> = vec![];
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut cur: Option<(ImportItem, Vec<String>)> = None;
    let mut in_section = section.is_none();
    let mut section_level = 0i64;
    let mut fence = false;

    for (i, raw) in text.split('\n').enumerate() {
        let l = trim_ws(raw);
        if l.starts_with("```") || l.starts_with("~~~") {
            fence = !fence;
            if let Some((_, lines)) = cur.as_mut() {
                lines.push(l.to_string());
            }
            continue;
        }
        if fence {
            if let Some((_, lines)) = cur.as_mut() {
                lines.push(l.to_string());
            }
            continue;
        }
        if let Some((level, title)) = heading(l) {
            flush_cur(&mut cur, &mut out);
            if let Some(sec) = section {
                if norm(&title) == norm(sec) {
                    in_section = true;
                    section_level = level;
                } else if in_section && level <= section_level {
                    in_section = false;
                }
            }
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((ind, boxc, rest)) = list_line(raw) {
            if ind < 2 {
                flush_cur(&mut cur, &mut out);
                let t0 = trim_ws(rest).to_string();
                if t0.is_empty() || out.len() >= TODO_MAX_ITEMS {
                    continue;
                }
                let box_str = boxc.map(|c| c.to_string());
                let (state, state_note) = todo_mark_state(box_str.as_deref(), &t0);
                let title = if t0.starts_with("~~") && t0.ends_with("~~") && grapheme_len(&t0) > 4 {
                    trim_ws(&t0[2..t0.len() - 2]).to_string()
                } else {
                    t0.clone()
                };
                let title = grapheme_prefix(&title, 300).to_string();
                let base = slug(&title);
                let counter = seen.entry(base.clone()).or_insert(0);
                *counter += 1;
                let n = *counter;
                let key = if n > 1 {
                    format!("{}{}#{}~{}", key_prefix, path, base, n)
                } else {
                    format!("{}{}#{}", key_prefix, path, base)
                };
                cur = Some((
                    ImportItem {
                        key,
                        title,
                        body: String::new(),
                        state,
                        state_note,
                        place: format!("{}:{}", path, i + 1),
                        url: String::new(),
                        source: source.to_string(),
                        kind: "할 일".to_string(),
                        tickets: vec![],
                        story: None,
                        fields: vec![],
                    },
                    vec![],
                ));
                continue;
            }
        }
        // 잇는 줄 — 빈 줄은 건너뛰고, 두 칸 이상 들였으면 본문에 더하고, 아니면 앞 항목이 여기서 끝난다
        let t = trim_ws(raw);
        if t.is_empty() || cur.is_none() {
            continue;
        }
        if indent_width(raw) >= 2 {
            cur.as_mut().unwrap().1.push(t.to_string());
        } else {
            flush_cur(&mut cur, &mut out);
        }
    }
    flush_cur(&mut cur, &mut out);
    out
}

/// 저장소 할 일 문서 → 항목(키 접두사 `todo:`, 원천 `todo`). 원본: `apTodoItems`
pub fn todo_items(text: &str, path: &str, section: Option<&str>) -> Vec<ImportItem> {
    list_items(text, path, section, "todo:", "todo")
}

// R-Import-b 가 여기서 이어 쓴다


