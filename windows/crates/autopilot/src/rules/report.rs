//! (청크 Report 2/4 — 원본 1~251줄만 이식됨. 이 첫 줄은 `tests/ported.rs` 가 「구현됐는데 PORTED.json 에 없는 규칙」으로 잡지 않게 하는 표시다. R-Report-c·d 가 나머지(Writer 부터)를 옮긴다.
//! 실행 보고서 틀 앞부분. 원본: `Sources/AutopilotRules/Report.swift`(568줄). 이 구간은 결과 기록(`result.json`, 모델이 쓴 글)을 값으로 읽는 일과
//! 보고서가 쓰는 값 꼴이다. 모델이 쓴 글은 믿지 않는다: 칸의 꼴이 틀리면 그 칸만 비우고, 글·목록 길이는 상한에서 자른다(`rules::formats`·`rules::prose` 와 같은 태도).
//! 긴 줄표 바꾸기(`rules::prose::plain`)와 HTML 이스케이프(`rules::prose::escape_html`)는 그릴 때(뒤 구간) 한다. 여기서는 글자를 그대로 둔다.
//!
//! 이식됨(원본 순서): `ResultTest`(APResultTest) · `ResultChange`(APResultChange) · `ResultDecision`(APResultDecision) · `ResultFlow`(APResultFlow) ·
//! `ResultCarry`(APResultCarry) · `ResultTicket`(APResultTicket) · `ResultRef`(APResultRef) · `ResultRecord`(APResultRecord) ·
//! `REPORT_TEXT_MAX`·`REPORT_LIST_MAX`·`REPORT_FLOW_MAX`·`REPORT_TAIL_MAX`(apReportTextMax·ListMax·FlowMax·TailMax) · `cut`(apCut) ·
//! `report_text_n`·`report_text`(apReportText) · `parse_result`(apParseResult) · `ResultState`(APResultState) · `ReportCheck`(APReportCheck) ·
//! `ReportUnit`(APReportUnit) · `ReportRun`(APReportRun) · `ReportOrigin`(APReportOrigin, 원본은 `Ports.swift` 에 있다).
//! 이식됨(2/4, 원본 143~251줄): `ReportSource`(APReportSource, 원본은 `Ports.swift`) · `num`·`int`·`str`(`str_of`)·`lastPart`(`last_part`) · `report_run`(apReportRun, 기록 → 보고서 값) · 이름표 사전 일곱(`unit_label`·`unit_class`·`run_label`·`kind_label`·`outcome_label`·`test_label`·`merge_label`) · `report_dur`(apReportDur) · `kind_text`(비공개 `kind`).
//! 남음(원본 253~568줄, 뒤 구간이 이 파일 끝에 이어 붙인다): `Writer`(시간대 글 만들기 포함) · `chip` · 그리는 쪽 전부(`apReportHTML`·`apDrawReports` 등). 쓸 곳 포트(`APReportSink`·`APReportFile`)는 그릴 때 정한다.
//! 원본 시험 중 이 구간의 것: `ReportTests.swift` 의 `ResultParseTests` 셋(`아님`·`꼴`·`상한`)과 `ReportRunTests` 셋(`견본`·`칸`·`없음`) → `tests/rules_report.rs`.
//!
//! Swift ↔ Rust 로 달라진 점:
//! - 기본 인자(`apReportText(v, n = 4000)`)가 없다: `report_text`(상한 4000)와 `report_text_n`(상한을 준다)으로 나눈다.
//! - 사전 `[String: Any]` 는 `serde_json::Value`·`Map`(키 차례 보존)이다. Swift `NSNumber`(불리언 아님)는 `Value::Number`, 불리언은 `Value::Bool` 이라 글이 되지 않는다.
//! - 수를 글로 바꾸는 `NSNumber.stringValue` 는 정수는 그대로, 실수는 `7.0` → `7`, `1.5` → `1.5` 이다. 아주 크거나(지수 17 이상) 아주 작은(-5 아래) 실수는
//!   `e+XX`·`e-XX` 꼴로 쓰는데 Foundation 이 그 경계에서 쓰는 자릿수와 같은지 이 머신(Windows)에서는 확인하지 못했다. 결과 기록 칸에 그런 수가 오는 일은 모델이 틀리게 쓴 때뿐이다.
//! - `String.count`(Character 수)는 `formats::grapheme_len` 과 같은 근사다(CR LF 는 한 글자). `cut` 의 앞부분 자르기도 같은 덩어리 경계에서 한다.
//! - `JSONSerialization` 대신 `serde_json`: 깊이 한도(128)가 달라 아주 깊은 중첩은 nil 이 된다. 같은 키가 둘이면 둘 다 마지막 것을 쓴다.
//! - `refs` 키 정렬은 Swift `String` 비교(유니코드 정규 동등)이고 여기는 UTF-8 바이트 순이다. 키가 정규화되지 않은 글자를 섞어 쓰면 차례가 다를 수 있다.
//! - `last_part` 는 `/` 와 `\` 를 모두 구분자로 본다(Swift `lastPathComponent` 는 `/` 만). 엔진이 Windows 경로(`C:\work\todo-api`)를 기록해도 저장소 이름이 `todo-api` 가 되게 하려는 것이고, `/` 만 쓰는 입력은 같다.
//! - `as? Bool`(`same`·`timed_out`·`interrupted`)은 JSON 불리언만 받는다. Foundation 은 수 0·1 도 받을 수 있는데 이 머신에서 확인하지 못했다(엔진이 쓴 기록은 불리언이다).
//! - `as? [[String: Any]]` 는 원소 하나라도 객체가 아니면 통째로 빈 배열이 된다(`moved`·`changed`·`checks`·`denials`). 같게 옮겼다.
//! - `apReportDur` 의 `Int(sec)` 는 범위 밖에서 멈추고 Rust `as i64` 는 끝값에 붙는다.
//! - 사전 일곱은 Swift 에서 파일 안 비공개 상수이고 Rust 에서는 `Option` 을 돌려주는 함수다(뒤 구간이 쓰므로 공개).
//!
//! Windows 판단: 글 칸에 CRLF 가 들어와도(`\r\n` 은 한 글자) 자르는 자리·글자 수는 Swift 와 같다. 경로 가운데 `ResultChange::place` 는 받은 글자 그대로 두고,
//! 저장소 이름만 `last_part` 가 `/` 와 `\` 로 끊는다(`C:\work\todo-api` → `todo-api`). 이 변환은 홈 폴더 줄이기(`~/`)와 달리 그리는 쪽이 아니라 값을 만들 때 한다.

