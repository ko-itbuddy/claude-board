//! (청크 Critique a/2 — 원본 1~393줄. PORTED.json 항목은 마지막 청크가 더한다)
//! 자율주행 규칙(AutopilotRules) — 판정 비판 일반판의 앞쪽(5단계 ⑪-3, 설계 2장 "판정과 지시의 비판·다듬기"). 원본: `Sources/AutopilotRules/Critique.swift` 1~393줄.
//! 판정(골든셋)을 쓴 모델의 것을 그대로 믿지 않는다. 출발 준비 때 ① 반대 탐침(모델 호출 0)으로 판정이 보는 자리의 꼴에서 기계로 망친 결과물
//! (빈 파일·꼴만 맞는 엉터리·잘린 파일·실행하면 실패하는 코드·빈 본문에 200 을 주는 가짜 서버)을 버리는 작업 트리에 만들어 판정을 돌린다.
//! 망친 결과물로 통과하면 그 판정은 뜻을 보지 않는다는 뜻이라 출발 카드("판정 고치기(추천)/그대로/빼기")를 만든다. ② 비판 세션은 뒤 청크(R-Critique-b)가 옮긴다.
//! 반례·탐침 파일은 믿지 않는 출력이다: 작업 폴더 기준 상대 경로만, 판정 자체(시나리오·판정 스크립트·시험 파일)·하네스·.git·.autopilot 자리는 받지 않는다.
//! 사람이 읽는 글(준비 화면·카드·모델에게 넘기는 글)에는 긴 줄표를 쓰지 않는다.
//!
//! 이식 선언(원본 순서): MARK `판정이 보는 자리` — `JudgeText`(APJudgeText) · `JudgeView`(APJudgeView) · `judge_paths`(apJudgePaths) · `judge_run_scripts`(apJudgeRunScripts) ·
//! `test_helper_path`(apTestHelperPath) · [`path_key`](apPathKey 는 `rules::path_key` 를 다시 내보낸다) · `JUDGE_CONFIG_NAMES`(apJudgeConfigNames) · `judge_config_path` ·
//! `protected_judge_path` · `judge_files` · `case_path_ok` · `case_path_problem` · `PROBE_TARGET_MAX` · `probe_targets` · `PROBE_LITERAL_MAX` · `judge_literals`.
//! MARK `① 반대 탐침` — `ProbeKind`(APProbeKind) · `probe_label` · `ProbeFile` · `Probe` · `probe_plan`(apProbePlan) · MARK `버리는 작업 트리에서 돌리기` —
//! `ProbeWorkspace`(포트 APProbeWorkspace) · `ProbeRun` · `ProbeHit` · `ProbeReport` · `run_probes`(apRunProbes) · `FIX_CHECK` · `probe_card`(apProbeCard) · `probe_run_json`.
//!
//! 차이(원본 대비, 이 포트의 자리):
//! - 정규식은 손으로 훑는다: `urlRe`·`pathRe`·`testFlagRe`·`runRe`·`dotRunRe`·`dqRe`·`sqRe` 는 정규식 크레이트 없이(lookbehind·lookahead 포함) 같은 뜻의 훑기로 옮겼다.
//!   `\s` 는 ICU 처럼 `\t \n \f \r` 와 Z 범주, `\b` 는 글자·숫자·`_` 기준이다. `pathRe` 의 확장자 판정은 「점으로 나눈 조각 가운데 확장자 목록과 같은 조각」을 가장 긴 쪽부터 본다.
//! - `ProbeWorkspace` 포트는 `ports.rs` 에 아직 없어(Ports.swift 201줄) 이 파일에 둔다(scenarios.rs 의 `JudgeRunner` 와 같은 자리). 뒤 청크가 쓰는 쪽도 여기서 `use` 한다.
//!   원본은 클래스(참조)라 `&self` 로 받는다(바꾸는 것은 구현이 안쪽에서).
//! - `jsonString`(JSONSerialization 으로 문자열 하나를 직렬화): 직접 쓴다. 이스케이프는 `\" \\ \b \f \n \r \t` 와 그 밖 제어 글자 `\u00xx`(소문자 16진수), 슬래시는 그대로.
//!   Foundation 이 그 밖 제어 글자의 16진수를 대문자로 쓰는지는 확인하지 않았다(판정 글에서 따옴표 안에 제어 글자가 들 일은 탭·CR 뿐이다).
//! - `keysOf` 의 `^[A-Za-z_][A-Za-z0-9_]{0,40}$`: ICU 의 `$` 는 끝의 줄 끝 글자 앞에도 맞으므로 끝의 줄 끝 글자 하나(`\n`·`\r\n`·`\r`·U+0085·U+2028·U+2029)는 떼고 견준다.
//! - `String.count`·`prefix`(Character 단위)는 `formats::grapheme_len` 과 그 근사로, `CharacterSet.alphanumerics` 는 `char::is_alphanumeric`(글자·숫자) 근사다.
//! - `[String: Any]`(탐침 카드·실행 JSON)는 `serde_json::Value`(`preserve_order`)이다. 단위 번호(`Int`)는 `i64`.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::rules::formats::grapheme_len;
pub use crate::rules::path_key::path_key;
use crate::rules::scenarios::{scenario_path_ok, JudgeOutcome, StepResult};

// MARK: - 판정이 보는 자리

/// 출발 기준의 시나리오 글. 원본: `APJudgeText`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgeText {
    /// 대기열에 적힌 시나리오 경로(작업 폴더 기준)
    pub file: String,
    /// 출발 기준의 글(없으면 빈 글자)
    pub text: String,
}

impl JudgeText {
    pub fn new(file: &str, text: &str) -> Self {
        JudgeText { file: file.to_string(), text: text.to_string() }
    }
}

