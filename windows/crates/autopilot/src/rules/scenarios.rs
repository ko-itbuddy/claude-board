//! (청크 Scenarios 1/2 전반만 이식됨 — 이 첫 줄은 `tests/ported.rs` 가 「구현됐는데 PORTED.json 에 없는 규칙」으로 잡지 않게 하는 표시다. R-Scenarios-b 가 후반을 옮긴 뒤 이 줄을 지우고 PORTED.json 항목을 넣는다)
//! 시험 실행기의 순수 규칙: 단위의 판정 시나리오(실행 파일)와 수용 기준 읽기, 돌리는 법과 필요한 도구. 원본: `Sources/AutopilotRules/Scenarios.swift`.
//! 프로세스·파일·샌드박스는 엔진(진짜 어댑터)이, 가짜는 시험이 맡는다. 순수 함수만 있다(파일·시각·환경 없음).
//!
//! 이식된 선언(원본 순서, 원본 1~235 줄 = `// MARK: - 환경 확인` 앞까지): `ScenarioKind`(APScenarioKind) · `BrowserDriver`(APBrowserDriver) ·
//! `Scenario`(APScenario) · `Criterion`(APCriterion) · `SCENARIO_MAX`(apScenarioMax) · `scenario_path_ok`(apScenarioPathOK) ·
//! `criterion_id_ok`(apCriterionIdOK) · `parse_scenarios`(apParseScenarios) · `parse_criteria`(apParseCriteria) ·
//! `scenario_criterion_problems`(apScenarioCriterionProblems) · `criteria_gaps`(apCriteriaGaps) · `scenario_json`(apScenarioJSON) ·
//! `criterion_json`(apCriterionJSON) · `CRITERION_MODEL_JUDGE`·`CRITERION_DROP` · `criterion_card_id`(apCriterionCardId) ·
//! `CardText`·`CardOption`(APCardText) · `criterion_card`(apCriterionCard) · `criterion_answer`(apCriterionAnswer) ·
//! `ToolNeed`(APToolNeed) · `tool_label`(apToolLabel) · `ScenarioRun`(APScenarioRun) · 비공개 `ext_interpreter`(extInterpreters)·
//! `interpreter_needs` · `scenario_run`(apScenarioRun).
//! 남은 선언(R-Scenarios-b, 원본 237 줄 `// MARK: - 환경 확인` 부터 끝까지): 포트 `ToolProbe`(APToolProbe, Ports.swift) · `ToolStatus` · `EnvRow` ·
//! `apEnvCheck` · `apKindLabel` · `apEnvProblem` · `apEnvRowJSON` · `APJudgeStep` · `APPinnedScenario` · `apScenarioLabel` · `apJudgeSteps` ·
//! `APStepResult` · `APJudgeOutcome` · 포트 `JudgeRunner`(APJudgeRunner) · `apRunJudge` · 비공개 `imageExt` · `apJudgeSummary` ·
//! `apPrerunScenarioCard`. 후반이 `ScenarioKind` 의 이름 글(`as_str`)과 `ToolNeed`·`tool_label` 을 그대로 쓴다.
//!
//! 글자 수는 `rules::formats::grapheme_len` 을 불러 센다(복사하지 않았다).
//!
//! Foundation 과 다른 점(원본 Swift 는 `Character`=자소 묶음 단위, 여기는 바이트·유니코드 글자 단위. 모두 시험 「추가」 절이 고정한다):
//! - `criterion_id_ok`: 원본은 ICU 정규식이라 `$` 가 끝의 줄바꿈 앞에서도 맞아 `"AC-1\n"` 이 통과한다. 여기는 줄바꿈이 붙은 id 를 거절한다
//!   (id 는 카드 id·파일 글에 들어간다 — 더 엄격한 쪽).
//! - `scenario_path_ok`: `starts_with("/")`·`"~"` 와 `/` 로 나누기가 바이트 기준이라, 원본이 `/`+결합 글자(`/\u{301}`)를 별개 글자로 보아 받는 경로를
//!   여기서는 거절한다(더 엄격한 쪽). 받는 것이 늘어나는 방향의 차이는 없다.
//! - `scenario_run` 의 첫 줄: 원본은 `"\n"` 으로 나누는데 `"\r\n"` 이 글자 하나라 CRLF 머리에서는 나눠지지 않고 `\r\n` 이 해석기 글자에 섞인다
//!   (그 파일은 도구 확인에서 막힌다). 여기도 같게 `\n` 이 `\r` 바로 뒤가 아닐 때만 줄 끝으로 보고 `\r`(바로 뒤가 `\n` 아님)·빈칸·탭으로 가른다.
//! - `#!` 접두와 `-` 접두 판정은 바이트 기준(바로 뒤에 결합 글자가 붙은 꼴만 갈린다). 오류 글의 `prefix(60)` 은 `grapheme_len` 으로 자소 묶음 60개에서 자른다.
//! - `trimmingCharacters(in: .whitespacesAndNewlines)` 는 `str::trim`(유니코드 White_Space, 같은 글자 집합)으로 옮겼다.
//! - 원본의 `Any?`(JSONSerialization 결과)는 `Option<&serde_json::Value>` 다. `Some(Null)`(NSNull)은 배열도 글자도 아니라 원본처럼 고칠 것을 낸다.

