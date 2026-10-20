#!/usr/bin/env python3
"""Claude Code 세션 보드 — 작업 경로별 카드로 세션을 묶어 상태를 실시간으로 보여 준다.

사용법: claude-board   (Ghostty 분할 창에 띄워 두는 용도, q 로 종료)
상태 파일은 state.py 가 hook 에서 기록하는 ~/.claude/notifier/sessions/*.json 이다.
"""
import datetime
import json
import re
import os
import select
import shutil
import sys
import termios
import time
import tty
import unicodedata

SESSIONS = os.path.join(os.path.dirname(os.path.realpath(__file__)), "sessions")
HOME = os.path.expanduser("~")
FRAME = 0.12   # 화면을 다시 그리는 간격(애니메이션)
RELOAD = 1.0   # 상태 파일을 다시 읽는 간격

ORDER = {"waiting": 0, "working": 1, "idle": 2, "ended": 3}
NAME = {"working": "작업 중", "waiting": "승인 대기", "idle": "대기", "ended": "종료"}
# 보드 색 테마 — t 키로 바꾸고, 고른 테마는 board-theme 파일에 남긴다
THEMES = {
    "Catppuccin Mocha": {   # Ghostty 테마와 같은 색
        "working": (137, 180, 250), "waiting": (250, 179, 135), "idle": (166, 227, 161),
        "ended": (108, 112, 134), "accent": (203, 166, 247), "text": (205, 214, 244),
        "muted": (166, 173, 200), "faint": (88, 91, 112), "base": (17, 17, 27),
    },
    "Tokyo Night": {
        "working": (122, 162, 247), "waiting": (255, 158, 100), "idle": (158, 206, 106),
        "ended": (86, 95, 137), "accent": (187, 154, 247), "text": (192, 202, 245),
        "muted": (154, 165, 206), "faint": (65, 72, 104), "base": (26, 27, 38),
    },
    "Rosé Pine": {
        "working": (196, 167, 231), "waiting": (246, 193, 119), "idle": (156, 207, 216),
        "ended": (110, 106, 134), "accent": (235, 188, 186), "text": (224, 222, 244),
        "muted": (144, 140, 170), "faint": (82, 79, 103), "base": (25, 23, 36),
    },
    "Nord": {
        "working": (136, 192, 208), "waiting": (235, 203, 139), "idle": (163, 190, 140),
        "ended": (97, 110, 136), "accent": (180, 142, 173), "text": (236, 239, 244),
        "muted": (155, 165, 185), "faint": (67, 76, 94), "base": (46, 52, 64),
    },
}
THEME_FILE = os.path.join(os.path.dirname(os.path.realpath(__file__)), "board-theme")


def load_theme():
    try:
        with open(THEME_FILE) as f:
            name = f.read().strip()
    except OSError:
        name = ""
    return name if name in THEMES else "Catppuccin Mocha"


theme = load_theme()
RGB = THEMES[theme]
SPINNER = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
RESET, BOLD = "\033[0m", "\033[1m"


def fg(key):
    r, g, b = RGB[key]
    return f"\033[38;2;{r};{g};{b}m"


def bg(key):
    r, g, b = RGB[key]
    return f"\033[48;2;{r};{g};{b}m"


def dark():
    r, g, b = RGB["base"]
    return f"\033[38;2;{r};{g};{b}m"


def width(s):
    return sum(2 if unicodedata.east_asian_width(c) in "WF" else 0 if unicodedata.combining(c) else 1 for c in s)


def fit(s, w):
    """표시 폭 w 에 맞춰 자르거나 공백으로 채운다(한글·이모지는 2칸)."""
    if w <= 0:
        return ""
    if width(s) <= w:
        return s + " " * (w - width(s))
    out, used = "", 0
    for c in s:
        if used + width(c) > w - 1:
            break
        out, used = out + c, used + width(c)
    return out + "…" + " " * (w - used - 1)


def ago(sec):
    sec = int(max(0, sec))
    if sec < 60:
        return f"{sec}초"
    if sec < 3600:
        return f"{sec // 60}분 {sec % 60:02d}초"
    return f"{sec // 3600}시간 {sec % 3600 // 60:02d}분"


def alive(pid):
    if not pid:
        return True
    try:
        os.kill(pid, 0)
        return True
    except ProcessLookupError:
        return False
    except PermissionError:
        return True