use crate::ports::Row;
use crate::rules::formats::grapheme_len;
use serde_json::{Map, Number, Value};

// ── 결과 기록 (result.json, 설계 5장 3절) ─────────────

/// 시험 한 줄. 원본: `APResultTest`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultTest {
    pub name: String,
    pub result: String,
    pub note: String,
}

/// 바뀐 것 한 줄. 원본: `APResultChange`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultChange {
    pub what: String,
    pub place: String,
}

/// 적용한 결정 한 줄. 원본: `APResultDecision`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultDecision {
    pub what: String,
    pub basis: String,
}

/// 흐름 그림(mermaid 원문). 원본: `APResultFlow`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultFlow {
    pub title: String,
    pub source: String,
}

/// 다음에 이어 할 일·새로 생긴 일 한 줄. 원본: `APResultCarry`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultCarry {
    pub title: String,
    pub why: String,
}

/// 실행 중에 새로 필요해진 티켓(4단계 ⑩-3) — 모델이 임시 번호(`TMP-<단위>-<차례>`)로 쓰고 결과 기록 `new_tickets` 에 남긴 것. 보드가 검수 뒤에 만든다. 원본: `APResultTicket`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultTicket {
    pub temp: String,
    pub title: String,
    pub why: String,
}

/// 참고 한 줄(키 차례). 원본: `APResultRef`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultRef {
    pub key: String,
    pub value: String,
}