use super::formats::grapheme_len;
use serde_json::{json, Map, Value};

/// 시나리오 종류. 원본: `APScenarioKind`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScenarioKind {
    /// 명령·HTTP 요청을 단언하는 실행 파일(셸·파이썬·노드 등, 해석기는 첫 줄 #! 이나 확장자)
    Shell,
    /// 브라우저 시나리오: 해석기 + 브라우저 드라이버
    Browser,
}

impl ScenarioKind {
    /// 원본 `rawValue`
    pub fn as_str(self) -> &'static str {
        match self {
            ScenarioKind::Shell => "shell",
            ScenarioKind::Browser => "browser",
        }
    }

    /// 원본 `APScenarioKind(rawValue:)`
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "shell" => Some(ScenarioKind::Shell),
            "browser" => Some(ScenarioKind::Browser),
            _ => None,
        }
    }
}

/// 브라우저 드라이버. 원본: `APBrowserDriver`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrowserDriver {
    /// 깔려 있는 Chrome 계열 브라우저를 headless 로(엔진이 `$CLAUDE_BOARD_BROWSER`·`$CLAUDE_BOARD_BROWSER_FLAGS` 로 넘김)
    Chrome,
    /// node 모듈 playwright(보드는 깔지 않는다)
    Playwright,
}

impl BrowserDriver {
    /// 원본 `rawValue`
    pub fn as_str(self) -> &'static str {
        match self {
            BrowserDriver::Chrome => "chrome",
            BrowserDriver::Playwright => "playwright",
        }
    }

    /// 원본 `APBrowserDriver(rawValue:)`
    pub fn from_raw(s: &str) -> Option<Self> {
        match s {
            "chrome" => Some(BrowserDriver::Chrome),
            "playwright" => Some(BrowserDriver::Playwright),
            _ => None,
        }
    }
}

/// 판정 시나리오 하나. 원본: `APScenario`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    /// 작업 폴더(cwd) 기준 상대 경로
    pub file: String,
    pub kind: ScenarioKind,
    /// 짝지은 수용 기준 id
    pub criterion: Option<String>,
    /// 브라우저만(기본 chrome)
    pub driver: Option<BrowserDriver>,
}

impl Scenario {
    /// 원본 `init`: 브라우저면 드라이버가 없을 때 chrome, 셸이면 드라이버는 늘 없음
    pub fn new(file: &str, kind: ScenarioKind, criterion: Option<&str>, driver: Option<BrowserDriver>) -> Self {
        let driver = if kind == ScenarioKind::Browser { Some(driver.unwrap_or(BrowserDriver::Chrome)) } else { None };
        Scenario { file: file.to_string(), kind, criterion: criterion.map(str::to_string), driver }
    }
}

/// 수용 기준 하나. 원본: `APCriterion`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Criterion {
    pub id: String,
    pub text: String,
}

impl Criterion {
    pub fn new(id: &str, text: &str) -> Self {
        Criterion { id: id.to_string(), text: text.to_string() }
    }
}

/// 단위 하나의 시나리오 수 상한. 원본: `apScenarioMax`
pub const SCENARIO_MAX: usize = 30;