/// 단위 하나의 판정 — 판정 명령 글·시나리오 글·판정이 스스로 띄우는 포트. 원본: `APJudgeView`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgeView {
    pub unit: i64,
    pub unit_id: String,
    pub title: String,
    pub check: String,
    pub scenarios: Vec<JudgeText>,
    pub check_ports: Vec<i64>,
}

impl JudgeView {
    /// 원본 `init`: `checkPorts` 는 빈 목록으로 시작한다
    pub fn new(unit: i64, unit_id: &str, title: &str, check: &str, scenarios: Vec<JudgeText>) -> Self {
        JudgeView { unit, unit_id: unit_id.to_string(), title: title.to_string(), check: check.to_string(), scenarios, check_ports: vec![] }
    }
}

// ICU `\s`: `\t \n \f \r` 와 Z 범주
fn re_space(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{c}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{2028}' | '\u{2029}' | '\u{202F}' | '\u{205F}' | '\u{3000}'
    )
}

// 글 at 에서 시작하는 빈칸 묶음 뒤의 바이트 위치(빈칸이 없으면 at)
fn skip_space(s: &str, at: usize) -> usize {
    let mut i = at;
    for c in s[at..].chars() {
        if !re_space(c) {
            break;
        }
        i += c.len_utf8();
    }
    i
}

// 글 i 자리 글자 하나의 바이트 길이
fn step(s: &str, i: usize) -> usize {
    s[i..].chars().next().map_or(1, char::len_utf8)
}

fn is_start(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

// `[A-Za-z0-9_.-]`
fn is_dir_char(b: u8) -> bool {
    is_start(b) || b == b'.' || b == b'-'
}

// `[A-Za-z0-9_./-]`
fn is_cap_char(b: u8) -> bool {
    is_dir_char(b) || b == b'/'
}

const PATH_EXTS: &[&str] = &[
    "json", "jsonl", "txt", "md", "csv", "tsv", "yaml", "yml", "toml", "ini", "cfg", "conf", "html", "htm", "xml", "sql", "log", "py", "js", "mjs", "cjs", "ts", "sh",
];

fn path_ext(seg: &[u8]) -> bool {
    std::str::from_utf8(seg).is_ok_and(|t| PATH_EXTS.contains(&t))
}

// 원본 `urlRe` = `[A-Za-z][A-Za-z0-9+.-]*://[^\s'"`)]+` 가 i 에서 맞으면 끝 위치
fn url_end(s: &str, i: usize) -> Option<usize> {
    let b = s.as_bytes();
    let mut j = i + 1;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || matches!(b[j], b'+' | b'.' | b'-')) {
        j += 1;
    }
    if !s[j..].starts_with("://") {
        return None;
    }
    let k = j + 3;
    let mut e = k;
    for c in s[k..].chars() {
        if re_space(c) || matches!(c, '\'' | '"' | '`' | ')') {
            break;
        }
        e += c.len_utf8();
    }
    (e > k).then_some(e)
}