/// 결과 기록 한 건. 원본: `APResultRecord`
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResultRecord {
    // unit 칸
    pub title: String,
    pub kind: String,
    pub r#ref: String,
    pub outcome: String,
    pub headline: String,
    pub next: String,
    pub summary: Vec<String>,
    pub risks: Vec<String>,
    pub tests: Vec<ResultTest>,
    pub changes: Vec<ResultChange>,
    pub decisions: Vec<ResultDecision>,
    pub flows: Vec<ResultFlow>,
    pub next_start: Vec<ResultCarry>,
    pub new_work: Vec<ResultCarry>,
    pub new_tickets: Vec<ResultTicket>,
    /// 키 차례
    pub refs: Vec<ResultRef>,
}

/// 글 한 칸의 글자 수 상한(넘으면 자르고 …). 원본: `apReportTextMax`
pub const REPORT_TEXT_MAX: usize = 4000;
/// 목록 한 칸의 줄 수 상한. 원본: `apReportListMax`
pub const REPORT_LIST_MAX: usize = 200;
/// 흐름 그림 원문 글자 수 상한. 원본: `apReportFlowMax`
pub const REPORT_FLOW_MAX: usize = 20_000;
/// 판정 출력 끝부분 글자 수 상한. 원본: `apReportTailMax`
pub const REPORT_TAIL_MAX: usize = 2000;

// 글자(Character) 수가 `n` 이하인 가장 긴 앞 조각의 끝 바이트 위치(글자 수가 `n` 이하면 글 전체). 앞 조각의 글자 수는 길어질수록 줄지 않는다:
// 그런 가장 긴 앞 조각이 앞 `n` 글자다(다음 글자가 새 덩어리를 열면 n+1 이 된다)
fn prefix_end(s: &str, n: usize) -> usize {
    // 글자 하나는 바이트 하나 이상이다
    if s.len() <= n || grapheme_len(s) <= n {
        return s.len();
    }
    let bounds: Vec<usize> = s.char_indices().map(|(i, _)| i).chain(std::iter::once(s.len())).collect();
    let (mut lo, mut hi) = (0, bounds.len() - 1);
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if grapheme_len(&s[..bounds[mid]]) <= n {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    bounds[lo]
}

/// 글자(Character) 수가 `n` 을 넘으면 앞 `n` 글자에서 자르고 `…` 를 붙인다. 원본: `apCut`
pub fn cut(s: &str, n: usize) -> String {
    let end = prefix_end(s, n);
    if end == s.len() {
        s.to_string()
    } else {
        format!("{}…", &s[..end])
    }
}

// 수 → 글. 정수는 그대로, 실수는 짧은 왕복 꼴(정수 값이면 소수점 없이). 지수가 -5 아래거나 17 이상이면 `1e+20` 꼴
fn number_text(x: &Number) -> String {
    if let Some(i) = x.as_i64() {
        return i.to_string();
    }
    if let Some(u) = x.as_u64() {
        return u.to_string();
    }
    let f = x.as_f64().unwrap_or(0.0);
    let e = format!("{f:e}");
    if let Some((mant, exp)) = e.split_once('e') {
        if let Ok(exp) = exp.parse::<i32>() {
            if !(-5..17).contains(&exp) {
                return format!("{mant}e{}{:02}", if exp < 0 { '-' } else { '+' }, exp.abs());
            }
        }
    }
    format!("{f}")
}

/// 글 칸 하나(상한 4000) — 글자는 그대로, 숫자는 글로, 그 밖(배열·객체·null·불리언)은 빈 글자. 원본: `apReportText`
pub fn report_text(v: Option<&Value>) -> String {
    report_text_n(v, REPORT_TEXT_MAX)
}

/// 글 칸 하나 — 상한 `n` 을 준다. 원본: `apReportText`
pub fn report_text_n(v: Option<&Value>, n: usize) -> String {
    match v {
        Some(Value::String(s)) => cut(s, n),
        Some(Value::Number(x)) => number_text(x),
        _ => String::new(),
    }
}

// 목록 칸: 배열의 앞 200 개 가운데 객체인 것만(상한을 먼저 자르고 객체를 고른다)
fn list<'a>(j: &'a Map<String, Value>, k: &str) -> Vec<&'a Map<String, Value>> {
    j.get(k).and_then(Value::as_array).map(|a| a.iter().take(REPORT_LIST_MAX).filter_map(Value::as_object).collect()).unwrap_or_default()
}