/// 작업 폴더 기준 상대 경로로 쓸 수 있는가(절대 경로·~·.. ·빈 조각·조절 글자·따옴표·역슬래시는 받지 않는다 — 고정본 자리와 샌드박스 글에 들어가므로). 원본: `apScenarioPathOK`
pub fn scenario_path_ok(p: &str) -> bool {
    if p.is_empty() || grapheme_len(p) > 300 || p.starts_with('/') || p.starts_with('~') {
        return false;
    }
    if p.chars().any(|c| (c as u32) < 0x20 || c == '"' || c == '\\' || c as u32 == 0x7f) {
        return false;
    }
    !p.split('/').any(|part| part.is_empty() || part == ".." || part == ".")
}

/// 수용 기준 id 꼴: 영문·숫자·`.-_` 1~40자(카드 id 와 파일 글에 들어간다). 원본: `apCriterionIdOK`
pub fn criterion_id_ok(s: &str) -> bool {
    !s.is_empty() && grapheme_len(s) <= 40 && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn problem_scenario(n: i64, k: usize, rest: &str) -> String {
    format!("단위 {n} 의 시나리오 {k} 의 {rest}")
}

/// 단위의 `scenarios` 칸 읽기. 고칠 것은 "단위 n 의 …" 문장으로(엔진 출발 전 확인의 고칠 것). 원본: `apParseScenarios`
pub fn parse_scenarios(v: Option<&Value>, unit: i64) -> (Vec<Scenario>, Vec<String>) {
    let n = unit;
    let Some(v) = v else { return (vec![], vec![]) };
    let Some(arr) = v.as_array() else { return (vec![], vec![format!("단위 {n} 의 scenarios 는 배열이어야 합니다")]) };
    let mut items: Vec<Scenario> = vec![];
    let mut problems: Vec<String> = vec![];
    if arr.len() > SCENARIO_MAX {
        problems.push(format!("단위 {n} 의 시나리오가 {}개입니다({SCENARIO_MAX}개까지)", arr.len()));
    }
    for (i, x) in arr.iter().take(SCENARIO_MAX).enumerate() {
        let k = i + 1;
        // 글자 하나면 셸 시나리오 파일 경로로 본다
        let o: Map<String, Value> = match x {
            Value::String(s) => {
                let mut m = Map::new();
                m.insert("file".to_string(), Value::String(s.clone()));
                m
            }
            Value::Object(m) => m.clone(),
            _ => Map::new(),
        };
        let file = match o.get("file").and_then(Value::as_str) {
            Some(f) if scenario_path_ok(f) => f,
            _ => {
                problems.push(problem_scenario(n, k, "file 은 작업 폴더 기준 상대 경로(.. 없이)여야 합니다"));
                continue;
            }
        };
        let mut kind = ScenarioKind::Shell;
        if let Some(ks) = o.get("kind") {
            match ks.as_str().and_then(ScenarioKind::from_raw) {
                Some(kk) => kind = kk,
                None => {
                    problems.push(problem_scenario(n, k, "kind 는 shell 또는 browser 입니다"));
                    continue;
                }
            }
        }
        let mut driver: Option<BrowserDriver> = None;
        if let Some(ds) = o.get("driver") {
            match ds.as_str().and_then(BrowserDriver::from_raw) {
                Some(dd) if kind == ScenarioKind::Browser => driver = Some(dd),
                _ => {
                    problems.push(problem_scenario(n, k, "driver 는 브라우저 시나리오에만, chrome 또는 playwright 입니다"));
                    continue;
                }
            }
        }
        let mut crit: Option<&str> = None;
        if let Some(c) = o.get("criterion") {
            match c.as_str() {
                Some(s) if criterion_id_ok(s) => crit = Some(s),
                _ => {
                    problems.push(problem_scenario(n, k, "criterion 은 수용 기준 id(영문·숫자·.-_)입니다"));
                    continue;
                }
            }
        }
        if items.iter().any(|s| s.file == file) {
            problems.push(format!("단위 {n} 의 시나리오 {file} 가 두 번 있습니다"));
            continue;
        }
        items.push(Scenario::new(file, kind, crit, driver));
    }
    (items, problems)
}

/// 단위의 `criteria` 칸 읽기(수용 기준: `[{id, text}]`). 원본: `apParseCriteria`
pub fn parse_criteria(v: Option<&Value>, unit: i64) -> (Vec<Criterion>, Vec<String>) {
    let n = unit;
    let Some(v) = v else { return (vec![], vec![]) };
    let Some(arr) = v.as_array() else { return (vec![], vec![format!("단위 {n} 의 criteria 는 배열이어야 합니다")]) };
    let mut items: Vec<Criterion> = vec![];
    let mut problems: Vec<String> = vec![];
    for (i, x) in arr.iter().enumerate() {
        let id = match x.as_object().and_then(|o| o.get("id")).and_then(Value::as_str) {
            Some(id) if criterion_id_ok(id) => id,
            _ => {
                problems.push(format!("단위 {n} 의 수용 기준 {} 에 id(영문·숫자·.-_)가 없습니다", i + 1));
                continue;
            }
        };
        let text = x.get("text").and_then(Value::as_str).unwrap_or("").trim();
        if text.is_empty() {
            problems.push(format!("단위 {n} 의 수용 기준 {id} 에 text 가 없습니다"));
            continue;
        }
        if items.iter().any(|c| c.id == id) {
            problems.push(format!("단위 {n} 의 수용 기준 {id} 가 두 번 있습니다"));
            continue;
        }
        items.push(Criterion::new(id, text));
    }
    (items, problems)
}

/// 시나리오가 가리킨 수용 기준이 목록에 있는가(수용 기준을 적은 단위만 본다). 원본: `apScenarioCriterionProblems`
pub fn scenario_criterion_problems(scenarios: &[Scenario], criteria: &[Criterion], unit: i64) -> Vec<String> {
    if criteria.is_empty() {
        return vec![];
    }
    scenarios
        .iter()
        .filter_map(|s| {
            let c = s.criterion.as_deref()?;
            if criteria.iter().any(|x| x.id == c) {
                None
            } else {
                Some(format!("단위 {unit} 의 시나리오 {} 가 가리킨 수용 기준 {c} 가 criteria 에 없습니다", s.file))
            }
        })
        .collect()
}

/// 실행 파일(시나리오)이 짝지어지지 않은 수용 기준 — 출발 카드로 묻는다(동결 뒤로는 묻지 않는다). 원본: `apCriteriaGaps`
pub fn criteria_gaps(criteria: &[Criterion], scenarios: &[Scenario]) -> Vec<Criterion> {
    criteria.iter().filter(|c| !scenarios.iter().any(|s| s.criterion.as_deref() == Some(c.id.as_str()))).cloned().collect()
}

/// 원본: `apScenarioJSON`(`file`·`kind`, 있으면 `criterion`·`driver`)
pub fn scenario_json(s: &Scenario) -> Value {
    let mut o = Map::new();
    o.insert("file".into(), json!(s.file));
    o.insert("kind".into(), json!(s.kind.as_str()));
    if let Some(c) = &s.criterion {
        o.insert("criterion".into(), json!(c));
    }
    if let Some(d) = s.driver {
        o.insert("driver".into(), json!(d.as_str()));
    }
    Value::Object(o)
}

/// 원본: `apCriterionJSON`
pub fn criterion_json(c: &Criterion) -> Value {
    json!({"id": c.id, "text": c.text})
}

/// 모델 판정으로 두기: 단위에 넣고, 그 기준은 결과 기록의 근거로 본다. 원본: `apCriterionModelJudge`
pub const CRITERION_MODEL_JUDGE: &str = "model_judge";
/// 범위에서 빼기(추천): 이 기준만 이번 범위에서 뺀다(단위는 남는다). 원본: `apCriterionDrop`
pub const CRITERION_DROP: &str = "drop_criterion";

/// 원본: `apCriterionCardId`
pub fn criterion_card_id(unit: &str, criterion: &str) -> String {
    format!("criterion-{unit}-{criterion}")
}

/// 카드의 고르기 하나. 원본: `APCardText.options` 의 `(key, label, note)`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardOption {
    pub key: String,
    pub label: String,
    pub note: String,
}

