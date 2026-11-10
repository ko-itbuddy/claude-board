//! (청크 R-Branches-a 전반만 이식됨 — 이 첫 줄은 `tests/ported.rs` 가 「구현됐는데 PORTED.json 에 없는 규칙」으로 잡지 않게 하는 표시다. R-Branches-b 가 후반을 옮긴 뒤 이 줄을 지우고 PORTED.json 항목을 넣는다)
//! 끝난 실행의 가지 기록(엔진 run.json 의 `branches` — 기록 저장소 `runs.branches` 도 같은 꼴)을 읽고 합치기 상태를 정하는 규칙.
//! 원본: `Sources/AutopilotRules/Branches.swift`. 엔진 finish 가 남긴 promoted·failed·discarded 로만 보고 git 은 부르지 않는다.
//! 입력은 JSON 값(`serde_json`)이다. 원본의 `as? String`·`as? [[String: Any]]` 캐스트는 같은 뜻으로 옮겼다: 글이 아니면 빈 글자,
//! 배열 안에 객체가 아닌 칸이 하나라도 있으면 그 배열 전체를 빈 목록으로 본다.
//!
//! 이식된 선언(원본 순서): `Step`(APStep) · `Promoted`(APPromoted) · `Failed`(APFailed) · `story_repos`(apStoryRepos) · `promoted_rows` ·
//! `failed_rows` · `unresolved` · `step_done` · `merge_sources` · 비공개 `discarded_sources`(apDiscardedSources) · `merge_open`.
//! 남은 선언(R-Branches-b): `APMergeMark`·`apMergeMark`(원본 「// 지난 실행 목록의 합치기 상태 표지」 줄부터) · `APRefFate` · `apDiscardRows` ·
//! `apCarriesBase` · `apFinishRoot`. 비공개 `discarded_sources` 는 `apMergeMark` 도 쓴다.
//!
//! Windows: 저장소 경로(`C:\w\a`·`/mnt/c/w/a`)는 JSON 키·글자 그대로 견주며 고치지 않는다.

use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};

/// JSON 객체 — 원본의 `[String: Any]`.
pub type Obj = Map<String, Value>;

/// JSON 숫자만 읽는다(원본 `apNum` = `as? NSNumber`). 글자로 적힌 수는 읽지 않는다.
/// Foundation 의 NSNumber 는 JSON `true`·`false` 도 받아 1·0 으로 읽으므로 같게 옮겼다.
fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        _ => None,
    }
}

/// `as? String ?? ""`
fn text(o: &Obj, key: &str) -> String {
    o.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

/// `as? [[String: Any]] ?? []` — 객체가 아닌 칸이 하나라도 있으면 전체가 빈 목록이다.
fn object_rows<'a>(br: &'a Obj, key: &str) -> Vec<&'a Obj> {
    let Some(Value::Array(items)) = br.get(key) else { return Vec::new() };
    let rows: Vec<&Obj> = items.iter().filter_map(Value::as_object).collect();
    if rows.len() == items.len() {
        rows
    } else {
        Vec::new()
    }
}

// ---- 가지 기록 읽기 ----

/// 승격 차례 한 단계(`projects/<프로젝트>.json` promotion: `from` 이 `autopilot` 이면 이 실행의 자율주행 가지). 원본: `APStep`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub from: String,
    pub into: String,
    pub after: Option<String>,
}

impl Step {
    pub fn new(from: &str, into: &str, after: Option<&str>) -> Self {
        Step { from: from.to_string(), into: into.to_string(), after: after.map(str::to_string) }
    }
}

/// 엔진 finish 가 합친 줄(`branches.promoted`) — `source` 는 `autopilot`·`story:<키>`·`branch:<가지>`. 원본: `APPromoted`
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Promoted {
    pub source: String,
    pub repo: String,
    pub branch: String,
    pub into: String,
    pub how: String,
    pub to: String,
    pub at: Option<f64>,
}

/// 엔진 finish 가 저장소 하나에서 하지 못한 것(먼저 본 확인을 지난 뒤의 중간 실패, `branches.failed`). 원본: `APFailed`
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Failed {
    pub op: String,
    pub source: String,
    pub repo: String,
    pub branch: String,
    pub into: String,
    pub why: String,
    pub at: Option<f64>,
}

/// 엔진 run.json 의 `branches.stories.<키>` 를 저장소별로 읽는다. 새 꼴 `{repos: {<저장소>: {branch, base, head, merged, unmerged}}}`,
/// 옛 꼴(스토리 하나에 저장소 하나 `{repo, branch, base, head, merged, unmerged}`)은 그 저장소 하나로 본다(`repo` 칸은 뗀다).
/// 저장소 기록에 `head` 가 없으면 그 저장소에는 스토리 가지를 만들지 않았다. 원본: `apStoryRepos`
pub fn story_repos(s: Option<&Value>) -> BTreeMap<String, Obj> {
    let Some(Value::Object(s)) = s else { return BTreeMap::new() };
    if let Some(Value::Object(rs)) = s.get("repos") {
        return rs.iter().filter_map(|(k, v)| v.as_object().map(|o| (k.clone(), o.clone()))).collect();
    }
    match s.get("repo").and_then(Value::as_str) {
        Some(r) if !r.is_empty() => {
            let mut o = s.clone();
            o.remove("repo");
            BTreeMap::from([(r.to_string(), o)])
        }
        _ => BTreeMap::new(),
    }
}