// 글 목록 칸: 배열의 앞 200 개를 글로 바꾸고 빈 것은 뺀다
fn text_list(j: &Map<String, Value>, k: &str) -> Vec<String> {
    j.get(k)
        .and_then(Value::as_array)
        .map(|a| a.iter().take(REPORT_LIST_MAX).map(|v| report_text(Some(v))).filter(|s| !s.is_empty()).collect())
        .unwrap_or_default()
}

/// 결과 기록 글 → 값. JSON 객체가 아니면 `None`. 칸의 꼴이 틀리면(글 자리에 객체 등) 그 칸만 비운다. 긴 줄표는 그릴 때(`prose::plain`) 바꾼다. 원본: `apParseResult`
pub fn parse_result(text: Option<&str>) -> Option<ResultRecord> {
    let v: Value = serde_json::from_str(text?).ok()?;
    let j = v.as_object()?;
    let empty = Map::new();
    let u = j.get("unit").and_then(Value::as_object).unwrap_or(&empty);
    let mut r = ResultRecord {
        title: report_text_n(u.get("title"), 300),
        kind: report_text_n(u.get("kind"), 40),
        r#ref: report_text_n(u.get("ref"), 300),
        outcome: report_text_n(j.get("outcome"), 40),
        headline: report_text(j.get("headline")),
        next: report_text(j.get("next")),
        summary: text_list(j, "summary"),
        risks: text_list(j, "risks"),
        ..ResultRecord::default()
    };
    r.tests = list(j, "tests")
        .iter()
        .map(|m| ResultTest { name: report_text_n(m.get("name"), 500), result: report_text_n(m.get("result"), 40), note: report_text(m.get("note")) })
        .collect();
    r.changes = list(j, "changes").iter().map(|m| ResultChange { what: report_text(m.get("what")), place: report_text_n(m.get("where"), 500) }).collect();
    r.decisions = list(j, "decisions_applied").iter().map(|m| ResultDecision { what: report_text(m.get("what")), basis: report_text(m.get("basis")) }).collect();
    r.flows = list(j, "sequence")
        .iter()
        .map(|m| ResultFlow { title: report_text_n(m.get("title"), 300), source: report_text_n(m.get("mermaid"), REPORT_FLOW_MAX) })
        .collect();
    r.next_start = list(j, "next_start").iter().map(|m| ResultCarry { title: report_text(m.get("title")), why: report_text(m.get("why")) }).collect();
    r.new_work = list(j, "new_work").iter().map(|m| ResultCarry { title: report_text(m.get("title")), why: report_text(m.get("why")) }).collect();
    r.new_tickets = list(j, "new_tickets")
        .iter()
        .take(20)
        .map(|m| ResultTicket { temp: report_text_n(m.get("temp"), 20), title: report_text_n(m.get("title"), 300), why: report_text_n(m.get("why"), 1000) })
        .collect();
    if let Some(refs) = j.get("refs").and_then(Value::as_object) {
        let mut keys: Vec<&String> = refs.keys().collect();
        keys.sort();
        r.refs = keys
            .into_iter()
            .take(50)
            .filter_map(|k| {
                let v = report_text_n(refs.get(k), 1000);
                (!v.is_empty()).then(|| ResultRef { key: cut(k, 60), value: v })
            })
            .collect();
    }
    Some(r)
}

// ── 보고서 값 ────────────────────────────────────────

/// 결과 기록의 상태. 원본: `APResultState`(rawValue `ok`·`missing`·`broken`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResultState {
    Ok,
    #[default]
    Missing,
    Broken,
}