impl CardOption {
    fn new(key: &str, label: &str, note: &str) -> Self {
        CardOption { key: key.to_string(), label: label.to_string(), note: note.to_string() }
    }
}

/// 카드의 글. 원본: `APCardText`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardText {
    pub label: String,
    pub title: String,
    pub why: String,
    pub options: Vec<CardOption>,
    pub recommended: String,
}

/// 실행 파일이 없는 수용 기준 카드의 글. 추천은 범위에서 빼기(실행으로 확인하지 못한 통과를 남기지 않게. 설계 1장 "기준을 낮춰야만 통과하면 미통과"와 같은 뿌리).
/// 원본: `apCriterionCard`
pub fn criterion_card(unit_title: &str, c: &Criterion) -> CardText {
    CardText {
        label: "실행 파일이 없는 수용 기준".to_string(),
        title: format!("「{unit_title}」 의 수용 기준 {} 를 볼 판정 시나리오가 없습니다", c.id),
        why: format!("{} (이 기준은 실행 파일로 판정하지 않아 통과·실패를 기계로 가리지 못합니다.)", c.text),
        options: vec![
            CardOption::new(CRITERION_DROP, "범위에서 빼기", "이 기준만 이번 실행에서 하지 않습니다. 단위는 남고 다음 출발에서 다시 보입니다"),
            CardOption::new(CRITERION_MODEL_JUDGE, "모델 판정으로 두기", "단위에 넣고 그 기준은 결과 기록의 근거로 봅니다. 사람이 합치기·승격 때 확인합니다"),
        ],
        recommended: CRITERION_DROP.to_string(),
    }
}