/// `branches.promoted` 줄. 원본: `apPromotedRows`
pub fn promoted_rows(br: &Obj) -> Vec<Promoted> {
    object_rows(br, "promoted")
        .into_iter()
        .map(|o| Promoted {
            source: text(o, "source"),
            repo: text(o, "repo"),
            branch: text(o, "branch"),
            into: text(o, "into"),
            how: text(o, "how"),
            to: text(o, "to"),
            at: num(o.get("at")),
        })
        .collect()
}

/// `branches.failed` 줄. 원본: `apFailedRows`
pub fn failed_rows(br: &Obj) -> Vec<Failed> {
    object_rows(br, "failed")
        .into_iter()
        .map(|o| Failed {
            op: text(o, "op"),
            source: text(o, "source"),
            repo: text(o, "repo"),
            branch: text(o, "branch"),
            into: text(o, "into"),
            why: text(o, "why"),
            at: num(o.get("at")),
        })
        .collect()
}

/// 합치기·올리기 가운데 하지 못하고 아직 다시 하지 않은 것 — 그 뒤 같은 합칠 쪽·받는 가지·저장소로 합친 줄(시각이 같거나 뒤)이 없으면 남았다.
/// 시각이 없으면 0 으로 견준다. `merge`·`promote` 가 아닌 못 한 일(cleanup 등)은 세지 않는다. 원본: `apUnresolved`
pub fn unresolved(promoted: &[Promoted], failed: &[Failed], source: Option<&str>, into: &str) -> Vec<Failed> {
    failed
        .iter()
        .filter(|f| {
            ["merge", "promote"].contains(&f.op.as_str())
                && f.into == into
                && source.is_none_or(|s| f.source == s)
                && !promoted.iter().any(|p| {
                    p.source == f.source && p.into == f.into && p.repo == f.repo && p.at.unwrap_or(0.0) >= f.at.unwrap_or(0.0)
                })
        })
        .cloned()
        .collect()
}

/// 단계가 끝났는가 — 엔진 finish 의 차례 규칙: 그 합칠 쪽(`source` 가 없으면 무엇이든)을 `into` 에 합친 줄이 있고, 일부 저장소만 하고 남긴 것이 없다.
/// 원본: `apStepDone`
pub fn step_done(promoted: &[Promoted], failed: &[Failed], source: Option<&str>, into: &str) -> bool {
    promoted.iter().any(|p| p.into == into && source.is_none_or(|s| p.source == s)) && unresolved(promoted, failed, source, into).is_empty()
}

// ---- 합치기 상태 ----

/// 합칠 쪽(엔진 finish 의 source) — 스토리마다 `story:<키>`(키 차례), 스토리가 없거나 `with_autopilot` 이면 자율주행 가지 `autopilot` 을 맨 앞에.
/// 어느 저장소에도 스토리 가지를 만들지 않은 스토리(저장소 기록에 `head` 칸이 없음)는 넣지 않는다. `stories` 가 객체가 아니면 스토리 없음.
/// 원본: `apMergeSources`
pub fn merge_sources(br: &Obj, with_autopilot: bool) -> Vec<String> {
    let empty = Obj::new();
    let st = br.get("stories").and_then(Value::as_object).unwrap_or(&empty);
    let mut keys: Vec<&String> = st.keys().collect();
    keys.sort();
    let mut sources: Vec<String> = keys
        .into_iter()
        .filter(|k| story_repos(st.get(k.as_str())).values().any(|r| r.contains_key("head")))
        .map(|k| format!("story:{k}"))
        .collect();
    if sources.is_empty() || with_autopilot {
        sources.insert(0, "autopilot".to_string());
    }
    sources
}

/// 버린 합칠 쪽(`branches.discarded` 의 `source`). 원본: `apDiscardedSources`(비공개)
fn discarded_sources(br: &Obj) -> BTreeSet<String> {
    object_rows(br, "discarded").into_iter().filter_map(|o| o.get("source").and_then(Value::as_str).map(str::to_string)).collect()
}

/// 합치기·승격에 남은 것이 있는가 — 보드 본체의 끝난 실행 줄 「합치기 기다림」. 엔진 finish 의 promoted·discarded 로만 본다.
/// 합칠 쪽(`merge_sources`)이 버려지지도 승격 차례의 마지막 자율주행 단계에 합쳐지지도 않았거나, 하나라도 그 단계에 합쳤는데 기존 가지끼리의
/// 마지막 단계를 아직 안 했으면 남았다. 자율주행 단계가 없는 차례는 합친 줄이 하나라도 있으면 닿은 것으로 본다.
/// 일부 저장소만 합친 단계(`failed` 가 남고 그 뒤 그 저장소를 합치지 않음)는 끝나지 않은 것이다. 원본: `apMergeOpen`
pub fn merge_open(br: &Obj, steps: &[Step], with_autopilot: bool) -> bool {
    let promoted = promoted_rows(br);
    let failed = failed_rows(br);
    let discarded = discarded_sources(br);
    let done = |src: Option<&str>, into: &str| step_done(&promoted, &failed, src, into);
    let sources = merge_sources(br, with_autopilot);
    let last_auto = steps.iter().rev().find(|s| s.from == "autopilot").map(|s| s.into.as_str());
    let reached = |s: &str| match last_auto {
        Some(into) => done(Some(s), into),
        None => promoted.iter().any(|p| p.source == s),
    };
    if sources.iter().any(|s| !discarded.contains(s) && !reached(s)) {
        return true;
    }
    if let Some(last) = steps.last() {
        if last.from != "autopilot" && sources.iter().any(|s| reached(s)) && !done(None, &last.into) {
            return true;
        }
    }
    false
}