impl ResultState {
    /// 원본 `rawValue`
    pub fn raw(self) -> &'static str {
        match self {
            ResultState::Ok => "ok",
            ResultState::Missing => "missing",
            ResultState::Broken => "broken",
        }
    }
}

/// 판정 한 번. 원본: `APReportCheck`
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReportCheck {
    pub n: i64,
    pub exit: Option<i64>,
    pub sec: f64,
    pub tail: String,
    pub timed_out: bool,
    pub interrupted: bool,
}

/// 단위 하나. `status` 의 처음 값은 `pending`, `result_state` 는 `Missing`. 원본: `APReportUnit`
#[derive(Debug, Clone, PartialEq)]
pub struct ReportUnit {
    pub n: i64,
    pub title: String,
    pub kind: String,
    pub status: String,
    pub reason: String,
    pub task: String,
    pub story: String,
    pub branch: String,
    pub merge: String,
    pub active_sec: f64,
    pub cost: f64,
    pub turns: i64,
    pub retries: i64,
    pub started: Option<f64>,
    pub ended: Option<f64>,
    pub checks: Vec<ReportCheck>,
    pub last_exit: Option<i64>,
    pub denials: Vec<String>,
    pub result: Option<ResultRecord>,
    pub result_state: ResultState,
}

impl Default for ReportUnit {
    fn default() -> Self {
        ReportUnit {
            n: 0,
            title: String::new(),
            kind: String::new(),
            status: "pending".to_string(),
            reason: String::new(),
            task: String::new(),
            story: String::new(),
            branch: String::new(),
            merge: String::new(),
            active_sec: 0.0,
            cost: 0.0,
            turns: 0,
            retries: 0,
            started: None,
            ended: None,
            checks: Vec::new(),
            last_exit: None,
            denials: Vec::new(),
            result: None,
            result_state: ResultState::Missing,
        }
    }
}

/// 기록의 바탕: 실행 폴더 · 기록 저장소. 원본: `APReportOrigin`(Ports.swift, rawValue `folder`·`records`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReportOrigin {
    #[default]
    Folder,
    Records,
}

/// 실행 하나의 보고서 값. 원본: `APReportRun`
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReportRun {
    pub id: String,
    pub title: String,
    pub project: String,
    pub model: String,
    pub state: String,
    pub reason: String,
    pub started: Option<f64>,
    pub ended: Option<f64>,
    /// worktree · in_place · 빈 글자(기록 없음)
    pub mode: String,
    /// 자율주행 가지
    pub work: String,
    /// 저장소 이름(마지막 경로 조각)
    pub repos: Vec<String>,
    /// "저장소: 출발 가지"
    pub bases: Vec<String>,
    pub harness_same: Option<bool>,
    pub harness_files: i64,
    pub harness_changed: Vec<String>,
    /// 움직인 기존 가지(0 이어야 함)
    pub moved: Vec<String>,
    pub stashed: i64,
    pub epic: String,
    pub units: Vec<ReportUnit>,
    pub origin: ReportOrigin,
}

// 쓸 곳 포트(APReportSink)와 APReportFile 은 원본 Ports.swift 에 있다(뒤 구간이 그리는 쪽을 옮길 때 같이 정한다)

// ── 기록 → 보고서 값 ──────────────────────────────────

/// 보고서가 읽을 곳(원본 `APReportSource`, `Ports.swift`). 실행 폴더든 기록 저장소든 사전(JSON 객체)으로 돌려준다.
/// 포트 모음(`ports.rs`)에 아직 없어 이 파일에 둔다(옮기면 `use` 만 바뀐다).
pub trait ReportSource {
    fn origin(&self) -> ReportOrigin;
    fn run(&self) -> Option<Row>;
    fn queue(&self) -> Vec<Row>;
    fn engine(&self, n: i64) -> Option<Row>;
    /// 모델이 쓴 결과 기록 글(보통 파일이고 크기 상한 아래일 때만, 어댑터가 본다). 없으면 `None`
    fn result(&self, n: i64) -> Option<String>;
}