// 주소를 빈칸 하나로 바꾼 글
fn strip_urls(text: &str) -> String {
    let b = text.as_bytes();
    let (mut out, mut copied, mut i) = (String::new(), 0, 0);
    while i < b.len() {
        if b[i].is_ascii_alphabetic() {
            if let Some(end) = url_end(text, i) {
                out.push_str(&text[copied..i]);
                out.push(' ');
                copied = end;
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out.push_str(&text[copied..]);
    out
}

// 원본 `pathRe` 의 마지막 조각: `[A-Za-z0-9_][A-Za-z0-9_-]*(?:\.[A-Za-z0-9_-]+)*\.(?:확장자)(?![A-Za-z0-9_/-])` 가 p 에서 맞으면 끝 위치(가장 긴 쪽)
fn path_final(b: &[u8], p: usize) -> Option<usize> {
    if p >= b.len() || !is_start(b[p]) {
        return None;
    }
    let mut r = p;
    while r < b.len() && is_dir_char(b[r]) {
        r += 1;
    }
    let (mut best, mut seg_start, mut idx) = (None, p, 0);
    loop {
        let seg_end = b[seg_start..r].iter().position(|&c| c == b'.').map_or(r, |o| seg_start + o);
        if idx >= 1 {
            if seg_end == seg_start {
                break;
            }
            if path_ext(&b[seg_start..seg_end]) {
                let next = b.get(seg_end).copied();
                if !matches!(next, Some(c) if c.is_ascii_alphanumeric() || matches!(c, b'_' | b'/' | b'-')) {
                    best = Some(seg_end);
                }
            }
        }
        if seg_end >= r {
            break;
        }
        seg_start = seg_end + 1;
        idx += 1;
    }
    best
}

// 원본 `pathRe` 가 i 에서 맞으면 끝 위치: `(?:[A-Za-z0-9_][A-Za-z0-9_.-]*/)*` 폴더 조각을 가장 많이 잡은 쪽부터 마지막 조각을 맞춰 본다
fn path_end(b: &[u8], i: usize) -> Option<usize> {
    let mut dir_ends = vec![i];
    loop {
        let p = *dir_ends.last().unwrap_or(&i);
        if p < b.len() && is_start(b[p]) {
            let mut q = p + 1;
            while q < b.len() && is_dir_char(b[q]) {
                q += 1;
            }
            if q < b.len() && b[q] == b'/' {
                dir_ends.push(q + 1);
                continue;
            }
        }
        break;
    }
    dir_ends.iter().rev().find_map(|&d| path_final(b, d))
}

// 원본 `matches(pathRe, plain)`: 앞 글자가 `[A-Za-z0-9_./~$-]` 가 아닌 자리에서만 시작한다(lookbehind)
fn path_matches(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let (mut out, mut i) = (vec![], 0);
    while i < b.len() {
        let prev_ok = i == 0 || !(is_dir_char(b[i - 1]) || matches!(b[i - 1], b'/' | b'~' | b'$'));
        if prev_ok && is_start(b[i]) {
            if let Some(end) = path_end(b, i) {
                out.push(s[i..end].to_string());
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out
}

// 원본 `matches(testFlagRe, plain)` = `(?:\btest|\[)\s+-[efs]\s+['"]?([A-Za-z0-9_][A-Za-z0-9_./-]*)` 의 묶음 1
fn test_flag_matches(s: &str) -> Vec<String> {
    let b = s.as_bytes();
    let (mut out, mut i) = (vec![], 0);
    while i < s.len() {
        let head = if s[i..].starts_with("test") {
            let boundary = s[..i].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
            boundary.then_some(i + 4)
        } else if s[i..].starts_with('[') {
            Some(i + 1)
        } else {
            None
        };
        if let Some(cap) = head.and_then(|j| test_flag_capture(s, b, j)) {
            out.push(s[cap.0..cap.1].to_string());
            i = cap.1;
            continue;
        }
        i += step(s, i);
    }
    out
}

fn test_flag_capture(s: &str, b: &[u8], j: usize) -> Option<(usize, usize)> {
    let w = skip_space(s, j);
    if w == j || b.get(w) != Some(&b'-') || !matches!(b.get(w + 1), Some(b'e' | b'f' | b's')) {
        return None;
    }
    let k = w + 2;
    let w2 = skip_space(s, k);
    if w2 == k {
        return None;
    }
    let mut c0 = w2;
    if matches!(b.get(c0), Some(b'\'' | b'"')) {
        c0 += 1;
    }
    capture_run(b, c0)
}

// 묶음 `([A-Za-z0-9_][A-Za-z0-9_./-]*)` 가 c0 에서 맞으면 (시작, 끝)
fn capture_run(b: &[u8], c0: usize) -> Option<(usize, usize)> {
    if c0 >= b.len() || !is_start(b[c0]) {
        return None;
    }
    let mut e = c0 + 1;
    while e < b.len() && is_cap_char(b[e]) {
        e += 1;
    }
    Some((c0, e))
}

// `(?:^|[\s;&|(])` 로 시작하는 두 정규식(`runRe`·`dotRunRe`)의 훑기. body 는 앞붙이 뒤 위치에서 묶음 (시작, 끝)을 낸다
fn prefixed_matches(s: &str, body: fn(&str, usize) -> Option<(usize, usize)>) -> Vec<String> {
    let (mut out, mut i) = (vec![], 0);
    while i < s.len() {
        let mut found = if i == 0 { body(s, 0) } else { None };
        if found.is_none() {
            let c = s[i..].chars().next().unwrap_or(' ');
            if re_space(c) || matches!(c, ';' | '&' | '|' | '(') {
                found = body(s, i + c.len_utf8());
            }
        }
        match found {
            Some((a, e)) => {
                out.push(s[a..e].to_string());
                i = e;
            }
            None => i += step(s, i),
        }
    }
    out
}

const INTERPRETERS: [&str; 11] = ["python3", "python", "node", "sh", "bash", "zsh", "ruby", "perl", "deno", "bun", "swift"];

// `runRe` 의 앞붙이 뒤: `(?:python3?|node|…)(?:\s+-[A-Za-z]+)*\s+['"]?(?:\./)?([A-Za-z0-9_][A-Za-z0-9_./-]*)`
fn run_body(s: &str, p: usize) -> Option<(usize, usize)> {
    let b = s.as_bytes();
    INTERPRETERS.iter().filter(|n| s[p..].starts_with(**n)).find_map(|n| {
        let mut cur = p + n.len();
        loop {
            let w = skip_space(s, cur);
            if w > cur && b.get(w) == Some(&b'-') {
                let mut m = w + 1;
                while m < b.len() && b[m].is_ascii_alphabetic() {
                    m += 1;
                }
                if m > w + 1 {
                    cur = m;
                    continue;
                }
            }
            break;
        }
        let w = skip_space(s, cur);
        if w == cur {
            return None;
        }
        let mut c0 = w;
        if matches!(b.get(c0), Some(b'\'' | b'"')) {
            c0 += 1;
        }
        if s[c0..].starts_with("./") {
            c0 += 2;
        }
        capture_run(b, c0)
    })
}

// `dotRunRe` 의 앞붙이 뒤: `\./([A-Za-z0-9_][A-Za-z0-9_./-]*)`
fn dot_run_body(s: &str, p: usize) -> Option<(usize, usize)> {
    s[p..].starts_with("./").then(|| capture_run(s.as_bytes(), p + 2)).flatten()
}

// `dqRe`(`"([^"\n\\$`]{2,80})"`)·`sqRe`(`'([^'\n\\]{2,80})'`) 의 묶음 1: 여는 따옴표 뒤 금지 글자가 나올 때까지가 2~80자이고 그 글자가 닫는 따옴표여야 한다
fn quoted_matches(s: &str, quote: char, banned: &[char]) -> Vec<String> {
    let (mut out, mut i) = (vec![], 0);
    while i < s.len() {
        let c = s[i..].chars().next().unwrap_or(' ');
        if c == quote {
            let (mut n, mut at, mut closed) = (0usize, i + 1, false);
            for d in s[i + 1..].chars() {
                if d == quote || banned.contains(&d) {
                    closed = d == quote;
                    break;
                }
                n += 1;
                at += d.len_utf8();
            }
            if closed && (2..=80).contains(&n) {
                out.push(s[i + 1..at].to_string());
                i = at + 1;
                continue;
            }
        }
        i += c.len_utf8();
    }
    out
}

/// 글 안의 작업 폴더 기준 상대 경로 꼴(확장자가 있거나 `test -f` 의 대상). 주소(`http://…`)는 먼저 지운다. 원본: `apJudgePaths`
pub fn judge_paths(text: &str) -> Vec<String> {
    let plain = strip_urls(text);
    let mut out: Vec<String> = vec![];
    for p in path_matches(&plain).into_iter().chain(test_flag_matches(&plain)) {
        if scenario_path_ok(&p) && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// 판정 명령이 돌리는 스크립트(판정 자체라 탐침·반례로 바꾸지 않는다). 원본: `apJudgeRunScripts`
pub fn judge_run_scripts(check: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    // 확장자나 / 가 없는 조각(node -e 1 의 1, python3 -m pytest 의 pytest)은 파일이 아니다
    for p in prefixed_matches(check, run_body).into_iter().chain(prefixed_matches(check, dot_run_body)) {
        if scenario_path_ok(&p) && (p.contains('.') || p.contains('/')) && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

fn parts_of(key: &str) -> Vec<&str> {
    key.split('/').filter(|x| !x.is_empty()).collect()
}

/// 시험 파일 꼴(판정을 돕는 파일): `tests/`·`test/`·`spec/`·`__tests__/` 아래, `test_*.`·`*_test.`·`*.test.`·`*.spec.`. 원본: `apTestHelperPath`
pub fn test_helper_path(p: &str) -> bool {
    let key = path_key(p);
    let parts = parts_of(&key);
    let dirs = &parts[..parts.len().saturating_sub(1)];
    if dirs.iter().any(|d| ["test", "tests", "spec", "specs", "__tests__"].contains(d)) {
        return true;
    }
    let name = parts.last().copied().unwrap_or("");
    name.starts_with("test_") || name.contains("_test.") || name.contains(".test.") || name.contains(".spec.")
}

/// 판정에 영향을 주는 설정 파일 이름(⑪-3 자동 보안 검토 MEDIUM 둘째): 반례가 이것을 바꾸면 결과물이 틀려도 판정이 통과하는 가짜 확인이 된다.
/// 반례는 허용 목록(그 단위 판정이 보는 자리)으로만 받고, 허용 목록에 들었어도 이 꼴은 받지 않는다(덧막기). 원본: `apJudgeConfigNames`
pub const JUDGE_CONFIG_NAMES: &[&str] = &[
    "makefile",
    "gnumakefile",
    "package.json",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "conftest.py",
    "pytest.ini",
    "pyproject.toml",
    "setup.cfg",
    "setup.py",
    "tox.ini",
    "noxfile.py",
    "sitecustomize.py",
    "usercustomize.py",
    ".npmrc",
    ".yarnrc",
    ".yarnrc.yml",
    ".pnpmfile.cjs",
    ".env",
    ".envrc",
    "deno.json",
    "deno.jsonc",
    "bunfig.toml",
    "tsconfig.json",
    "go.mod",
    "cargo.toml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "settings.gradle.kts",
    "pom.xml",
    "gradle.properties",
    "justfile",
    "rakefile",
    "gemfile",
];

/// 판정에 영향을 주는 설정 파일 꼴인가. 원본: `apJudgeConfigPath`
pub fn judge_config_path(p: &str) -> bool {
    let key = path_key(p);
    let parts = parts_of(&key);
    let Some(name) = parts.last().copied() else { return false };
    if JUDGE_CONFIG_NAMES.contains(&name) || name.ends_with(".pth") {
        return true;
    }
    for stem in ["jest.config.", "vitest.config.", "babel.config.", "karma.conf.", "playwright.config.", "vite.config.", ".babelrc", ".mocharc"] {
        if name.starts_with(stem) {
            return true;
        }
    }
    parts[..parts.len() - 1].iter().any(|d| [".github", ".husky", "node_modules", ".venv", "venv"].contains(d))
}

/// 탐침·반례가 쓰면 안 되는 자리: 판정 자체(시나리오·판정 스크립트·시험 파일), 하네스(`CLAUDE.md`·`.claude/`·`.mcp.json`·`AGENTS.md`), `.git`, `.autopilot`(판정 시나리오 자리).
/// 대소문자·유니코드 정규화를 접어 견준다(`path_key`). 원본: `apProtectedJudgePath`
pub fn protected_judge_path(p: &str, judge_files: &[String]) -> bool {
    let k = path_key(p);
    let parts = parts_of(&k);
    if parts.contains(&".git") || parts.contains(&".claude") || parts.first() == Some(&".autopilot") {
        return true;
    }
    if let Some(last) = parts.last() {
        if ["claude.md", "claude.local.md", "agents.md", ".mcp.json"].contains(last) {
            return true;
        }
    }
    judge_files.iter().any(|f| path_key(f) == k) || test_helper_path(p)
}

/// 단위 판정 자체를 이루는 파일(시나리오와 판정 명령이 돌리는 스크립트). 원본: `apJudgeFiles`
pub fn judge_files(v: &JudgeView) -> Vec<String> {
    v.scenarios.iter().map(|s| s.file.clone()).chain(judge_run_scripts(&v.check)).collect()
}

/// 반례 파일 경로를 받을 수 있나: 작업 폴더 기준 상대 경로이고 판정·하네스·`.git` 자리가 아님. 원본: `apCasePathOK`
pub fn case_path_ok(p: &str, judge_files: &[String]) -> bool {
    scenario_path_ok(p) && !protected_judge_path(p, judge_files)
}

/// 반례가 그 파일을 바꿔도 되나(⑪-3 자동 보안 검토 MEDIUM 둘째): 그 단위 판정이 보는 자리(`probe_targets`)에 들고(대소문자·정규화를 접어 견줌),
/// 판정 자체·하네스·`.git` 자리도, 판정에 영향을 주는 설정 파일 꼴도 아님. 아니면 까닭. 원본: `apCasePathProblem`
pub fn case_path_problem(p: &str, judge_files: &[String], targets: &[String]) -> Option<String> {
    if !scenario_path_ok(p) {
        return Some("작업 폴더 기준 상대 경로가 아닙니다".to_string());
    }
    if protected_judge_path(p, judge_files) {
        return Some("판정·하네스 자리라 보드가 받지 않았습니다".to_string());
    }
    if judge_config_path(p) {
        return Some("판정에 영향을 주는 설정 파일이라 보드가 받지 않았습니다".to_string());
    }
    let k = path_key(p);
    if !targets.iter().any(|t| path_key(t) == k) {
        let seen = if targets.is_empty() { "찾지 못함".to_string() } else { targets.iter().take(6).cloned().collect::<Vec<_>>().join(", ") };
        return Some(format!("판정이 보는 자리({seen})가 아닌 파일이라 보드가 받지 않았습니다"));
    }
    None
}

/// 탐침 대상 상한. 원본: `apProbeTargetMax`
pub const PROBE_TARGET_MAX: usize = 12;

/// 탐침 대상: 시나리오 글과 판정 명령 글이 가리키는 파일 가운데 판정 자체·보호 자리가 아닌 것(차례대로, 12개까지). 원본: `apProbeTargets`
pub fn probe_targets(v: &JudgeView) -> Vec<String> {
    let judge = judge_files(v);
    let mut out: Vec<String> = vec![];
    for t in v.scenarios.iter().map(|s| s.text.as_str()).chain(std::iter::once(v.check.as_str())) {
        for p in judge_paths(t) {
            if !out.iter().any(|o| path_key(o) == path_key(&p)) && !protected_judge_path(&p, &judge) {
                out.push(p);
            }
        }
    }
    out.truncate(PROBE_TARGET_MAX);
    out
}

/// 찾는 글자 상한. 원본: `apProbeLiteralMax`
pub const PROBE_LITERAL_MAX: usize = 24;

// Swift `trimmingCharacters(in: .whitespaces)` — Zs 범주와 탭
fn trim_ws(s: &str) -> &str {
    s.trim_matches(|c| matches!(c, '\t' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}'))
}

/// 판정 글이 찾는 글자(따옴표 안 2~80자, 글자·숫자가 하나는 있음). 망친 결과물에 그대로 넣어 "글자만 보는 판정"을 가린다. 원본: `apJudgeLiterals`
pub fn judge_literals(texts: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for t in texts {
        let dq = quoted_matches(t, '"', &['\n', '\\', '$', '`']);
        let sq = quoted_matches(t, '\'', &['\n', '\\']);
        for s in dq.into_iter().chain(sq) {
            let x = trim_ws(&s).to_string();
            if grapheme_len(&x) >= 2 && x.chars().any(char::is_alphanumeric) && !out.contains(&x) {
                out.push(x);
            }
        }
    }
    out.truncate(PROBE_LITERAL_MAX);
    out
}

// MARK: - ① 반대 탐침

/// 반대 탐침의 종류. 원본: `APProbeKind`
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProbeKind {
    /// 빈 결과 파일
    Empty,
    /// 꼴만 맞고 값이 엉터리(자료: 판정이 찾는 칸·글자에 엉터리 값, 코드: 그 글자를 담았지만 실행하면 실패)
    Junk,
    /// 꼴만 맞는 엉터리 목록(JSON 대상이 있을 때)
    JunkList,
    /// 잘린 파일(자료는 중간에서 끊김, 코드는 문법 오류)
    Truncated,
    /// 빈 본문에 200 을 주는 가짜 서버(판정이 스스로 띄우는 포트가 있고 코드 대상이 있을 때)
    StubServer,
}

impl ProbeKind {
    /// 원본 `CaseIterable.allCases`
    pub const ALL: [ProbeKind; 5] = [ProbeKind::Empty, ProbeKind::Junk, ProbeKind::JunkList, ProbeKind::Truncated, ProbeKind::StubServer];

    /// 원본 `rawValue`
    pub fn as_str(self) -> &'static str {
        match self {
            ProbeKind::Empty => "empty",
            ProbeKind::Junk => "junk",
            ProbeKind::JunkList => "junk_list",
            ProbeKind::Truncated => "truncated",
            ProbeKind::StubServer => "stub_server",
        }
    }

    pub fn from_raw(s: &str) -> Option<Self> {
        ProbeKind::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// 원본: `apProbeLabel`
pub fn probe_label(k: ProbeKind) -> &'static str {
    match k {
        ProbeKind::Empty => "빈 결과 파일",
        ProbeKind::Junk => "꼴만 맞고 값이 엉터리인 결과물(코드는 실행하면 실패)",
        ProbeKind::JunkList => "꼴만 맞는 엉터리 목록",
        ProbeKind::Truncated => "잘린 파일",
        ProbeKind::StubServer => "빈 본문에 200 을 주는 가짜 서버",
    }
}

/// 탐침이 쓰는 파일 하나. 원본: `APProbeFile`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeFile {
    pub path: String,
    pub text: String,
}

/// 탐침 하나(종류와 쓸 파일들). 원본: `APProbe`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probe {
    pub kind: ProbeKind,
    pub files: Vec<ProbeFile>,
}

// 원본 비공개 `FileShape`
enum Shape {
    Json,
    Jsonl,
    Code(String),
    Text,
}

// Foundation `NSString.pathExtension`: 마지막 조각의 마지막 점 뒤(점이 맨 앞이거나 뒤가 비면 없음)
fn path_extension(p: &str) -> String {
    let last = p.rsplit('/').next().unwrap_or("");
    match last.rfind('.') {
        Some(i) if i > 0 && i + 1 < last.len() => last[i + 1..].to_string(),
        _ => String::new(),
    }
}

fn shape(p: &str) -> Shape {
    let ext = path_extension(p).to_lowercase();
    match ext.as_str() {
        "json" => Shape::Json,
        "jsonl" => Shape::Jsonl,
        "py" | "js" | "mjs" | "cjs" | "ts" | "sh" => Shape::Code(ext),
        _ => Shape::Text,
    }
}

// JSONSerialization 으로 문자열 하나를 직렬화한 글(따옴표 포함, 슬래시는 그대로)
fn json_string(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\u{8}' => o.push_str("\\b"),
            '\u{c}' => o.push_str("\\f"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

const PROBE_MARK: &str = "반대 탐침(Claude Board): 판정이 망친 결과물을 막는지 보려고 일부러 만든 파일입니다";
const JUNK_VALUE: &str = "zz-probe";

// ICU `^[A-Za-z_][A-Za-z0-9_]{0,40}$` — `$` 는 끝의 줄 끝 글자 하나 앞에도 맞는다
fn key_like(s: &str) -> bool {
    let t = s
        .strip_suffix("\r\n")
        .or_else(|| s.strip_suffix(['\n', '\r', '\u{85}', '\u{2028}', '\u{2029}']))
        .unwrap_or(s);
    let mut it = t.chars();
    let Some(first) = it.next() else { return false };
    (first.is_ascii_alphabetic() || first == '_') && t.chars().count() <= 41 && it.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn keys_of(literals: &[String]) -> Vec<String> {
    let ks: Vec<String> = literals.iter().filter(|l| key_like(l)).cloned().collect();
    if ks.is_empty() {
        vec!["zz_probe".to_string()]
    } else {
        ks
    }
}

fn junk_object(keys: &[String]) -> String {
    format!("{{{}}}", keys.iter().map(|k| format!("{}: {}", json_string(k), json_string(JUNK_VALUE))).collect::<Vec<_>>().join(", "))
}

fn comment_lines(lits: &[String], lead: &str) -> String {
    lits.iter().map(|l| format!("{lead} {}", l.replace('\n', " "))).collect::<Vec<_>>().join("\n")
}

fn lit_array(lits: &[String]) -> String {
    format!("[{}]", lits.iter().map(|l| json_string(l)).collect::<Vec<_>>().join(", "))
}

fn junk_text(p: &str, lits: &[String]) -> String {
    match shape(p) {
        Shape::Json | Shape::Jsonl => junk_object(&keys_of(lits)) + "\n",
        Shape::Text => {
            let lines: Vec<String> = if lits.is_empty() { vec![JUNK_VALUE.to_string()] } else { lits.iter().map(|l| format!("{l} {JUNK_VALUE}")).collect() };
            lines.join("\n") + "\n"
        }
        Shape::Code(ext) => {
            let arr = lit_array(lits);
            match ext.as_str() {
                "py" => format!("# {PROBE_MARK}\nZZ_PROBE = {arr}\nraise SystemExit(3)\n"),
                "sh" => format!("#!/bin/sh\n# {PROBE_MARK}\n{}{}exit 3\n", comment_lines(lits, "#"), if lits.is_empty() { "" } else { "\n" }),
                _ => format!("// {PROBE_MARK}\nconst ZZ_PROBE = {arr};\nprocess.exit(3);\n"),
            }
        }
    }
}

// Swift `String.prefix(n)`(Character 단위) 근사 — grapheme 으로 센 n 개 덩어리까지
fn take(s: &str, n: usize) -> String {
    if n == 0 {
        return String::new();
    }
    for (i, c) in s.char_indices() {
        if grapheme_len(&s[..i + c.len_utf8()]) == n + 1 {
            return s[..i].to_string();
        }
    }
    s.to_string()
}

fn truncated_text(p: &str, lits: &[String]) -> String {
    match shape(p) {
        Shape::Json | Shape::Jsonl => format!("{{{}: \"zz-pro", json_string(&keys_of(lits)[0])),
        Shape::Text => {
            let first = lits.first().map_or(JUNK_VALUE, String::as_str);
            take(first, 1.max(grapheme_len(first) / 2))
        }
        Shape::Code(ext) => match ext.as_str() {
            "py" => format!("# {PROBE_MARK}\n{}\ndef zz_probe(:\n", comment_lines(lits, "#")),
            "sh" => format!("#!/bin/sh\n# {PROBE_MARK}\n{}\nif true; then\n", comment_lines(lits, "#")),
            _ => format!("// {PROBE_MARK}\n{}\nfunction zz_probe( {{\n", comment_lines(lits, "//")),
        },
    }
}

fn stub_text(p: &str, lits: &[String], ports: &[i64]) -> Option<String> {
    let Shape::Code(ext) = shape(p) else { return None };
    if ext == "sh" {
        return None;
    }
    let arr = lit_array(lits);
    let ps = ports.iter().map(i64::to_string).collect::<Vec<_>>().join(", ");
    if ext == "py" {
        return Some(format!(
            "# {PROBE_MARK}\n\
             # 무엇을 물어도 200 과 빈 본문을 돌려주는 가짜 서버입니다(불러오기만 하면 아무것도 하지 않음)\n\
             import http.server, threading\n\
             ZZ_PROBE = {arr}\n\
             class _ZZ(http.server.BaseHTTPRequestHandler):\n\
             \x20   def _ok(self):\n\
             \x20       self.send_response(200); self.send_header(\"Content-Length\", \"0\"); self.end_headers()\n\
             \x20   do_GET = do_POST = do_PUT = do_DELETE = do_PATCH = do_HEAD = _ok\n\
             \x20   def log_message(self, *a):\n\
             \x20       pass\n\
             if __name__ == \"__main__\":\n\
             \x20   servers = [http.server.ThreadingHTTPServer((\"127.0.0.1\", p), _ZZ) for p in [{ps}]]\n\
             \x20   for s in servers[1:]:\n\
             \x20       threading.Thread(target=s.serve_forever, daemon=True).start()\n\
             \x20   servers[0].serve_forever()\n"
        ));
    }
    Some(format!(
        "// {PROBE_MARK}\n\
         // 무엇을 물어도 200 과 빈 본문을 돌려주는 가짜 서버입니다\n\
         const ZZ_PROBE = {arr};\n\
         import(\"node:http\").then((m) => {{ const http = m.default || m; for (const p of [{ps}]) http.createServer((q, s) => {{ s.writeHead(200, {{ \"Content-Length\": \"0\" }}); s.end(); }}).listen(p, \"127.0.0.1\"); }});\n"
    ))
}

/// 탐침 계획: 대상·찾는 글자와 탐침들. 원본이 돌려주는 이름 붙은 튜플 `(targets, literals, probes)`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbePlan {
    pub targets: Vec<String>,
    pub literals: Vec<String>,
    pub probes: Vec<Probe>,
}

/// 탐침 계획: 대상·찾는 글자와 탐침들(같은 판정이면 같은 글자 — 시험이 된다). 대상이 없으면 탐침도 없다. 원본: `apProbePlan`
pub fn probe_plan(v: &JudgeView) -> ProbePlan {
    let targets = probe_targets(v);
    if targets.is_empty() {
        return ProbePlan { targets: vec![], literals: vec![], probes: vec![] };
    }
    let texts: Vec<&str> = v.scenarios.iter().map(|s| s.text.as_str()).chain(std::iter::once(v.check.as_str())).collect();
    let lits = judge_literals(&texts);
    let each = |f: &dyn Fn(&str) -> String| -> Vec<ProbeFile> { targets.iter().map(|p| ProbeFile { path: p.clone(), text: f(p) }).collect() };
    let mut probes: Vec<Probe> = vec![];
    probes.push(Probe { kind: ProbeKind::Empty, files: each(&|_| String::new()) });
    probes.push(Probe { kind: ProbeKind::Junk, files: each(&|p| junk_text(p, &lits)) });
    if targets.iter().any(|t| matches!(shape(t), Shape::Json)) {
        probes.push(Probe {
            kind: ProbeKind::JunkList,
            files: each(&|p| if matches!(shape(p), Shape::Json) { format!("[{}]\n", junk_object(&keys_of(&lits))) } else { junk_text(p, &lits) }),
        });
    }
    probes.push(Probe { kind: ProbeKind::Truncated, files: each(&|p| truncated_text(p, &lits)) });
    let ports: Vec<i64> = v.check_ports.iter().copied().filter(|p| (1024..=65535).contains(p)).collect();
    if !ports.is_empty() && targets.iter().any(|t| stub_text(t, &lits, &ports).is_some()) {
        probes.push(Probe { kind: ProbeKind::StubServer, files: each(&|p| stub_text(p, &lits, &ports).unwrap_or_else(|| junk_text(p, &lits))) });
    }
    ProbePlan { targets, literals: lits, probes }
}

// MARK: - 버리는 작업 트리에서 돌리기 (포트 ProbeWorkspace, Ports.swift)

/// 버리는 작업 트리(반대 탐침·반례 확인) — 출발 기준(출발 전 자율주행 가지 머리 또는 출발 가지 머리)의 분리 작업 트리. 저장소에는 쓰지 않는다.
/// 진짜 어댑터는 엔진: 쓰기는 그 작업 트리 안 상대 경로만(링크를 따라가지 않음), 판정은 ⑪-1 판정 샌드박스 안에서 고정본으로. 원본: `APProbeWorkspace`(Ports.swift)
pub trait ProbeWorkspace {
    /// 단위 n 의 버리는 작업 트리를 출발 기준 그대로 되돌린다(없으면 만든다). 되면 `None`, 아니면 까닭
    fn reset(&self, unit: i64) -> Option<String>;
    /// 그 작업 트리의 단위 작업 폴더 기준 상대 경로에 파일을 쓴다. 되면 `None`, 아니면 까닭
    fn put(&self, unit: i64, relative: &str, data: &[u8]) -> Option<String>;
    /// 단위 판정을 그 작업 트리에서 돌린다. label 은 증거 폴더 이름 조각(`probe-<종류>` · `case-<지적 id>`)
    fn judge(&self, unit: i64, label: &str) -> JudgeOutcome;
}

/// 탐침 하나를 돌린 결과. 원본: `APProbeRun`
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeRun {
    pub kind: ProbeKind,
    /// 이 탐침으로 통과한 판정 걸음 이름(일하기 전에 이미 통과한 걸음은 뺌)
    pub passed_steps: Vec<String>,
    /// 단위 판정 전체가 통과
    pub passed: bool,
    pub sec: f64,
    /// 돌리지 못한 까닭
    pub note: Option<String>,
}

/// 망친 결과물로 통과한 판정 걸음. 원본: `APProbeHit`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeHit {
    /// 걸음 이름 `scenario-n` · `check`
    pub step: String,
    pub label: String,
    pub file: Option<String>,
    pub criterion: Option<String>,
    pub kinds: Vec<ProbeKind>,
}

/// 탐침 보고. 원본: `APProbeReport`
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProbeReport {
    pub targets: Vec<String>,
    pub literals: Vec<String>,
    pub runs: Vec<ProbeRun>,
    pub hits: Vec<ProbeHit>,
    /// 탐침을 돌리지 않은 까닭
    pub note: Option<String>,
}

// 걸음이 통과했나(종료 코드 0, 시간 상한·중단 아님)
fn step_passed(r: &StepResult) -> bool {
    r.code == 0 && !r.timed_out && !r.interrupted
}

/// 탐침을 차례로 돌린다. 일하기 전 판정(baseline)이 이미 통과한 걸음은 보지 않는다(그건 판정 미리 돌리기 카드). 판정 전체가 일하기 전부터 통과하면 돌리지 않는다.
/// `budget_sec`: 단위 하나의 탐침에 쓸 판정 시간(넘으면 남은 탐침을 하지 않고 까닭을 남김). 원본: `apRunProbes`
pub fn run_probes(v: &JudgeView, baseline: &JudgeOutcome, ws: &dyn ProbeWorkspace, budget_sec: f64) -> ProbeReport {
    let mut rep = ProbeReport::default();
    if baseline.passed() {
        rep.note = Some("일하기 전부터 판정이 통과해 반대 탐침을 돌리지 않았습니다(판정 미리 돌리기 카드로)".to_string());
        return rep;
    }
    if baseline.interrupted() {
        rep.note = Some("판정 미리 돌리기가 끊겨 반대 탐침을 돌리지 않았습니다".to_string());
        return rep;
    }
    let plan = probe_plan(v);
    rep.targets = plan.targets;
    rep.literals = plan.literals;
    if plan.probes.is_empty() {
        rep.note = Some("판정 글에서 판정이 보는 파일을 찾지 못해 반대 탐침을 돌리지 않았습니다".to_string());
        return rep;
    }
    let already: Vec<&str> = baseline.steps.iter().zip(baseline.results.iter()).filter(|(_, r)| step_passed(r)).map(|(s, _)| s.name.as_str()).collect();
    let mut spent = 0.0;
    let mut by_step: HashMap<String, Vec<ProbeKind>> = HashMap::new();
    let skipped = |kind: ProbeKind, note: String| ProbeRun { kind, passed_steps: vec![], passed: false, sec: 0.0, note: Some(note) };
    for p in &plan.probes {
        if spent > budget_sec {
            rep.runs.push(skipped(p.kind, format!("탐침에 쓸 판정 시간({}초)을 넘어 돌리지 않았습니다", budget_sec as i64)));
            continue;
        }
        if let Some(why) = ws.reset(v.unit) {
            rep.runs.push(skipped(p.kind, why));
            continue;
        }
        if let Some(why) = p.files.iter().find_map(|f| ws.put(v.unit, &f.path, f.text.as_bytes())) {
            rep.runs.push(skipped(p.kind, why));
            continue;
        }
        let o = ws.judge(v.unit, &format!("probe-{}", p.kind.as_str()));
        let sec: f64 = o.results.iter().map(|r| r.sec).sum();
        spent += sec;
        let passed: Vec<String> =
            o.steps.iter().zip(o.results.iter()).filter(|(s, r)| step_passed(r) && !already.contains(&s.name.as_str())).map(|(s, _)| s.name.clone()).collect();
        for s in &passed {
            by_step.entry(s.clone()).or_default().push(p.kind);
        }
        rep.runs.push(ProbeRun { kind: p.kind, passed_steps: passed, passed: o.passed(), sec, note: None });
    }
    let _ = ws.reset(v.unit);
    for s in &baseline.steps {
        if let Some(kinds) = by_step.get(&s.name) {
            rep.hits.push(ProbeHit { step: s.name.clone(), label: s.label.clone(), file: s.file.clone(), criterion: s.criterion.clone(), kinds: kinds.clone() });
        }
    }
    rep
}

/// 탐침 카드의 추천 키. 원본: `apFixCheck`
pub const FIX_CHECK: &str = "fix_check";

/// 탐침 카드(prerun.json 의 `probe_cards`). 추천은 판정 고치기. 원본: `apProbeCard`
pub fn probe_card(n: i64, unit_id: &str, title: &str, hit: &ProbeHit, targets: &[String]) -> Value {
    let what = hit.kinds.iter().map(|k| probe_label(*k)).collect::<Vec<_>>().join(" · ");
    let place = hit.file.as_ref().map_or("판정 명령".to_string(), |f| format!("시나리오 {f}"));
    let crit = hit.criterion.as_ref().map_or(String::new(), |c| format!("(수용 기준 {c})"));
    let mut o = Map::new();
    o.insert("id".into(), json!(format!("probe-{unit_id}-{}", hit.file.as_deref().unwrap_or("check"))));
    o.insert("unit".into(), json!(n));
    o.insert("unit_id".into(), json!(unit_id));
    o.insert("kind".into(), json!("probe"));
    o.insert("step".into(), json!(hit.step));
    o.insert("title".into(), json!(format!("「{title}」 의 {place}{crit}이 망친 결과물로도 통과합니다")));
    o.insert(
        "why".into(),
        json!(format!(
            "{what}(으)로 바꾼 버리는 작업 트리에서 이 판정이 통과했습니다. 판정이 결과의 뜻(값·동작)을 보지 않고 꼴만 보는 것 같습니다. 탐침 대상: {}",
            targets.join(", ")
        )),
    );
    o.insert("options".into(), json!([{"key": FIX_CHECK, "label": "판정 고치기"}, {"key": "keep", "label": "그대로"}, {"key": "drop", "label": "빼기"}]));
    o.insert("recommended".into(), json!(FIX_CHECK));
    o.insert("probes".into(), json!(hit.kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>()));
    if let Some(f) = &hit.file {
        o.insert("file".into(), json!(f));
    }
    if let Some(c) = &hit.criterion {
        o.insert("criterion".into(), json!(c));
    }
    Value::Object(o)
}

/// 탐침 실행 하나의 JSON(초는 밀리초로 반올림). 원본: `apProbeRunJSON`
pub fn probe_run_json(r: &ProbeRun) -> Value {
    let mut o = Map::new();
    o.insert("kind".into(), json!(r.kind.as_str()));
    o.insert("label".into(), json!(probe_label(r.kind)));
    o.insert("passed_steps".into(), json!(r.passed_steps));
    o.insert("passed".into(), json!(r.passed));
    o.insert("sec".into(), json!((r.sec * 1000.0).round() / 1000.0));
    if let Some(n) = &r.note {
        o.insert("note".into(), json!(n));
    }
    Value::Object(o)
}

// R-Critique-b 가 여기서 이어 쓴다