/// 출발 때 고정할 수용 기준의 답: 시나리오가 있으면 "scenario", 없으면 카드 답(없거나 꼴이 틀리면 추천 = 범위에서 빼기, defaulted 참).
/// 원본: `apCriterionAnswer`
pub fn criterion_answer(c: &Criterion, scenarios: &[Scenario], answer: Option<&str>) -> (String, bool) {
    if scenarios.iter().any(|s| s.criterion.as_deref() == Some(c.id.as_str())) {
        return ("scenario".to_string(), false);
    }
    if let Some(a) = answer {
        if a == CRITERION_MODEL_JUDGE || a == CRITERION_DROP {
            return (a.to_string(), false);
        }
    }
    (CRITERION_DROP.to_string(), true)
}

/// 시나리오를 돌리는 데 필요한 도구. 원본: `APToolNeed`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ToolNeed {
    /// 절대 경로의 실행 파일(첫 줄 #! 의 해석기 등)
    Program(String),
    /// 로그인 셸 PATH 에서 찾는 명령
    Command(String),
    /// Chrome 계열 브라우저(드라이버 chrome)
    Browser,
    /// 작업 폴더에서 node 가 찾는 모듈(드라이버 playwright)
    NodeModule(String),
}

/// 원본: `apToolLabel`
pub fn tool_label(n: &ToolNeed) -> String {
    match n {
        ToolNeed::Program(p) => p.clone(),
        ToolNeed::Command(c) => format!("{c}(PATH)"),
        ToolNeed::Browser => "브라우저(Google Chrome·Chromium·Microsoft Edge)".to_string(),
        ToolNeed::NodeModule(m) => format!("node 모듈 {m}"),
    }
}

/// 시나리오를 돌리는 법. 원본: `APScenarioRun`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScenarioRun {
    /// 파일 경로 앞에 붙는 해석기와 인자
    pub interpreter: Vec<String>,
    pub needs: Vec<ToolNeed>,
    /// 돌리는 법을 모름
    pub problem: Option<String>,
}

// 원본 `extInterpreters`
fn ext_interpreter(ext: &str) -> Option<&'static [&'static str]> {
    Some(match ext {
        "sh" => &["/bin/sh"],
        "bash" => &["/bin/bash"],
        "zsh" => &["/bin/zsh"],
        "py" => &["/usr/bin/env", "python3"],
        "js" | "mjs" | "cjs" | "ts" | "mts" => &["/usr/bin/env", "node"],
        _ => return None,
    })
}

// `NSString.lastPathComponent`: 끝의 `/` 를 떼고 마지막 `/` 뒤
fn last_path_component(p: &str) -> &str {
    let t = p.trim_end_matches('/');
    t.rsplit('/').next().unwrap_or("")
}

