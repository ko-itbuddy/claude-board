//! (청크 Reading a/2 — 원본 1~180줄. PORTED.json 항목은 마지막 청크가 더한다)
//! 자율주행 읽기(4단계 ⑩-2): 링크 → 읽을 대상, 읽기 요청 꼴(GET 만), 그림 주소 허용. 원본: `Sources/AutopilotRules/Reading.swift` 1~180줄.
//! 다음 청크(Reading b)는 181줄(MARK 응답 → 메타)부터 이어 쓴다.
//!
//! Swift ↔ Rust 로 달라진 점:
//! - `URLComponents(string:)` 는 `connections.rs` 의 `parse_url`(RFC 3986 손으로 옮긴 판)을 확장한 비공개 `parse_link_url` 로 읽는다. 차이:
//!   Foundation 은 경로·질의·조각·사용자 정보의 비 ASCII 글자(…/회의실-예약-0123…, ?p=…)를 받고 ReadingTests 의 견본 줄이 그렇다. 여기도 받는다. 호스트는 ASCII 만
//!   (유니코드 호스트는 IDNA 변환을 옮기지 않았다 — 그런 링크는 꼴이 틀림으로 간다), `[::1]` IP 표기·빈 포트·65535 넘는 포트는 connections.rs 와 같이 받지 않는다.
//! - 정규식 다섯(`^[A-Z][A-Z0-9_]*-[0-9]{1,9}$`(Outbox 의 apIssueKeyPattern — `tickets::issue_key_ok` 을 쓴다)·`^[0-9]{1,20}$`·
//!   `^[A-Za-z0-9]{10,64}$`·`^[0-9]{1,9}:[0-9]{1,9}$`·`^[A-Za-z0-9_-]{1,64}$`(MCPRead 의 apMCPServerNameOK — 아직 이식 전이라 비공개 `mcp_server_name_ok`)) 는
//!   손으로 옮긴 판별자다. ICU 의 `$` 는 끝의 줄바꿈 하나 앞에서도 맞으므로 원본은 `"TODO-7\n"` 도 받을 수 있다(직접 재지 못함). 여기는 받지 않는다(더 엄격).
//! - `now: Date`·`tz: TimeZone` 은 `now: f64`(유닉스 초)·`tz: stopline::Tz` 로 바꿨다(connections.rs 의 `ap_expiry` 와 같은 바꿈).
//! - `secrets: Set<String>` 은 `&HashSet<String>`(carry.rs 의 같은 바꿈).
//! - 빈칸 깎기는 `.whitespacesAndNewlines` 대신 Rust `char::is_whitespace`(유니코드 White_Space — 같은 목록)를 썼다.
//! - `addingPercentEncoding` 의 16진수는 원본처럼 대문자 두 자리다. `fields` 의 기본 인자는 부르는 쪽이 `JIRA_READ_FIELDS` 를 넣는다.
//! - 비밀 가림은 이 청크에 비밀이 없어 그대로다(요청의 `auth` 는 꼴만 담는다).

use std::collections::BTreeMap;
use std::collections::HashSet;

use crate::rules::connections::{
    ap_expiry, auth_for, same_site, service_base, url_host, Auth, Connection, Expiry, HttpRequest, Service, UrlParts, NOTION_VERSION,
};
use crate::rules::formats::grapheme_len;
use crate::rules::links::{link_info, ApLink};
use crate::rules::stopline::Tz;
use crate::rules::tickets::issue_key_ok;

// ── 링크 → 읽을 대상 ─────────────────────────────────

/// 링크 하나를 무엇으로 읽을지. 원본: `APReadTarget`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadTarget {
    Jira { key: String },
    Confluence { page: String },
    /// 8-4-4-4-12 꼴
    Notion { page: String },
    /// node 는 API 꼴(12:4)
    Figma { file: String, node: Option<String> },
    /// 내장하지 않음 — MCP 읽기 세션으로
    Web,
    /// 주소 꼴이 틀림
    Bad(String),
}

