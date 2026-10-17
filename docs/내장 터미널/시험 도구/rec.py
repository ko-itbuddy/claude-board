#!/usr/bin/env python3
"""pty 로 들어오는 바이트를 raw 모드로 읽어 JSON 줄로 기록한다.
사용: rec.py <출력.jsonl> [--bp] [--kitty N] [--seconds S] [--echo] [--clip]
  --bp       bracketed paste(2004) 켜기
  --kitty N  kitty keyboard protocol flags N 켜기 (CSI > N u)
  --echo     받은 바이트를 화면에도 글자+hex 로 보여 줌 (사람이 직접 시험할 때)
  --clip     클립보드 시험용 출력 일정(COPYME 줄, OSC 52 쓰기/읽기)
Ctrl+D(0x04) 단독 입력 또는 --seconds 경과 시 끝난다."""
import os, sys, tty, termios, time, json, select, base64

args = sys.argv[1:]
out = args[0]
bp = "--bp" in args
kitty = int(args[args.index("--kitty") + 1]) if "--kitty" in args else 0
seconds = float(args[args.index("--seconds") + 1]) if "--seconds" in args else 30
echo = "--echo" in args
clip = "--clip" in args

fd = 0
old = termios.tcgetattr(fd)
tty.setraw(fd)
w = lambda s: (os.write(1, s.encode() if isinstance(s, str) else s))
if bp: w("\x1b[?2004h")
if kitty: w(f"\x1b[>{kitty}u")
start = time.time()
log = open(out, "a")
def rec(kind, data):
    log.write(json.dumps({"t": round(time.time(), 4), "kind": kind, "hex": data.hex(" "),
                          "text": data.decode("utf-8", "backslashreplace")}, ensure_ascii=False) + "\n")
    log.flush()
rec("ready", f"bp={bp} kitty={kitty} tty={os.ttyname(0)} pid={os.getpid()}".encode())
w(f"rec ready bp={bp} kitty={kitty} {os.ttyname(0)}\r\n")
sched = [(3, "copyme"), (5, "osc52w"), (7, "osc52r")] if clip else []
try:
    while time.time() - start < seconds:
        now = time.time() - start
        while sched and now >= sched[0][0]:
            _, what = sched.pop(0)
            if what == "copyme": w("COPYME-0123456789\r\n")
            elif what == "osc52w": w("\x1b]52;c;" + base64.b64encode(b"osc52-written").decode() + "\x07")
            elif what == "osc52r": w("\x1b]52;c;?\x07")
            rec("sent-" + what, b"")
        r, _, _ = select.select([fd], [], [], 0.1)
        if not r: continue
        data = os.read(fd, 4096)
        if not data: break
        rec("in", data)
        if echo:
            w("got: " + data.decode("utf-8", "backslashreplace").replace("\x1b", "\\e").replace("\r", "\\r").replace("\n", "\\n")
              + "   hex: " + data.hex(" ") + "\r\n")
        if data == b"\x04": break
finally:
    if kitty: w("\x1b[<u")
    if bp: w("\x1b[?2004l")
    termios.tcsetattr(fd, termios.TCSADRAIN, old)
    rec("exit", b"")