// `NSString.pathExtension`: 마지막 조각에서 마지막 `.` 뒤(`.`이 맨 앞이면 확장자 없음)
fn path_extension(p: &str) -> &str {
    let comp = last_path_component(p);
    match comp.rfind('.') {
        Some(i) if i > 0 => &comp[i + 1..],
        _ => "",
    }
}

// 해석기 argv 에서 필요한 도구: 첫 조각은 절대 경로 실행 파일, env 면 다음 이름(-S·-i 같은 선택 칸은 건너뜀)을 PATH 에서
fn interpreter_needs(argv: &[String]) -> Vec<ToolNeed> {
    let Some(first) = argv.first() else { return vec![] };
    let mut needs = vec![ToolNeed::Program(first.clone())];
    if last_path_component(first) == "env" {
        if let Some(name) = argv.iter().skip(1).find(|a| !a.starts_with('-')) {
            needs.push(ToolNeed::Command(name.clone()));
        }
    }
    needs
}

// 원본 `head.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false).first`.
// Swift 의 `"\n"` 은 글자 하나라 `"\r\n"` 은 글자 하나로 묶여 나눠지지 않는다 → `\r` 바로 뒤가 아닌 첫 `\n` 앞까지
fn first_line(head: &str) -> &str {
    let b = head.as_bytes();
    let cut = (0..b.len()).find(|&i| b[i] == b'\n' && (i == 0 || b[i - 1] != b'\r')).unwrap_or(b.len());
    &head[..cut]
}

// 원본 `split(whereSeparator: { " " · "\t" · "\r" })`(빈 조각은 버림). `"\r\n"` 은 글자 하나라 구분자가 아니다
fn split_argv(s: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut cur = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        let sep = c == ' ' || c == '\t' || (c == '\r' && it.peek() != Some(&'\n'));
        if sep {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

// 원본 `String.prefix(60)`: 자소 묶음 60개까지(`grapheme_len` 으로 센다)
fn prefix_clusters(s: &str, n: usize) -> &str {
    if grapheme_len(s) <= n {
        return s;
    }
    let mut keep = 0;
    for (i, _) in s.char_indices().skip(1) {
        if grapheme_len(&s[..i]) > n {
            break;
        }
        keep = i;
    }
    &s[..keep]
}

/// 시나리오를 어떻게 돌릴까: 첫 줄이 #! 면 그 해석기(절대 경로여야 함), 아니면 확장자. 브라우저는 드라이버 도구를 더한다. 원본: `apScenarioRun`
pub fn scenario_run(s: &Scenario, head: &str) -> ScenarioRun {
    let mut argv: Vec<String> = vec![];
    let mut problem: Option<String> = None;
    let first = first_line(head);
    if let Some(rest) = first.strip_prefix("#!") {
        argv = split_argv(rest);
        if argv.first().is_none_or(|a| !a.starts_with('/')) {
            problem = Some(format!("첫 줄 #! 의 해석기가 절대 경로가 아닙니다({})", prefix_clusters(first, 60)));
            argv = vec![];
        }
    } else {
        let ext = path_extension(&s.file).to_lowercase();
        match ext_interpreter(&ext) {
            Some(a) => argv = a.iter().map(|x| x.to_string()).collect(),
            None => {
                problem = Some("돌리는 법을 모릅니다. 첫 줄에 #! 해석기를 쓰거나 확장자를 .sh·.bash·.zsh·.py·.js·.mjs·.cjs·.ts 로 씁니다".to_string());
            }
        }
    }
    let mut needs = interpreter_needs(&argv);
    if s.kind == ScenarioKind::Browser {
        match s.driver.unwrap_or(BrowserDriver::Chrome) {
            BrowserDriver::Chrome => needs.push(ToolNeed::Browser),
            BrowserDriver::Playwright => {
                let node = ToolNeed::Command("node".to_string());
                if !needs.contains(&node) {
                    needs.push(node);
                }
                needs.push(ToolNeed::NodeModule("playwright".to_string()));
            }
        }
    }
    ScenarioRun { interpreter: argv, needs, problem }
}