impl ReadTarget {
    /// 읽을 서비스. Web·Bad 는 없다. 원본: `APReadTarget.service`
    pub fn service(&self) -> Option<Service> {
        match self {
            ReadTarget::Jira { .. } | ReadTarget::Confluence { .. } => Some(Service::Atlassian),
            ReadTarget::Notion { .. } => Some(Service::Notion),
            ReadTarget::Figma { .. } => Some(Service::Figma),
            ReadTarget::Web | ReadTarget::Bad(_) => None,
        }
    }

    /// 사람이 보는 종류 이름. 원본: `APReadTarget.label`
    pub fn label(&self) -> &'static str {
        match self {
            ReadTarget::Jira { .. } => "Jira",
            ReadTarget::Confluence { .. } => "Confluence",
            ReadTarget::Notion { .. } => "Notion",
            ReadTarget::Figma { .. } => "Figma",
            ReadTarget::Web | ReadTarget::Bad(_) => "웹",
        }
    }

    /// 근거 묶음의 자료 번호 앞 글자. Web·Bad 는 `web`. 원본: `APReadTarget.sourceKey`
    pub fn source_key(&self) -> &'static str {
        match self {
            ReadTarget::Jira { .. } => "jira",
            ReadTarget::Confluence { .. } => "confluence",
            ReadTarget::Notion { .. } => "notion",
            ReadTarget::Figma { .. } => "figma",
            ReadTarget::Web | ReadTarget::Bad(_) => "web",
        }
    }
}

/// Notion 주소 끝(또는 ?p=)의 32자리 16진수 → 8-4-4-4-12. 원본: `apNotionID`
pub fn notion_id(s: &str) -> Option<String> {
    let hex: String = s.to_lowercase().chars().filter(char::is_ascii_hexdigit).collect();
    if hex.len() != 32 {
        return None;
    }
    let a = hex.as_bytes();
    let f = |r: std::ops::Range<usize>| std::str::from_utf8(&a[r]).unwrap_or_default();
    Some([f(0..8), f(8..12), f(12..16), f(16..20), f(20..32)].join("-"))
}

// ── URLComponents(string:) 의 손으로 옮긴 판 ─────────

fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

fn is_sub_delim(b: u8) -> bool {
    matches!(b, b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'=')
}

/// `allowed` 글자·비 ASCII 글자·`%XX` 만으로 된 칸인가(Foundation 은 비 ASCII 경로·질의를 받는다)
fn comp_ok(s: &str, extra: &[u8]) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c == b'%' {
            if i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit() {
                return false;
            }
            i += 3;
            continue;
        }
        if !(c >= 0x80 || is_unreserved(c) || is_sub_delim(c) || extra.contains(&c)) {
            return false;
        }
        i += 1;
    }
    true
}

