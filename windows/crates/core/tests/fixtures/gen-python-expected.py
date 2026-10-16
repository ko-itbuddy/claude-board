# notify.py·state.py 의 함수로 python-expected.json·transcript-sample.jsonl 을 만든다
# 실행(windows/ 에서): python3 -I crates/core/tests/fixtures/gen-python-expected.py .. crates/core/tests/fixtures
import json, sys
repo, out = sys.argv[1], sys.argv[2]
sys.dont_write_bytecode = True  # 저장소에 __pycache__ 를 남기지 않는다
sys.path.insert(0, repo)
import notify, state

texts = [
    "작업을 마쳤습니다. 다음 단계는\n테스트입니다.",
    "## 요약\n\n```rust\nfn main() {}\n```\n- **굵게** 항목 `code` 입니다\n1. 둘째 줄!",
    "가" * 50 + " " + "나" * 60 + " " + "다" * 50,
    "| 표 | 머리 |\n|---|---|\n| a | b |",
    "First sentence?Second. Third",
    "끝。다음 문장",
    "",
    "   \n\t  ",
    "v1.2.3 버전을 올렸습니다. 확인해 주세요.",
    "Unicode\x1cseparator. 다음",
    "**bold * star** and `a` `b` ```inline block``` end.",
    "  # 제목\n   * 항목 하나\n  12. 번호 있는 항목. 끝",
    "가나다라마바사아자차 " * 14 + "끝.",
    "Wait... what? ok",
    "a\n-b\n*c\n#d\n|e",
    "줄 바꿈 없는　공백들!다음",
    "```\n코드만\n```",
    "**열린 굵게 `열린 코드",
]
cases = [{"text": t, "plain": notify.plain(t), "first_sentence": notify.first_sentence(t)} for t in texts]

# transcript — 맨 끝 assistant 는 공백뿐인 text 와 진짜 text 두 개, 그 뒤 깨진 줄과 user 줄
lines = [
    {"type": "user", "message": {"role": "user", "content": "안녕"}},
    {"type": "assistant", "message": {"content": [{"type": "text", "text": "이전 답"}]}},
    {"type": "assistant", "message": {"content": [{"type": "text", "text": "  \n"}, {"type": "text", "text": "첫째 줄"}, {"type": "tool_use", "name": "Bash"}, {"type": "text", "text": "둘째 **줄**."}]}},
    {"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Read"}]}},
    {"type": "assistant", "message": {"model": "x", "content": "문자열 content"}},
    {"type": "user", "message": {"content": [{"type": "tool_result", "content": "ok"}]}},
]
transcript = "\n".join(json.dumps(l, ensure_ascii=False) for l in lines) + "\n{broken json\n"
tpath = out + "/transcript-sample.jsonl"
open(tpath, "w").write(transcript)
last = notify.last_assistant_text(tpath)

argvs = [
    ["/x/claude", "--allow-dangerously-skip-permissions"],
    ["claude", "--model", "opus", "--permission-mode=plan", "--add-dir", "my dir", "그곳", "-c", "--settings", "{\"a\": 1}"],
    ["claude", "--mcp-config", "a.json", "b's.json", "--verbose", "--print", "--plugin-dir=/p q"],
    ["claude", "--model"],
    ["claude", "--agent", ""],
]
records = [
    {"permission_mode": "default", "model": "claude-opus-5-5"},
    {"permission_mode": "acceptEdits", "model": "claude-opus-4[1m]"},
    {},
    {"model": "sonnet"},
    {"permission_mode": "bypassPermissions"},
]
resume = []
for argv, extra in zip(argvs, records):
    rec = dict(extra, session_id="11111111-2222-3333-4444-555555555555", args=state.carry_args(argv))
    resume.append({"argv": argv, "record": extra, "carry": rec["args"], "resume_posix": state.resume_cmd(rec)})

json.dump({"source": "저장소 최상위 notify.py·state.py (커밋 68ffed2) 로 만든 기대값 — gen-python-expected.py",
           "text": cases, "last_assistant_text": last, "resume": resume},
          open(out + "/python-expected.json", "w"), ensure_ascii=False, indent=2)
print(last)