// `v as? NSNumber`(불리언 제외) → Double
fn num(v: Option<&Value>) -> Option<f64> {
    match v {
        Some(Value::Number(x)) => x.as_f64(),
        _ => None,
    }
}

// 유한하고 절댓값이 1e15 아래인 수만 정수로(소수는 0 쪽으로 버림)
fn int(v: Option<&Value>) -> Option<i64> {
    num(v).filter(|x| x.is_finite() && x.abs() < 1e15).map(|x| x as i64)
}

// `v as? String ?? ""`
fn str_of(v: Option<&Value>) -> String {
    match v {
        Some(Value::String(s)) => s.clone(),
        _ => String::new(),
    }
}

// `v as? String`
fn as_string(v: Option<&Value>) -> Option<&str> {
    match v {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

// `v as? [String: Any]`
fn object(v: Option<&Value>) -> Option<&Map<String, Value>> {
    v.and_then(Value::as_object)
}

// `v as? [[String: Any]] ?? []` — Swift 의 배열 형 바꾸기는 모두 객체여야 하고, 하나라도 아니면 통째로 실패해 빈 배열이 된다
fn objects(v: Option<&Value>) -> Vec<&Map<String, Value>> {
    let Some(Value::Array(a)) = v else { return Vec::new() };
    a.iter().map(Value::as_object).collect::<Option<Vec<_>>>().unwrap_or_default()
}

/// `NSString.lastPathComponent`: 끝의 구분자를 떼고 마지막 구분자 뒤. 구분자만 있으면 그 첫 글자(`/`). 빈 글은 빈 글. 원본: `lastPart`(비공개)
/// Windows 판은 `\` 도 구분자로 본다(저장소 경로가 `C:\work\todo-api` 꼴로 와도 `todo-api` 가 되게).
pub fn last_part(p: &str) -> String {
    let t = p.trim_end_matches(['/', '\\']);
    if t.is_empty() {
        return p.chars().next().map(String::from).unwrap_or_default();
    }
    t.rsplit(['/', '\\']).next().unwrap_or("").to_string()
}

// 글자 수가 `n` 을 넘을 때 끝 `n` 글자(`String.suffix(n)`)
fn suffix_chars(s: &str, n: usize) -> &str {
    let total = grapheme_len(s);
    if total <= n {
        return s;
    }
    &s[prefix_end(s, total - n)..]
}

/// 기록을 읽어 보고서 값으로. run 이 없으면 `None`. 단위는 queue 의 차례(1부터)이고, 상태는 queue 의 status(엔진 판정)가 먼저다. 원본: `apReportRun`
pub fn report_run(id: &str, source: &dyn ReportSource) -> Option<ReportRun> {
    let run = source.run()?;
    let mut r = ReportRun { origin: source.origin(), id: id.to_string(), ..ReportRun::default() };
    r.title = report_text_n(run.get("title"), 300);
    r.project = report_text_n(run.get("project"), 120);
    r.model = report_text_n(run.get("model"), 80);
    r.state = report_text_n(run.get("state"), 40);
    r.reason = report_text_n(run.get("reason"), 1000);
    r.started = num(run.get("started"));
    r.ended = num(run.get("ended"));
    r.epic = report_text_n(run.get("epic"), 80);
    let empty = Map::new();
    let wt = object(run.get("worktree")).unwrap_or(&empty);
    r.mode = str_of(wt.get("mode"));
    r.work = str_of(wt.get("branch"));
    let br = object(run.get("branches")).unwrap_or(&empty);
    let repos = object(br.get("repos")).unwrap_or(&empty);
    let mut keys: Vec<&String> = repos.keys().collect();
    keys.sort();
    for k in keys {
        r.repos.push(last_part(k));
        let e = object(repos.get(k)).unwrap_or(&empty);
        if let Some(b) = as_string(object(e.get("base")).and_then(|b| b.get("branch"))) {
            r.bases.push(format!("{}: {b}", last_part(k)));
        }
        if r.work.is_empty() {
            if let Some(w) = as_string(e.get("work")) {
                r.work = w.to_string();
            }
        }
    }
    if r.repos.is_empty() {
        if let Some(rp) = as_string(wt.get("repo")) {
            r.repos = vec![last_part(rp)];
        }
    }
    if r.repos.is_empty() {
        if let Some(Value::Array(a)) = wt.get("repos") {
            r.repos = a.iter().filter_map(|v| as_string(Some(v))).map(last_part).collect();
            r.repos.sort();
        }
    }
    r.moved = objects(br.get("moved")).iter().map(|m| format!("{} {}", last_part(&str_of(m.get("repo"))), str_of(m.get("branch")))).collect();
    r.stashed = br.get("stashed").and_then(Value::as_array).map_or(0, |a| a.len() as i64);
    if let Some(h) = object(run.get("harness")) {
        r.harness_files = int(h.get("files")).unwrap_or(0);
        r.harness_same = h.get("same").and_then(Value::as_bool);
        r.harness_changed = objects(h.get("changed"))
            .iter()
            .take(30)
            .map(|c| {
                let change = str_of(c.get("change"));
                let what = match change.as_str() {
                    "changed" => "바뀜",
                    "added" => "더함",
                    "removed" => "없어짐",
                    other => other,
                };
                format!("{} ({what})", str_of(c.get("path")))
            })
            .collect();
    }
    for (i, q) in source.queue().iter().enumerate() {
        let n = i as i64 + 1;
        let e = source.engine(n).unwrap_or_default();
        let mut u = ReportUnit { n, ..ReportUnit::default() };
        u.title = report_text_n(q.get("title").or_else(|| e.get("title")), 300);
        u.kind = report_text_n(q.get("kind"), 40);
        let (qs, es) = (str_of(q.get("status")), str_of(e.get("status")));
        u.status = if !qs.is_empty() {
            qs
        } else if es.is_empty() {
            "pending".to_string()
        } else {
            es
        };
        u.reason = report_text_n(e.get("reason"), 1000);
        u.story = report_text_n(q.get("story"), 80);
        if !u.story.is_empty() {
            if let Some(t) = q.get("tickets").and_then(Value::as_array).and_then(|a| a.first()) {
                u.task = report_text_n(Some(t), 80);
            }
        }
        u.branch = if str_of(e.get("branch")).is_empty() { str_of(q.get("branch")) } else { str_of(e.get("branch")) };
        u.merge = str_of(object(e.get("merge")).and_then(|m| m.get("state")));
        u.active_sec = num(e.get("active_sec")).unwrap_or(0.0);
        u.cost = num(e.get("cost_usd")).unwrap_or(0.0);
        u.turns = int(e.get("turns")).unwrap_or(0);
        u.retries = int(e.get("try")).unwrap_or(0);
        u.started = num(e.get("started"));
        u.ended = num(e.get("ended"));
        u.checks = objects(e.get("checks"))
            .iter()
            .take(50)
            .enumerate()
            .map(|(k, c)| {
                let tail = report_text_n(c.get("tail"), 100_000);
                ReportCheck {
                    n: int(c.get("try")).map_or(k as i64 + 1, |t| t + 1),
                    exit: int(c.get("exit")),
                    sec: num(c.get("sec")).unwrap_or(0.0),
                    tail: if grapheme_len(&tail) > REPORT_TAIL_MAX { format!("…{}", suffix_chars(&tail, REPORT_TAIL_MAX)) } else { tail },
                    timed_out: c.get("timed_out").and_then(Value::as_bool).unwrap_or(false),
                    interrupted: c.get("interrupted").and_then(Value::as_bool).unwrap_or(false),
                }
            })
            .collect();
        u.last_exit = u.checks.last().and_then(|c| c.exit).or_else(|| int(e.get("check_exit")));
        u.denials = objects(e.get("denials"))
            .iter()
            .take(30)
            .map(|d| [str_of(d.get("tool")), report_text_n(d.get("what"), 300)].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(": "))
            .collect();
        if let Some(t) = source.result(n) {
            match parse_result(Some(&t)) {
                Some(p) => {
                    u.result = Some(p);
                    u.result_state = ResultState::Ok;
                }
                None => u.result_state = ResultState::Broken,
            }
        }
        if u.kind.is_empty() {
            if let Some(k) = u.result.as_ref().map(|p| p.kind.clone()) {
                u.kind = k;
            }
        }
        r.units.push(u);
    }
    Some(r)
}

// ── 보고서 HTML 의 이름표(원본 234~241줄, Swift 에서는 파일 안 비공개 사전) ──────────

/// 단위 상태 이름표. 원본: `unitLabel`
pub fn unit_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "pending" => "대기",
        "running" => "진행 중",
        "done" => "통과",
        "failed" => "미통과",
        "capped" => "상한에서 멈춤",
        "stopped" => "중단",
        "skipped" => "건너뜀",
        _ => return None,
    })
}