/// 퍼센트 부호를 푼다(말은 UTF-8 이라 되살아난 바이트만 U+FFFD 로 바뀐다)
fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = |x: u8| (x as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (h(b[i + 1]), h(b[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// 원본 `URLComponents(string:)` — `connections::parse_url` 의 확장판(머리 설명). 꼴이 틀리면 None. 스킴 없는 글도 None 이다
fn parse_link_url(s: &str) -> Option<UrlParts> {
    let (scheme, rest) = s.split_once(':')?;
    let sb = scheme.as_bytes();
    if sb.is_empty() || !sb[0].is_ascii_alphabetic() || !sb.iter().all(|&c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'-' | b'.')) {
        return None;
    }
    let rest = rest.strip_prefix("//")?;
    // 조각 → 질의 → 칸(authority)·경로 순으로 뗀다
    let (rest, fragment) = match rest.split_once('#') {
        Some((a, f)) => (a, Some(f)),
        None => (rest, None),
    };
    let (rest, query) = match rest.split_once('?') {
        Some((a, q)) => (a, Some(q)),
        None => (rest, None),
    };
    let (auth, path) = match rest.find('/') {
        Some(i) => rest.split_at(i),
        None => (rest, ""),
    };
    if !comp_ok(path, b":@/") || !query.is_none_or(|q| comp_ok(q, b":@/?")) || !fragment.is_none_or(|f| comp_ok(f, b":@/?")) {
        return None;
    }
    if auth.matches('@').count() > 1 {
        return None;
    }
    let (userinfo, hostport) = match auth.split_once('@') {
        Some((u, h)) => (Some(u), h),
        None => (None, auth),
    };
    let (user, password) = match userinfo {
        None => (None, None),
        Some(u) => {
            if !comp_ok(u, b":") {
                return None;
            }
            match u.split_once(':') {
                Some((a, p)) => (Some(pct_decode(a)), Some(pct_decode(p))),
                None => (Some(pct_decode(u)), None),
            }
        }
    };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => {
            if p.is_empty() || p.len() > 5 || !p.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let n: u32 = p.parse().ok()?;
            if n > 65535 {
                return None;
            }
            (h, Some(n))
        }
        None => (hostport, None),
    };
    // 호스트는 ASCII 만(유니코드 호스트의 IDNA 는 옮기지 않았다)
    if !host.bytes().all(is_unreserved) {
        return None;
    }
    Some(UrlParts {
        scheme: Some(scheme.to_string()),
        user,
        password,
        host: Some(host.to_string()),
        port,
        path: pct_decode(path),
        query: query.map(str::to_string),
        fragment: fragment.map(str::to_string),
    })
}

/// 원본 `URLComponents.queryItems` — 질의 (이름, 값) 목록(퍼센트 풀이, `+` 는 빈칸으로 바꾸지 않는다). 이름만 있고 값이 없는 칸도 남긴다
fn query_items(q: &str) -> Vec<(String, Option<String>)> {
    q.split('&')
        .map(|pair| match pair.split_once('=') {
            Some((n, v)) => (pct_decode(n), Some(pct_decode(v))),
            None => (pct_decode(pair), None),
        })
        .collect()
}

/// 질의 항목 중 그 이름의 첫 값. 값이 없는 칸(None)은 건너뛴 것이 아니라 None 이다(원본의 `?.value` 와 같다)
fn query_first<'a>(items: &'a [(String, Option<String>)], name: &str) -> Option<&'a str> {
    items.iter().find(|(n, _)| n == name).and_then(|(_, v)| v.as_deref())
}

const NOT_URL: &str = "웹 주소 꼴이 아닙니다(https:// 로 시작하는 주소를 붙여 넣습니다)";
const WITH_PASSWORD: &str = "사용자 이름·비밀번호가 든 주소는 받지 않습니다";

/// `^[0-9]{1,20}$` — Confluence 페이지 번호
fn page_num_ok(s: &str) -> bool {
    (1..=20).contains(&s.chars().count()) && s.chars().all(|c| c.is_ascii_digit())
}

/// `^[A-Za-z0-9]{10,64}$` — Figma 파일 키
fn figma_key_ok(s: &str) -> bool {
    (10..=64).contains(&s.chars().count()) && s.chars().all(|c| c.is_ascii_alphanumeric())
}

/// `^[0-9]{1,9}:[0-9]{1,9}$` — node-id 를 API 꼴(12:4)로 바꾼 것
fn node_api_ok(s: &str) -> bool {
    match s.split_once(':') {
        Some((a, b)) => {
            (1..=9).contains(&a.chars().count())
                && (1..=9).contains(&b.chars().count())
                && a.chars().all(|c| c.is_ascii_digit())
                && b.chars().all(|c| c.is_ascii_digit())
        }
        None => false,
    }
}

