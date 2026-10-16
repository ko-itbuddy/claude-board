#!/usr/bin/env python3
"""전역 ~/.claude/settings.json 에 Claude Board 알림·상태 hook 을 넣거나 뺀다.

사용법: python3 hooks.py install | uninstall
- 다른 hook·설정은 건드리지 않는다. 명령에 .claude/notifier/ 가 들어간 hook 만 이 스크립트의 것으로 본다.
- 바꾸기 전에 settings.json.bak-claude-board-<시각> 으로 백업한다.
"""
import json
import os
import shutil
import sys
import time

PATH = os.path.expanduser("~/.claude/settings.json")
MARK = ".claude/notifier/"
ENV_KEY = "CLAUDE_CODE_DISABLE_TERMINAL_TITLE"


def cmd(script, arg):
    return {"type": "command", "command": f'python3 "$HOME/.claude/notifier/{script}" {arg}', "timeout": 5}


# (이벤트, matcher, hook 목록) — 도구 이벤트만 matcher "*" 를 쓴다
HOOKS = [
    ("SessionStart", None, [cmd("state.py", "idle")]),
    ("UserPromptSubmit", None, [cmd("state.py", "working")]),
    ("PreToolUse", "*", [cmd("state.py", "working")]),
    ("PostToolUse", "*", [cmd("state.py", "working")]),
    ("PermissionRequest", "*", [cmd("state.py", "waiting")]),
    ("Notification", None, [cmd("notify.py", "notification"), cmd("state.py", "waiting")]),
    ("Stop", None, [cmd("notify.py", "stop"), cmd("state.py", "idle")]),
    ("StopFailure", None, [cmd("state.py", "idle")]),
    ("SessionEnd", None, [cmd("state.py", "ended")]),
    ("SubagentStart", None, [cmd("state.py", "subagent")]),
    ("SubagentStop", None, [cmd("state.py", "subagent")]),
]


def strip(settings):
    """이 스크립트가 넣은 hook 을 모두 빼고, 비게 된 묶음·이벤트도 지운다."""
    hooks = settings.get("hooks", {})
    for event in list(hooks):
        groups = []
        for g in hooks[event]:
            kept = [h for h in g.get("hooks", []) if MARK not in h.get("command", "")]
            if kept:
                groups.append({**g, "hooks": kept})
        if groups:
            hooks[event] = groups
        else:
            del hooks[event]
    if not hooks:
        settings.pop("hooks", None)


def main():
    action = sys.argv[1] if len(sys.argv) > 1 else ""
    if action not in ("install", "uninstall"):
        sys.exit("사용법: hooks.py install | uninstall")
    settings = {}
    if os.path.exists(PATH):
        with open(PATH) as f:
            text = f.read()
        if text.strip():
            try:
                settings = json.loads(text)
            except ValueError as e:
                sys.exit(f"settings.json 을 읽지 못했습니다(JSON 오류: {e}). 파일을 고친 뒤 다시 실행하세요.")
        backup = f"{PATH}.bak-claude-board-{time.strftime('%Y%m%d-%H%M%S')}"
        shutil.copy2(PATH, backup)
        print(f"  백업: {backup}")
    else:
        os.makedirs(os.path.dirname(PATH), exist_ok=True)

    strip(settings)
    if action == "install":
        hooks = settings.setdefault("hooks", {})
        for event, matcher, items in HOOKS:
            group = {"hooks": items}
            if matcher:
                group = {"matcher": matcher, **group}
            hooks.setdefault(event, []).append(group)
        # 탭 제목은 state.py 가 상태 아이콘과 함께 쓴다
        settings.setdefault("env", {})[ENV_KEY] = "1"
    else:
        env = settings.get("env", {})
        env.pop(ENV_KEY, None)
        if not env:
            settings.pop("env", None)

    tmp = PATH + ".tmp"
    with open(tmp, "w") as f:
        json.dump(settings, f, ensure_ascii=False, indent=2)
        f.write("\n")
    os.replace(tmp, PATH)
    print(f"  {'등록' if action == 'install' else '제거'}: {PATH}")


if __name__ == "__main__":
    main()