/// 단위 상태 칩의 클래스. 원본: `unitClass`
pub fn unit_class(k: &str) -> Option<&'static str> {
    Some(match k {
        "done" => "ok",
        "failed" => "fail",
        "capped" => "cap",
        "running" => "run",
        _ => return None,
    })
}

/// 실행 상태 이름표. 원본: `runLabel`
pub fn run_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "starting" => "출발 중",
        "running" => "도는 중",
        "waiting_limit" => "한도 대기",
        "waiting_stopline" => "멈출 선 대기",
        "waiting_precheck" => "환경 확인 대기",
        "paused" => "일시정지",
        "stopped" => "중단됨",
        "done" => "끝남",
        "error" => "오류로 멈춤",
        _ => return None,
    })
}

/// 종류 이름표(키는 소문자로 접은 글). 원본: `kindLabel`
pub fn kind_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "fix" => "고침",
        "dev" | "feature" | "story" => "개발",
        "test" => "시험",
        "doc" | "docs" => "문서",
        "env" => "환경",
        _ => return None,
    })
}

/// 결과 기록의 끝맺음 이름표. 원본: `outcomeLabel`
pub fn outcome_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "done" => "끝냄",
        "failed" => "끝내지 못함",
        "parked" => "보류",
        "capped" => "상한",
        _ => return None,
    })
}