/// `^[A-Za-z0-9_-]{1,64}$` — 골라 읽을 MCP 서버의 이름. 원본: `apMCPServerNameOK`(`MCPRead.swift` — 아직 이식 전이라 이 청크가 비공개로 옮겼다)
fn mcp_server_name_ok(s: &str) -> bool {
    (1..=64).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// 링크 하나를 읽을 대상으로. Atlassian 링크는 연결한 사이트(시험 빌드는 http://127.0.0.1:<포트> 도)와 같은 호스트일 때만 Confluence·Jira 로 본다 —
/// 다른 사이트의 Atlassian 링크는 종류만 알리고(연결 필요) 그 사이트로 요청하지 않는다. 원본: `apReadTarget`
pub fn read_target(raw: &str, atlassian_site: Option<&str>) -> ReadTarget {
    let s = raw.trim_matches(char::is_whitespace);
    let Some(u) = parse_link_url(s) else { return ReadTarget::Bad(NOT_URL.to_string()) };
    let Some(scheme) = u.scheme.as_deref().map(str::to_lowercase) else { return ReadTarget::Bad(NOT_URL.to_string()) };
    if scheme != "https" && scheme != "http" {
        return ReadTarget::Bad(NOT_URL.to_string());
    }
    let Some(host0) = u.host.as_deref() else { return ReadTarget::Bad(NOT_URL.to_string()) };
    let host = host0.to_lowercase();
    if host.is_empty() {
        return ReadTarget::Bad(NOT_URL.to_string());
    }
    if u.user.is_some() || u.password.is_some() {
        return ReadTarget::Bad(WITH_PASSWORD.to_string());
    }
    let path = u.path;
    let atl = host.ends_with(".atlassian.net") || atlassian_site.is_some_and(|b| same_site(s, b));
    let q = query_items(u.query.as_deref().unwrap_or(""));
    if atl {
        let p: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();
        if let Some(i) = p.iter().position(|x| *x == "browse") {
            if i + 1 < p.len() && issue_key_ok(p[i + 1]) {
                return ReadTarget::Jira { key: p[i + 1].to_string() };
            }
        }
        if let Some(k) = query_first(&q, "selectedIssue").filter(|k| issue_key_ok(k)) {
            return ReadTarget::Jira { key: k.to_string() };
        }
        if path.starts_with("/wiki") {
            if let Some(i) = p.iter().position(|x| *x == "pages") {
                if i + 1 < p.len() && page_num_ok(p[i + 1]) {
                    return ReadTarget::Confluence { page: p[i + 1].to_string() };
                }
            }
            if let Some(id) = query_first(&q, "pageId").filter(|id| page_num_ok(id)) {
                return ReadTarget::Confluence { page: id.to_string() };
            }
            return ReadTarget::Bad("Confluence 페이지 주소에서 페이지 번호를 찾지 못했습니다(…/pages/<번호>/… 꼴로 붙여 넣습니다. 짧은 주소 /wiki/x/… 는 아직 읽지 않습니다)".to_string());
        }
        return ReadTarget::Bad("Jira 이슈(/browse/<키>)나 Confluence 페이지(/wiki/…/pages/<번호>) 주소가 아닙니다".to_string());
    }
    if host == "notion.so" || host.ends_with(".notion.so") || host.ends_with(".notion.site") {
        if let Some(id) = query_first(&q, "p").and_then(notion_id) {
            return ReadTarget::Notion { page: id };
        }
        // 끝의 슬래시는 칸만 더하니 자르고 마지막 칸 — 원본 path.split(/).filter { !$0.isEmpty }.last 와 같다
        let last = path.trim_end_matches('/').split('/').next_back().unwrap_or("");
        let tail = last.rsplit('-').next().unwrap_or("");
        if let Some(id) = notion_id(tail) {
            return ReadTarget::Notion { page: id };
        }
        return ReadTarget::Bad("Notion 페이지 주소 끝의 32자리 id 를 찾지 못했습니다".to_string());
    }
    if host == "figma.com" || host.ends_with(".figma.com") {
        let p: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();
        if !(p.len() >= 2 && matches!(p[0], "design" | "file" | "proto" | "board")) {
            return ReadTarget::Bad("Figma 파일 주소(/design/<키>/…)가 아닙니다".to_string());
        }
        let mut key = p[1];
        if p.len() >= 4 && p[2] == "branch" {
            key = p[3];
        }
        if !figma_key_ok(key) {
            return ReadTarget::Bad("Figma 파일 키 꼴이 아닙니다".to_string());
        }
        let mut node = None;
        if let Some(n) = query_first(&q, "node-id") {
            let api = n.replace('-', ":");
            if node_api_ok(&api) {
                node = Some(api);
            }
        }
        return ReadTarget::Figma { file: key.to_string(), node };
    }
    ReadTarget::Web
}

/// 읽을 수 있는 연결인가 — 연결이 있고 범위에 "읽기"가 있고 토큰이 키체인에 있고 만료되지 않고 바탕 주소가 규칙 안일 때 None, 아니면 까닭(요청 0). 원본: `apReadBlocker`
pub fn read_blocker(service: Service, connection: Option<&Connection>, has_secret: bool, dev: bool, now: f64, tz: Tz) -> Option<String> {
    let Some(c) = connection else { return Some(format!("{} 연결이 없습니다(설정 › 연결)", service.label())) };
    if !c.scopes.iter().any(|s| s == "read") {
        return Some(format!("{} 연결의 범위에 '읽기'가 없습니다(설정 › 연결)", service.label()));
    }
    if !has_secret {
        return Some(format!("{} 토큰이 키체인에 없습니다(설정 › 연결)", service.label()));
    }
    if matches!(ap_expiry(&c.expires, now, tz), Expiry::Expired(_)) {
        return Some(format!("{} 토큰 만료일({})이 지났습니다", service.label(), c.expires));
    }
    if service_base(c, dev).is_none() {
        return Some(format!("{} 사이트 주소가 규칙에 맞지 않습니다", service.label()));
    }
    if let Auth::Basic { user } = auth_for(c) {
        if user.is_empty() {
            return Some("Atlassian 계정 메일이 없습니다".to_string());
        }
    }
    None
}

/// 한 줄의 상태를 매기는 데 필요한 것(`apLinkState` 의 매개변수 모음 — 흐름 함수의 매개변수가 늘어나지 않게)
struct LineCtx<'a> {
    connections: &'a [Connection],
    secrets: &'a HashSet<String>,
    dev: bool,
    now: f64,
    tz: Tz,
}