def interrupted_at(transcript):
    """Esc 로 끊긴 세션이면 끊은 시각(epoch 초), 아니면 None.
    Claude Code 는 사용자가 응답을 끊으면 Stop hook 을 부르지 않아 세션 파일이 작업 중·승인 대기로 남는다.
    대화 기록 끝의 마지막 대화 줄이 중단 표시([Request interrupted by user], … for tool use])인지 본다 — 보드(InterruptReader)와 같은 규칙."""
    try:
        with open(transcript, "rb") as f:
            f.seek(0, os.SEEK_END)
            f.seek(max(0, f.tell() - 256_000))
            data = f.read()
    except (OSError, TypeError):
        return None
    if b"[Request interrupted by user" not in data:
        return None
    # 중단 뒤에는 cost-state·last-prompt 같은 대화 아닌 줄이 붙고, ! 명령·슬래시 명령 출력은 < 로 시작하는 사용자 줄로 남는다
    for line in reversed(data.splitlines()):
        try:
            entry = json.loads(line)
        except ValueError:
            continue
        kind = entry.get("type") if isinstance(entry, dict) else None
        if kind not in ("user", "assistant") or entry.get("isSidechain") or entry.get("isMeta"):
            continue
        content = (entry.get("message") or {}).get("content")
        if isinstance(content, list):
            content = content[0].get("text") if content and isinstance(content[0], dict) else None
        text = content if isinstance(content, str) else ""
        if kind == "user" and text.startswith("<"):
            continue
        if kind == "user" and text.startswith("[Request interrupted by user"):
            try:
                return datetime.datetime.fromisoformat(entry["timestamp"].replace("Z", "+00:00")).timestamp()
            except (KeyError, AttributeError, ValueError):
                return os.path.getmtime(transcript)
        return None
    return None


def load():
    now = time.time()
    sessions = []
    try:
        names = os.listdir(SESSIONS)
    except OSError:
        return []
    for name in names:
        if not name.endswith(".json"):
            continue
        try:
            with open(os.path.join(SESSIONS, name)) as f:
                s = json.load(f)
        except (OSError, ValueError):
            continue
        if s.get("state") != "ended" and not alive(s.get("pid")):
            s["state"] = "ended"
        # Esc 로 끊긴 세션은 입력을 기다리는 중이다 — 대기 시간은 끊은 때부터 센다
        if s.get("state") in ("working", "waiting"):
            at = interrupted_at(s.get("transcript_path"))
            if at is not None:
                s.update(state="idle", since=at)
        if s.get("state") == "ended" and now - s.get("updated", 0) > 1800:
            try:
                os.remove(os.path.join(SESSIONS, name))
            except OSError:
                pass
            continue
        sessions.append(s)
    return sessions