/// 시험 결과 이름표와 클래스. 원본: `testLabel`
pub fn test_label(k: &str) -> Option<(&'static str, &'static str)> {
    Some(match k {
        "pass" => ("통과", "ok"),
        "fail" => ("실패", "fail"),
        "skip" => ("건너뜀", ""),
        _ => return None,
    })
}

/// 합치기 상태 이름표. 원본: `mergeLabel`
pub fn merge_label(k: &str) -> Option<&'static str> {
    Some(match k {
        "merged" => "스토리 가지에 합침",
        "nothing" => "합칠 커밋 없음",
        "not_merged" => "스토리 가지에 합치지 않음",
        _ => return None,
    })
}

/// 걸린 시간 — 1분 안이면 초, 한 시간 안이면 분, 그 위는 시간·분(보드 `apDur` 과 같은 꼴). 원본: `apReportDur`
pub fn report_dur(sec: f64) -> String {
    // Swift `Int(sec)` 는 범위 밖이면 멈추지만 Rust `as` 는 끝값으로 붙는다(아주 큰 수는 시간 칸이 아주 커질 뿐)
    let s = (if sec.is_finite() { sec } else { 0.0 } as i64).max(0);
    if s == 0 {
        return "0분".to_string();
    }
    if s < 60 {
        return format!("{s}초");
    }
    if s >= 3600 {
        format!("{}시간 {}분", s / 3600, s % 3600 / 60)
    } else {
        format!("{}분", s / 60)
    }
}

/// 종류 글자 → 이름표(없으면 그대로). 원본: `kind`(비공개 함수)
pub fn kind_text(k: &str) -> String {
    kind_label(&k.to_lowercase()).map_or_else(|| k.to_string(), str::to_string)
}