/// 읽을 대상(Jira·Confluence·Notion·Figma) 한 줄의 상태 — 종류와 연결로 매긴다. 원본 `apLinkState` 의 default 면
fn linked(s: &str, label: &str, svc: Service, ctx: &LineCtx) -> ApLink {
    let c = ctx.connections.iter().find(|c| c.service == svc);
    if svc == Service::Atlassian {
        if let Some(c) = c {
            if let Some(base) = service_base(c, ctx.dev) {
                if !same_site(s, &base) {
                    return ApLink::new(s, label, "needs_connection", &format!("연결한 Atlassian 사이트({})와 다른 사이트의 주소입니다", url_host(&base)));
                }
            }
        }
    }
    let has_secret = c.is_some_and(|c| ctx.secrets.contains(&c.id));
    if let Some(why) = read_blocker(svc, c, has_secret, ctx.dev, ctx.now, ctx.tz) {
        return ApLink::new(s, label, "needs_connection", &why);
    }
    ApLink::new(s, label, "connected", &format!("{label} 연결로 메타 정보만 읽습니다"))
}

/// 보드의 자료 링크 줄 — 종류와 읽기 상태(connected·needs_connection·unreadable·mcp)와 까닭. 연결 목록·키체인 있음 여부·고른 MCP 서버로 매긴다. 원본: `apLinkState`
pub fn link_state(
    raw: &str, connections: &[Connection], secrets: &HashSet<String>, mcp: Option<&str>, dev: bool, now: f64, tz: Tz,
) -> ApLink {
    let ctx = LineCtx { connections, secrets, dev, now, tz };
    let s = raw.trim_matches(char::is_whitespace);
    let site = ctx.connections.iter().find(|c| c.service == Service::Atlassian).and_then(|c| service_base(c, dev));
    let t = read_target(s, site.as_deref());
    match &t {
        ReadTarget::Bad(why) => ApLink::new(s, &link_info(s).kind, "unreadable", why),
        ReadTarget::Web => {
            if let Some(m) = mcp.filter(|m| mcp_server_name_ok(m)) {
                ApLink::new(s, "웹", "mcp", &format!("MCP 서버 {} 의 읽기 도구로 읽습니다(그 서버 하나만 켠 짧은 세션)", m))
            } else {
                ApLink::new(s, "웹", "needs_connection", "내장하지 않은 주소라 MCP 서버를 골라 읽습니다(읽기 도구만 켠 짧은 세션)")
            }
        }
        ReadTarget::Jira { .. } | ReadTarget::Confluence { .. } => linked(s, t.label(), Service::Atlassian, &ctx),
        ReadTarget::Notion { .. } => linked(s, t.label(), Service::Notion, &ctx),
        ReadTarget::Figma { .. } => linked(s, t.label(), Service::Figma, &ctx),
    }
}

