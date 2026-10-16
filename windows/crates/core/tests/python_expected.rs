//! macOS `notify.py`·`state.py` 가 만든 값과 맞추는 시험.
//! 기대값 `tests/fixtures/python-expected.json` 은 `tests/fixtures/gen-python-expected.py` 로 만든다.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use claude_board_core::{notify, quote, state};
use serde_json::Value;

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn expected() -> Value {
    serde_json::from_str(&fs::read_to_string(fixtures().join("python-expected.json")).unwrap()).unwrap()
}

#[test]
fn plain_and_first_sentence_match_notify_py() {
    let e = expected();
    let cases = e["text"].as_array().unwrap();
    assert!(cases.len() >= 15);
    for c in cases {
        let text = c["text"].as_str().unwrap();
        assert_eq!(notify::plain(text), c["plain"].as_str().unwrap(), "plain({text:?})");
        assert_eq!(notify::first_sentence(text), c["first_sentence"].as_str().unwrap(), "first_sentence({text:?})");
    }
}

#[test]
fn last_assistant_text_matches_notify_py() {
    let e = expected();
    let got = notify::last_assistant_text(&fixtures().join("transcript-sample.jsonl"));
    assert_eq!(got, e["last_assistant_text"].as_str().unwrap());
    assert_eq!(notify::last_assistant_text(&fixtures().join("no-such-file.jsonl")), "");
}

#[test]
fn last_assistant_text_reads_only_the_tail() {
    // 맨 앞의 assistant 줄은 끝 400KB 밖이라 보이지 않는다(notify.py 와 같다)
    let path = std::env::temp_dir().join(format!("cb-tail-{}.jsonl", std::process::id()));
    let mut f = fs::File::create(&path).unwrap();
    writeln!(f, r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"너무 오래된 답"}}]}}}}"#).unwrap();
    let filler = format!(r#"{{"type":"user","message":{{"content":"{}"}}}}"#, "가".repeat(1000));
    for _ in 0..200 {
        writeln!(f, "{filler}").unwrap();
    }
    drop(f);
    assert_eq!(notify::last_assistant_text(&path), "");
    fs::remove_file(&path).unwrap();
}

#[test]
fn carry_args_and_resume_cmd_match_state_py() {
    let e = expected();
    for c in e["resume"].as_array().unwrap() {
        let argv: Vec<String> = serde_json::from_value(c["argv"].clone()).unwrap();
        let carry = state::carry_args(&argv);
        assert_eq!(serde_json::to_value(&carry).unwrap(), c["carry"], "carry_args({argv:?})");
        let mut rec = c["record"].as_object().unwrap().clone();
        rec.insert("session_id".into(), "11111111-2222-3333-4444-555555555555".into());
        rec.insert("args".into(), serde_json::to_value(&carry).unwrap());
        assert_eq!(state::resume_cmd_with(&rec, quote::join_posix), c["resume_posix"].as_str().unwrap());
    }
}

#[test]
fn resume_cmd_windows_quoting() {
    let e = expected();
    let want = [
        "claude --allow-dangerously-skip-permissions --permission-mode default --model claude-opus-5-5 --resume 11111111-2222-3333-4444-555555555555",
        r#"claude --add-dir "my dir" 그곳 --settings "{\"a\": 1}" --permission-mode acceptEdits --model claude-opus-4[1m] --resume 11111111-2222-3333-4444-555555555555"#,
        r#"claude --mcp-config a.json b's.json --verbose "--plugin-dir=/p q" --resume 11111111-2222-3333-4444-555555555555"#,
        "claude --model sonnet --resume 11111111-2222-3333-4444-555555555555",
        r#"claude --agent "" --permission-mode bypassPermissions --resume 11111111-2222-3333-4444-555555555555"#,
    ];
    for (c, want) in e["resume"].as_array().unwrap().iter().zip(want) {
        let mut rec = c["record"].as_object().unwrap().clone();
        rec.insert("session_id".into(), "11111111-2222-3333-4444-555555555555".into());
        rec.insert("args".into(), c["carry"].clone());
        assert_eq!(state::resume_cmd_with(&rec, quote::join_windows), want);
    }
}