def indicator(state, tick):
    """상태별 움직이는 표시 — 작업 중은 회전, 승인 대기는 깜빡임."""
    if state == "working":
        return fg("working") + SPINNER[tick % len(SPINNER)]
    if state == "waiting":
        return fg("waiting") + ("◉" if (tick // 4) % 2 == 0 else "○")
    if state == "idle":
        return fg("idle") + "✓"
    return fg("ended") + "✕"


def pill(state):
    label = f" {NAME.get(state, state)} "
    return f"{bg(state)}{dark()}{BOLD}{fit(label, 11)}{RESET}"


def header(sessions, cols):
    lines = []
    title = f"  {fg('accent')}{BOLD}✳ Claude Code{RESET}  {fg('muted')}세션 보드{RESET}"
    clock = f"{fg('muted')}{time.strftime('%H:%M:%S')}{RESET}  "
    lines.append(title + " " * max(1, cols - width("  ✳ Claude Code  세션 보드") - 11) + clock)

    parts = []
    for k in ("waiting", "working", "idle", "ended"):
        n = sum(1 for s in sessions if s.get("state") == k)
        if n:
            parts.append(f"{bg(k)}{dark()}{BOLD} {NAME[k]} {n} {RESET}")
    total = sum(1 for s in sessions if s.get("state") != "ended")
    lines.append(f"  {fg('muted')}활성 {total}개{RESET}   " + "  ".join(parts))
    lines.append("")
    return lines


def card(cwd, members, cols, now, tick):
    inner = cols - 6                       # " │ " + 내용 + " │" 에 마지막 한 칸을 비운다
    urgent = min(members, key=lambda s: ORDER.get(s.get("state"), 9)).get("state", "idle")
    edge = fg(urgent)
    name = os.path.basename(cwd) or cwd
    path = cwd.replace(HOME, "~", 1)

    # 윗변: 폴더 이름(왼쪽)과 경로(오른쪽)를 테두리 안에 새긴다
    left = f" 📁 {name} "
    right = f" {path} "
    room = cols - 2 - width(left) - 2
    if width(right) > room - 2:
        right = " " + fit(path, max(0, room - 4)).rstrip() + " "
    dash = max(1, cols - 6 - width(left) - width(right))
    top = (f" {edge}╭─{RESET}{BOLD}{fg('text')}{left}{RESET}{edge}{'─' * dash}{RESET}"
           f"{fg('muted')}{right}{RESET}{edge}─╮{RESET}")
    lines = [top]

    for s in members:
        st = s.get("state", "idle")
        elapsed = ago(now - s.get("since", now))
        when = (time.strftime("%H:%M", time.localtime(s.get("updated", now))) + " 종료" if st == "ended"
                else f"{elapsed} 전" if st == "idle" else elapsed)
        topic = re.sub(r"[#*`]+", "", s.get("topic") or s.get("last_message") or "요청 전").strip()
        tag = "백그라운드" if s.get("headless") else (s.get("tty") or "")
        topic_w = max(8, inner - (1 + 2 + 11 + 2 + 12 + 2) - (width(tag) + 2))
        text_color = fg("muted") if st == "ended" else fg("text")
        row = (f"{indicator(st, tick)}{RESET}  {pill(st)}  {fg('muted')}{fit(when, 12)}{RESET}  "
               f"{text_color}{fit(topic, topic_w)}{RESET}  {fg('faint')}{tag}{RESET}")
        lines.append(f" {edge}│{RESET} {row} {edge}│{RESET}")
        if st == "waiting" and s.get("last_message"):
            msg = "↳ " + s["last_message"]
            lines.append(f" {edge}│{RESET}    {fg('waiting')}{fit(msg, inner - 3)}{RESET} {edge}│{RESET}")

    # 아랫변: 이 그룹의 상태별 세션 수
    counts = [(k, sum(1 for s in members if s.get("state") == k)) for k in ("waiting", "working", "idle", "ended")]
    badge_plain = "  ".join(f"● {n}" for k, n in counts if n)
    badge = "  ".join(f"{fg(k)}● {n}{RESET}" for k, n in counts if n)
    dash = max(1, cols - 2 - 2 - width(badge_plain) - 2 - 1)
    lines.append(f" {edge}╰{'─' * dash}{RESET} {badge} {edge}─╯{RESET}")
    return lines


def render(sessions, tick):
    cols, rows = shutil.get_terminal_size((100, 30))
    cols = max(cols, 60)
    now = time.time()

    groups = {}
    for s in sessions:
        groups.setdefault(s.get("cwd", "?"), []).append(s)
    for g in groups.values():
        g.sort(key=lambda s: (ORDER.get(s.get("state"), 9), -s.get("since", 0)))
    ordered = sorted(groups.items(), key=lambda kv: (min(ORDER.get(s.get("state"), 9) for s in kv[1]),
                                                     -max(s.get("updated", 0) for s in kv[1])))

    lines = header(sessions, cols)
    if not ordered:
        lines += ["", "", f"{' ' * max(0, cols // 2 - 18)}{fg('muted')}아직 기록된 세션이 없습니다{RESET}",
                  f"{' ' * max(0, cols // 2 - 22)}{fg('faint')}Claude Code 세션에서 요청을 보내면 나타납니다{RESET}"]
    for cwd, members in ordered:
        lines += card(cwd, members, cols, now, tick)
        lines.append("")

    footer = (f"  {bg('faint')}{fg('text')}{BOLD} q {RESET}{fg('faint')} 종료   "
              f"{fg('working')}⠹{fg('faint')} 작업 중   {fg('waiting')}◉{fg('faint')} 승인 대기   "
              f"{fg('idle')}✓{fg('faint')} 대기   {fg('ended')}✕{fg('faint')} 종료   "
              f"{bg('faint')}{fg('text')}{BOLD} t {RESET}{fg('faint')} 테마: {fg('accent')}{theme}{RESET}")
    lines = lines[:rows - 1]
    lines += [""] * (rows - 1 - len(lines))
    lines.append(footer)
    out = "\033[?2026h\033[H" + "\n".join(l + "\033[K" for l in lines) + "\033[J\033[?2026l"
    sys.stdout.write(out)
    sys.stdout.flush()


def next_theme():
    global theme, RGB
    names = list(THEMES)
    theme = names[(names.index(theme) + 1) % len(names)]
    RGB = THEMES[theme]
    try:
        with open(THEME_FILE, "w") as f:
            f.write(theme)
    except OSError:
        pass


def main():
    fd = sys.stdin.fileno()
    old = termios.tcgetattr(fd)
    sys.stdout.write("\033]2;📋 Claude 세션 보드\007\033[?1049h\033[?25l")
    sessions, loaded, tick = [], 0.0, 0
    try:
        tty.setcbreak(fd)
        while True:
            if time.time() - loaded >= RELOAD:
                sessions, loaded = load(), time.time()
            render(sessions, tick)
            tick += 1
            ready, _, _ = select.select([sys.stdin], [], [], FRAME)
            key = sys.stdin.read(1).lower() if ready else ""
            if key == "q":
                break
            if key == "t":
                next_theme()
    except KeyboardInterrupt:
        pass
    finally:
        termios.tcsetattr(fd, termios.TCSADRAIN, old)
        sys.stdout.write("\033[?25h\033[?1049l")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
