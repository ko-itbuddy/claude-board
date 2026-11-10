//! (청크 Scenarios 1/2) 시험 실행기 규칙(`rules::scenarios`) 시험 — 전반. 원본: `Tests/AutopilotRulesTests/ScenariosTests.swift`.
//! 이 파일이 옮긴 Swift 시험 4개: `읽기`(표 9줄) · `배열` · `수용기준` · `해석기`(표 9줄) — 같은 이름. 표의 글자 값은 `convert.py` 로 Swift 파일에서 기계로 옮겼다
//! (`#"…"#` → `r#"…"#`). 마지막 줄 「상한 30개」만 원본이 코드로 만드는 입력이라 시험 함수 안에서 같은 식으로 만든다. 표 길이를 세어 줄이 빠지면 실패한다.
//! 남은 Swift 시험(R-Scenarios-b): `환경` · `옛판정` · `여럿` · `멈춤` · `지문` · `요약` · `카드`(+ 가짜 `FakeProbe`·`FakeRunner`·`pinned`).
//! `JudgeSandboxTests` 5개(`거르개`·`골든`·`차례`·`네트워크`·`ipc`)는 macOS Seatbelt 프로필 시험이라 이식하지 않는다(plan.md 의 이식불가 행).
//! 끝의 「추가」 절은 Swift 시험에 없는 이 이식의 몫(Foundation 과 다른 점·경계)이다.

use claude_board_autopilot::rules::scenarios::{
    criteria_gaps, criterion_answer, criterion_card, criterion_card_id, criterion_id_ok, criterion_json, parse_criteria, parse_scenarios,
    scenario_criterion_problems, scenario_json, scenario_path_ok, scenario_run, tool_label, BrowserDriver, Criterion, Scenario, ScenarioKind,
    ToolNeed, CRITERION_DROP, CRITERION_MODEL_JUDGE, SCENARIO_MAX,
};
use serde_json::{json, Value};

struct ParseRow {
    note: &'static str,
    json: &'static str,
    files: &'static [&'static str],
    problems: usize,
}

struct InterpreterRow {
    note: &'static str,
    file: &'static str,
    kind: ScenarioKind,
    driver: Option<BrowserDriver>,
    head: &'static str,
    argv: &'static [&'static str],
    needs: Vec<ToolNeed>,
    problem: bool,
}

fn sh(file: &str) -> Scenario {
    Scenario::new(file, ScenarioKind::Shell, None, None)
}

fn sh_crit(file: &str, crit: &str) -> Scenario {
    Scenario::new(file, ScenarioKind::Shell, Some(crit), None)
}