// ── 읽기 요청 꼴(GET 만) ─────────────────────────────

/// 원본 `readHeaders`(`[Accept: application/json]`) — GET 이라 본문은 없다
fn read_headers() -> BTreeMap<String, String> {
    [("Accept", "application/json")].into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

/// 원본 `[Accept: …, Notion-Version: apNotionVersion]`
fn notion_headers() -> BTreeMap<String, String> {
    let mut h = read_headers();
    h.insert("Notion-Version".to_string(), NOTION_VERSION.to_string());
    h
}

/// 주소 한 칸(경로 조각·물음 값) — UTF-8 바이트마다 unreserved(A-Za-z0-9-._~) 밖은 %XX(대문자). 원본 `q`(`addingPercentEncoding`)
fn q(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if is_unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// 원본 `addingPercentEncoding(withAllowedCharacters: .urlQueryAllowed)` — unreserved 에 `!$&'()*+,-./:;=?@` 도 그대로
fn query_allowed(s: &str) -> String {
    let allowed = |b: u8| {
        is_unreserved(b)
            || matches!(b, b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b'-' | b'.' | b'/' | b':' | b';' | b'=' | b'?' | b'@')
    };
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        if allowed(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Jira 읽기 때 요청할 칸. 원본: `apJiraReadFields`
pub const JIRA_READ_FIELDS: &str =
    "summary,status,issuetype,assignee,reporter,priority,labels,updated,created,description,attachment,issuelinks,parent,fixVersions,components";

/// 원본: `apJiraIssueRequest`(`fields` 는 Swift 기본 인자 `apJiraReadFields` — 부르는 쪽이 `JIRA_READ_FIELDS` 를 넣는다)
pub fn jira_issue_request(key: &str, base: &str, auth: Auth, fields: &str) -> HttpRequest {
    HttpRequest {
        url: format!("{base}/rest/api/3/issue/{}?fields={}", q(key), query_allowed(fields)),
        headers: read_headers(),
        auth,
        ..HttpRequest::default()
    }
}

/// 원본: `apConfluenceReadRequest`
pub fn confluence_read_request(page: &str, base: &str, auth: Auth) -> HttpRequest {
    HttpRequest {
        url: format!(
            "{base}/wiki/rest/api/content/{}?expand=body.storage,version,space,history,children.attachment,metadata.labels",
            q(page)
        ),
        headers: read_headers(),
        auth,
        ..HttpRequest::default()
    }
}

/// 원본: `apNotionPageGetRequest`
pub fn notion_page_get_request(page: &str, base: &str) -> HttpRequest {
    HttpRequest { url: format!("{base}/v1/pages/{}", q(page)), headers: notion_headers(), auth: Auth::Bearer, ..HttpRequest::default() }
}

/// 원본: `apNotionChildrenRequest` — 이어지는 페이지는 `&start_cursor=<커서>`
pub fn notion_children_request(block: &str, base: &str, cursor: Option<&str>) -> HttpRequest {
    let mut url = format!("{base}/v1/blocks/{}/children?page_size=100", q(block));
    if let Some(c) = cursor {
        url.push_str(&format!("&start_cursor={}", q(c)));
    }
    HttpRequest { url, headers: notion_headers(), auth: Auth::Bearer, ..HttpRequest::default() }
}

/// 원본: `apFigmaReadRequest` — node 가 있으면 골라 읽기(`/nodes?ids=`)
pub fn figma_read_request(file: &str, node: Option<&str>, base: &str) -> HttpRequest {
    let url = match node {
        Some(n) => format!("{base}/v1/files/{}/nodes?ids={}", q(file), q(n)),
        None => format!("{base}/v1/files/{}?depth=2", q(file)),
    };
    HttpRequest { url, headers: read_headers(), auth: Auth::Figma, ..HttpRequest::default() }
}

/// 원본: `apFigmaImageRequest`
pub fn figma_image_request(file: &str, node: &str, base: &str) -> HttpRequest {
    HttpRequest {
        url: format!("{base}/v1/images/{}?ids={}&format=png&scale=1", q(file), q(node)),
        headers: read_headers(),
        auth: Auth::Figma,
        ..HttpRequest::default()
    }
}

// ── Figma 그림 주소 허용 ─────────────────────────────

/// /v1/images 가 돌려주는 내보낸 그림 자리의 정한 호스트. 원본: `apFigmaImageHosts`
pub const FIGMA_IMAGE_HOSTS: [&str; 1] = ["figma-alpha-api.s3.us-west-2.amazonaws.com"];

/// 원본: `apFigmaImageHostSuffix`
pub const FIGMA_IMAGE_HOST_SUFFIX: &str = ".figma.com";

/// Figma 그림 주소 허용 — 내보낸 그림 자리(https 의 정한 호스트: FIGMA_IMAGE_HOSTS 와 *.figma.com 꼬리)만. 시험 빌드는 가짜 서버
/// http://127.0.0.1(포트 아무것)도. 사용자 정보·다른 포트(https 는 443)·다른 호스트는 받지 않는다. 이 요청에는 인증 머리를 붙이지 않는다. 원본: `apImageURLOK`
pub fn image_url_ok(url: &str, dev: bool) -> bool {
    if grapheme_len(url) > 4000 {
        return false;
    }
    let quoted = "\"'<>\\`{}|^";
    if !url.chars().all(|c| {
        let v = c as u32;
        (0x20 < v) && (v < 0x7F) && !quoted.contains(c)
    }) {
        return false;
    }
    let Some(c) = parse_link_url(url) else { return false };
    let Some(scheme) = c.scheme.as_deref().map(str::to_lowercase) else { return false };
    let Some(host) = c.host.as_deref().map(str::to_lowercase) else { return false };
    if host.is_empty() || c.user.is_some() || c.password.is_some() {
        return false;
    }
    if scheme == "https" {
        if !(c.port.is_none() || c.port == Some(443)) {
            return false;
        }
        return FIGMA_IMAGE_HOSTS.contains(&host.as_str())
            || (host.ends_with(FIGMA_IMAGE_HOST_SUFFIX) && host.len() > FIGMA_IMAGE_HOST_SUFFIX.len());
    }
    dev && scheme == "http" && host == "127.0.0.1"
}

/// 받은 그림의 크기 상한. 원본: `apImageMaxBytes`
pub const IMAGE_MAX_BYTES: usize = 8 << 20;

/// 그림 첫 8바이트의 PNG 서명(89 50 4E 47 0D 0A 1A 0A). 원본: `apIsPNG`(`d.count > 8` — 8바이트만은 아니다)
pub fn is_png(d: &[u8]) -> bool {
    d.len() > 8 && d.get(0..8) == Some(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A][..])
}

// R-Reading-b 가 여기서 이어 쓴다