const PARSE_ROWS: [ParseRow; 8] = [
    ParseRow { note: "글자 하나는 셸 시나리오", json: r#"["t/a.sh"]"#, files: &["t/a.sh"], problems: 0 },
    ParseRow { note: "브라우저·기준·드라이버", json: r#"[{"file": "t/p.mjs", "kind": "browser", "criterion": "AC-2", "driver": "playwright"}]"#, files: &["t/p.mjs"], problems: 0 },
    ParseRow { note: "절대 경로는 받지 않음", json: r#"["/etc/passwd"]"#, files: &[], problems: 1 },
    ParseRow { note: "..·~·빈 조각·따옴표·. 조각은 받지 않음", json: r#"["../x.sh", "~/x.sh", "a//b.sh", "a\"b.sh", "./a.sh"]"#, files: &[], problems: 5 },
    ParseRow { note: "kind 꼴", json: r#"[{"file": "a.sh", "kind": "gui"}]"#, files: &[], problems: 1 },
    ParseRow { note: "driver 는 브라우저만", json: r#"[{"file": "a.sh", "driver": "chrome"}]"#, files: &[], problems: 1 },
    ParseRow { note: "criterion 꼴", json: r#"[{"file": "a.sh", "criterion": "기준 1"}]"#, files: &[], problems: 1 },
    ParseRow { note: "같은 파일 두 번", json: r#"["a.sh", {"file": "a.sh"}]"#, files: &["a.sh"], problems: 1 },
];

// INTERPRETER ROWS (needs_N is replaced below)
fn interpreter_rows() -> Vec<InterpreterRow> {
    vec![
        InterpreterRow { note: "#! 절대 경로", file: "a", kind: ScenarioKind::Shell, driver: None, head: "#!/bin/bash -e\necho", argv: &["/bin/bash", "-e"], needs: vec![ToolNeed::Program("/bin/bash".to_string())], problem: false },
        InterpreterRow { note: "#! env 는 다음 이름을 PATH 에서", file: "a", kind: ScenarioKind::Shell, driver: None, head: "#!/usr/bin/env python3\n", argv: &["/usr/bin/env", "python3"], needs: vec![ToolNeed::Program("/usr/bin/env".to_string()), ToolNeed::Command("python3".to_string())], problem: false },
        InterpreterRow { note: "#! env -S", file: "a", kind: ScenarioKind::Shell, driver: None, head: "#!/usr/bin/env -S node --no-warnings\n", argv: &["/usr/bin/env", "-S", "node", "--no-warnings"], needs: vec![ToolNeed::Program("/usr/bin/env".to_string()), ToolNeed::Command("node".to_string())], problem: false },
        InterpreterRow { note: "#! 상대 해석기는 받지 않음", file: "a.sh", kind: ScenarioKind::Shell, driver: None, head: "#!bash\n", argv: &[], needs: vec![], problem: true },
        InterpreterRow { note: "확장자 .sh", file: "t/a.sh", kind: ScenarioKind::Shell, driver: None, head: "curl …", argv: &["/bin/sh"], needs: vec![ToolNeed::Program("/bin/sh".to_string())], problem: false },
        InterpreterRow { note: "확장자 .mjs", file: "t/a.mjs", kind: ScenarioKind::Shell, driver: None, head: "import x", argv: &["/usr/bin/env", "node"], needs: vec![ToolNeed::Program("/usr/bin/env".to_string()), ToolNeed::Command("node".to_string())], problem: false },
        InterpreterRow { note: "모르는 확장자", file: "t/a.rb", kind: ScenarioKind::Shell, driver: None, head: "puts 1", argv: &[], needs: vec![], problem: true },
        InterpreterRow { note: "브라우저 chrome", file: "p.sh", kind: ScenarioKind::Browser, driver: None, head: "#!/bin/sh\n", argv: &["/bin/sh"], needs: vec![ToolNeed::Program("/bin/sh".to_string()), ToolNeed::Browser], problem: false },
        InterpreterRow { note: "브라우저 playwright", file: "p.mjs", kind: ScenarioKind::Browser, driver: Some(BrowserDriver::Playwright), head: "import { chromium } from 'playwright'", argv: &["/usr/bin/env", "node"], needs: vec![ToolNeed::Program("/usr/bin/env".to_string()), ToolNeed::Command("node".to_string()), ToolNeed::NodeModule("playwright".to_string())], problem: false },
    ]
}

fn check_parse_row(note: &str, raw: &Value, files: &[String], problems: usize) {
    let (items, ps) = parse_scenarios(Some(raw), 2);
    let got: Vec<&str> = items.iter().map(|s| s.file.as_str()).collect();
    assert_eq!(got, files, "{note}");
    assert_eq!(ps.len(), problems, "{note}: {ps:?}");
    assert!(ps.iter().all(|p| p.starts_with("단위 2 의 ")), "{note}: {ps:?}");
}

#[test]
fn 읽기() {
    assert_eq!(PARSE_ROWS.len(), 8, "표 줄이 빠졌다");
    for r in PARSE_ROWS.iter() {
        let raw: Value = serde_json::from_str(r.json).unwrap_or_else(|e| panic!("{}: {e}", r.note));
        let files: Vec<String> = r.files.iter().map(|s| s.to_string()).collect();
        check_parse_row(r.note, &raw, &files, r.problems);
    }
    // 「상한 30개」 — Swift: "[" + (1...31).map { "\"s\($0).sh\"" }.joined(separator: ",") + "]", files: (1...30).map { "s\($0).sh" }, problems: 1
    let json31 = format!("[{}]", (1..=31).map(|i| format!("\"s{i}.sh\"")).collect::<Vec<_>>().join(","));
    let raw: Value = serde_json::from_str(&json31).unwrap();
    let files: Vec<String> = (1..=30).map(|i| format!("s{i}.sh")).collect();
    check_parse_row("상한 30개", &raw, &files, 1);
}

#[test]
fn 배열() {
    assert_eq!(parse_scenarios(Some(&json!("a.sh")), 1).1, vec!["단위 1 의 scenarios 는 배열이어야 합니다".to_string()]);
    assert!(parse_scenarios(None, 1).0.is_empty());
    assert_eq!(parse_scenarios(Some(&json!([{"file": "p.js", "kind": "browser"}])), 1).0.first().and_then(|s| s.driver), Some(BrowserDriver::Chrome));
}

#[test]
fn 수용기준() {
    let raw = json!([{"id": "AC-1", "text": "예약을 만들면 201"}, {"id": "AC-2", "text": " 겹치면 409 "}, {"id": "AC-2", "text": "x"}, {"text": "id 없음"}, {"id": "AC-3", "text": ""}]);
    let (cs, ps) = parse_criteria(Some(&raw), 1);
    assert_eq!(cs, vec![Criterion::new("AC-1", "예약을 만들면 201"), Criterion::new("AC-2", "겹치면 409")]);
    assert_eq!(
        ps,
        vec![
            "단위 1 의 수용 기준 AC-2 가 두 번 있습니다".to_string(),
            "단위 1 의 수용 기준 4 에 id(영문·숫자·.-_)가 없습니다".to_string(),
            "단위 1 의 수용 기준 AC-3 에 text 가 없습니다".to_string(),
        ]
    );
    let ss = vec![sh_crit("a.sh", "AC-1"), sh_crit("b.sh", "AC-9")];
    assert_eq!(criteria_gaps(&cs, &ss), vec![Criterion::new("AC-2", "겹치면 409")]);
    assert_eq!(scenario_criterion_problems(&ss, &cs, 1), vec!["단위 1 의 시나리오 b.sh 가 가리킨 수용 기준 AC-9 가 criteria 에 없습니다".to_string()]);
    assert!(scenario_criterion_problems(&ss, &[], 1).is_empty()); // 기준을 적지 않은 단위는 이름표로만
    assert_eq!(criterion_answer(&cs[0], &ss, Some("drop_criterion")), ("scenario".to_string(), false));
    assert_eq!(criterion_answer(&cs[1], &ss, Some("model_judge")), ("model_judge".to_string(), false));
    assert_eq!(criterion_answer(&cs[1], &ss, None), ("drop_criterion".to_string(), true));
    assert_eq!(criterion_answer(&cs[1], &ss, Some("keep")), ("drop_criterion".to_string(), true));
    let card = criterion_card("예약 API", &cs[1]);
    assert_eq!(card.recommended, "drop_criterion");
    assert_eq!(card.options.iter().map(|o| o.key.as_str()).collect::<Vec<_>>(), vec!["drop_criterion", "model_judge"]);
    let all: String = card.title.clone() + &card.why + &card.options.iter().map(|o| format!("{}{}", o.label, o.note)).collect::<String>();
    assert!(!all.contains('—'));
    assert_eq!(criterion_card_id("u2", "AC-2"), "criterion-u2-AC-2");
}

#[test]
fn 해석기() {
    let rows = interpreter_rows();
    assert_eq!(rows.len(), 9, "표 줄이 빠졌다");
    for r in rows {
        let x = scenario_run(&Scenario::new(r.file, r.kind, None, r.driver), r.head);
        let want: Vec<String> = r.argv.iter().map(|s| s.to_string()).collect();
        assert_eq!(x.interpreter, want, "{}", r.note);
        assert_eq!(x.needs, r.needs, "{}", r.note);
        assert_eq!(x.problem.is_some(), r.problem, "{}", r.note);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// 추가 — Swift 시험에 없는 것(원본 동작 고정·Foundation 과 다른 점·경계)
// ---------------------------------------------------------------------------------------------------------------------

#[test]
fn 추가_경로_꼴() {
    assert!(scenario_path_ok("t/a.sh"));
    assert!(scenario_path_ok("a.sh"));
    assert!(!scenario_path_ok(""));
    assert!(!scenario_path_ok("/a"));
    assert!(!scenario_path_ok("~a"));
    assert!(!scenario_path_ok("a/../b"));
    assert!(!scenario_path_ok("a/./b"));
    assert!(!scenario_path_ok("a/"));
    assert!(!scenario_path_ok("a\\b"));
    assert!(!scenario_path_ok("a\"b"));
    assert!(!scenario_path_ok("a\u{7f}b"));
    assert!(!scenario_path_ok("a\u{1f}b"));
    assert!(!scenario_path_ok("a\nb"));
    // 300 자(자소 묶음)까지: 결합 글자는 앞 글자와 한 글자로 센다
    assert!(scenario_path_ok(&"a".repeat(300)));
    assert!(!scenario_path_ok(&"a".repeat(301)));
    assert!(scenario_path_ok(&format!("{}\u{301}", "a".repeat(300))));
    // Foundation 과 다른 점: `/`+결합 글자는 바이트 기준이라 거절(원본은 받음, 더 엄격한 쪽)
    assert!(!scenario_path_ok("/\u{301}x"));
    assert!(!scenario_path_ok("~\u{301}x"));
}

#[test]
fn 추가_기준_id_꼴() {
    assert!(criterion_id_ok("AC-1"));
    assert!(criterion_id_ok("a.b_c-9"));
    assert!(!criterion_id_ok(""));
    assert!(!criterion_id_ok("기준 1"));
    assert!(!criterion_id_ok("a b"));
    assert!(criterion_id_ok(&"a".repeat(40)));
    assert!(!criterion_id_ok(&"a".repeat(41)));
    // Foundation 과 다른 점: ICU `$` 는 끝 줄바꿈 앞에서도 맞아 원본은 "AC-1\n" 을 받는다. 여기는 거절한다(더 엄격한 쪽)
    assert!(!criterion_id_ok("AC-1\n"));
}

#[test]
fn 추가_읽기_경계() {
    assert!(parse_scenarios(Some(&json!([])), 1).1.is_empty());
    // NSNull 은 배열도 글자도 아니다
    assert_eq!(parse_scenarios(Some(&Value::Null), 3).1, vec!["단위 3 의 scenarios 는 배열이어야 합니다".to_string()]);
    assert_eq!(parse_criteria(Some(&Value::Null), 3).1, vec!["단위 3 의 criteria 는 배열이어야 합니다".to_string()]);
    assert_eq!(parse_criteria(Some(&json!("x")), 1).1, vec!["단위 1 의 criteria 는 배열이어야 합니다".to_string()]);
    assert!(parse_criteria(None, 1).0.is_empty() && parse_criteria(None, 1).1.is_empty());
    // 글자도 객체도 아닌 칸은 file 이 없는 것
    let (items, ps) = parse_scenarios(Some(&json!([3, null, {"file": 3}])), 4);
    assert!(items.is_empty());
    assert_eq!(ps.len(), 3);
    assert_eq!(ps[0], "단위 4 의 시나리오 1 의 file 은 작업 폴더 기준 상대 경로(.. 없이)여야 합니다");
    // 값이 글자가 아닌 kind·driver·criterion 은 고칠 것
    assert_eq!(parse_scenarios(Some(&json!([{"file": "a.sh", "kind": 3}])), 1).1, vec!["단위 1 의 시나리오 1 의 kind 는 shell 또는 browser 입니다".to_string()]);
    assert_eq!(
        parse_scenarios(Some(&json!([{"file": "a.mjs", "kind": "browser", "driver": "firefox"}])), 1).1,
        vec!["단위 1 의 시나리오 1 의 driver 는 브라우저 시나리오에만, chrome 또는 playwright 입니다".to_string()]
    );
    assert_eq!(parse_scenarios(Some(&json!([{"file": "a.sh", "criterion": null}])), 1).1, vec!["단위 1 의 시나리오 1 의 criterion 은 수용 기준 id(영문·숫자·.-_)입니다".to_string()]);
    // 같은 파일 두 번의 고칠 것 글과 상한 초과 글
    assert_eq!(parse_scenarios(Some(&json!(["a.sh", "a.sh"])), 2).1, vec!["단위 2 의 시나리오 a.sh 가 두 번 있습니다".to_string()]);
    let many: Vec<Value> = (1..=SCENARIO_MAX + 1).map(|i| json!(format!("s{i}.sh"))).collect();
    let (items, ps) = parse_scenarios(Some(&Value::Array(many)), 2);
    assert_eq!((items.len(), ps), (SCENARIO_MAX, vec!["단위 2 의 시나리오가 31개입니다(30개까지)".to_string()]));
    // 상한 밖의 항목은 읽지 않는다: 31 번째가 틀려도 고칠 것은 상한 글 하나뿐
    let mut over: Vec<Value> = (1..=SCENARIO_MAX).map(|i| json!(format!("s{i}.sh"))).collect();
    over.push(json!("/bad"));
    assert_eq!(parse_scenarios(Some(&Value::Array(over)), 2).1.len(), 1);
    // 브라우저는 driver 가 없으면 chrome, 셸은 늘 없음
    let (items, _) = parse_scenarios(Some(&json!([{"file": "a.sh", "kind": "shell"}, {"file": "b.sh", "kind": "browser", "driver": "playwright", "criterion": "AC-1"}])), 1);
    assert_eq!(items[0].driver, None);
    assert_eq!(items[1], Scenario::new("b.sh", ScenarioKind::Browser, Some("AC-1"), Some(BrowserDriver::Playwright)));
    assert_eq!(Scenario::new("a.sh", ScenarioKind::Shell, None, Some(BrowserDriver::Playwright)).driver, None);
}

#[test]
fn 추가_수용_기준_경계() {
    // text 가 빈칸·줄바꿈뿐이면 없는 것, 앞뒤 줄바꿈은 다듬는다
    let (cs, ps) = parse_criteria(Some(&json!([{"id": "A", "text": " \n\t"}, {"id": "B", "text": "\n본문\n"}, 7])), 5);
    assert_eq!(cs, vec![Criterion::new("B", "본문")]);
    assert_eq!(ps, vec!["단위 5 의 수용 기준 A 에 text 가 없습니다".to_string(), "단위 5 의 수용 기준 3 에 id(영문·숫자·.-_)가 없습니다".to_string()]);
    // 짝이 모두 있으면 빈 목록, 시나리오가 없으면 모두 빈 칸
    let cs = vec![Criterion::new("A", "a"), Criterion::new("B", "b")];
    assert!(criteria_gaps(&cs, &[sh_crit("x.sh", "A"), sh_crit("y.sh", "B")]).is_empty());
    assert_eq!(criteria_gaps(&cs, &[sh("x.sh")]), cs);
    // 카드 글 전체와 답의 우선순위(시나리오가 이긴다)
    let card = criterion_card("단위", &cs[0]);
    assert_eq!(card.label, "실행 파일이 없는 수용 기준");
    assert_eq!(card.title, "「단위」 의 수용 기준 A 를 볼 판정 시나리오가 없습니다");
    assert_eq!(card.why, "a (이 기준은 실행 파일로 판정하지 않아 통과·실패를 기계로 가리지 못합니다.)");
    assert_eq!(card.options[0].label, "범위에서 빼기");
    assert_eq!(card.options[1].label, "모델 판정으로 두기");
    assert_eq!(CRITERION_DROP, "drop_criterion");
    assert_eq!(CRITERION_MODEL_JUDGE, "model_judge");
    assert_eq!(criterion_answer(&cs[0], &[sh_crit("x.sh", "A")], Some("model_judge")), ("scenario".to_string(), false));
    // 시나리오가 없는 기준이 카드에서 범위에서 빼기를 고르면 그 답 그대로(defaulted 아님)
    assert_eq!(criterion_answer(&cs[1], &[sh("x.sh")], Some("drop_criterion")), ("drop_criterion".to_string(), false));
    assert_eq!(criterion_answer(&cs[1], &[], Some("model_judge")), ("model_judge".to_string(), false));
}

#[test]
fn 추가_json() {
    assert_eq!(scenario_json(&sh("a.sh")), json!({"file": "a.sh", "kind": "shell"}));
    assert_eq!(
        scenario_json(&Scenario::new("b.mjs", ScenarioKind::Browser, Some("AC-1"), Some(BrowserDriver::Playwright))),
        json!({"file": "b.mjs", "kind": "browser", "criterion": "AC-1", "driver": "playwright"})
    );
    assert_eq!(scenario_json(&Scenario::new("b.mjs", ScenarioKind::Browser, None, None))["driver"], "chrome");
    assert_eq!(criterion_json(&Criterion::new("A", "글")), json!({"id": "A", "text": "글"}));
}

#[test]
fn 추가_도구_글() {
    assert_eq!(tool_label(&ToolNeed::Program("/bin/sh".into())), "/bin/sh");
    assert_eq!(tool_label(&ToolNeed::Command("node".into())), "node(PATH)");
    assert_eq!(tool_label(&ToolNeed::Browser), "브라우저(Google Chrome·Chromium·Microsoft Edge)");
    assert_eq!(tool_label(&ToolNeed::NodeModule("playwright".into())), "node 모듈 playwright");
}

fn run_of(file: &str, head: &str) -> (Vec<String>, Vec<ToolNeed>, Option<String>) {
    let r = scenario_run(&sh(file), head);
    (r.interpreter, r.needs, r.problem)
}

#[test]
fn 추가_돌리는_법_경계() {
    // 확장자는 대소문자를 가리지 않고, 점 파일·확장자 없음·끝 점은 확장자 없음
    assert_eq!(run_of("a.SH", "").0, vec!["/bin/sh"]);
    assert_eq!(run_of("d/a.Py", "").0, vec!["/usr/bin/env", "python3"]);
    for f in ["a.bash", "a.zsh", "a.js", "a.cjs", "a.ts", "a.mts", "a.mjs"] {
        assert!(run_of(f, "").2.is_none(), "{f}");
    }
    for f in [".sh", "sh", "a.", "a.sh.bak", "d.sh/x"] {
        assert!(run_of(f, "").2.is_some(), "{f}");
    }
    assert_eq!(run_of("d/a.sh/", "").0, vec!["/bin/sh"]); // NSString.pathExtension 은 끝 `/` 를 뗀다
    // #! 이 확장자보다 먼저, 빈칸·탭으로 가름, 빈 #! 은 절대 경로가 아님
    assert_eq!(run_of("a.py", "#!/bin/sh\n").0, vec!["/bin/sh"]);
    assert_eq!(run_of("a", "#!/usr/bin/env\tnode  --x\n").0, vec!["/usr/bin/env", "node", "--x"]);
    assert_eq!(run_of("a", "#!\n").2.as_deref(), Some("첫 줄 #! 의 해석기가 절대 경로가 아닙니다(#!)"));
    assert_eq!(run_of("a", "#! /bin/sh").0, vec!["/bin/sh"]);
    // env 만 있고 이름이 없으면 도구는 env 하나, -로 시작하는 것만 있어도 같다
    assert_eq!(run_of("a", "#!/usr/bin/env\n").1, vec![ToolNeed::Program("/usr/bin/env".into())]);
    assert_eq!(run_of("a", "#!/usr/bin/env -i\n").1, vec![ToolNeed::Program("/usr/bin/env".into())]);
    // env 가 아닌 해석기의 다음 조각은 이름으로 보지 않는다. 끝 `/` 가 붙은 env 도 env
    assert_eq!(run_of("a", "#!/bin/bash node\n").1, vec![ToolNeed::Program("/bin/bash".into())]);
    assert_eq!(run_of("a", "#!/usr/bin/env/ node\n").1, vec![ToolNeed::Program("/usr/bin/env/".into()), ToolNeed::Command("node".into())]);
    // 오류 글은 첫 줄을 자소 묶음 60개까지
    let long = format!("#!{}\nx", "b".repeat(100));
    let want = format!("첫 줄 #! 의 해석기가 절대 경로가 아닙니다(#!{})", "b".repeat(58));
    assert_eq!(run_of("a", &long).2, Some(want));
    // 브라우저 playwright 는 node 가 이미 있으면 다시 더하지 않는다. 드라이버 없는 브라우저는 chrome
    let pw = Scenario { file: "p.mjs".into(), kind: ScenarioKind::Browser, criterion: None, driver: Some(BrowserDriver::Playwright) };
    assert_eq!(scenario_run(&pw, "#!/usr/bin/env node\n").needs.iter().filter(|n| **n == ToolNeed::Command("node".into())).count(), 1);
    let bare = Scenario { file: "p.sh".into(), kind: ScenarioKind::Browser, criterion: None, driver: None };
    assert_eq!(scenario_run(&bare, "").needs.last(), Some(&ToolNeed::Browser));
    // 돌리는 법을 모르면 도구도 없다(브라우저도 드라이버 도구는 더한다)
    assert_eq!(scenario_run(&bare, "#!sh\n").needs, vec![ToolNeed::Browser]);
}

#[test]
fn 추가_돌리는_법_글() {
    assert_eq!(
        run_of("a.rb", "").2.as_deref(),
        Some("돌리는 법을 모릅니다. 첫 줄에 #! 해석기를 쓰거나 확장자를 .sh·.bash·.zsh·.py·.js·.mjs·.cjs·.ts 로 씁니다")
    );
    assert_eq!(run_of("a.sh", "#!bash\n").2.as_deref(), Some("첫 줄 #! 의 해석기가 절대 경로가 아닙니다(#!bash)"));
}

#[test]
fn 추가_crlf_머리() {
    // Foundation 과 같음: Swift 의 "\n" 은 글자 하나라 "\r\n" 으로 끝나는 첫 줄은 나눠지지 않아 `\r\n` 이 해석기 글자에 섞인다(도구 확인에서 막힘)
    let (argv, needs, problem) = run_of("a", "#!/bin/sh\r\necho hi");
    assert_eq!(argv, vec!["/bin/sh\r\necho", "hi"]);
    assert_eq!(needs[0], ToolNeed::Program("/bin/sh\r\necho".into()));
    assert!(problem.is_none());
    // 홀로 선 `\r` 은 구분자, 홀로 선 `\n` 은 줄 끝
    assert_eq!(run_of("a", "#!/bin/sh\r-e\nfoo").0, vec!["/bin/sh", "-e"]);
    assert!(run_of("a", "\n#!/bin/sh").2.is_some()); // 첫 줄이 빈 줄이면 #! 이 아님 → 확장자(없음)
}
