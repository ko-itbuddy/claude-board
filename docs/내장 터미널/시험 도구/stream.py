#!/usr/bin/env python3
# 숨김 시험용 출력 발생기 — 초당 약 40 KB, 50줄마다 제목 변경(OSC 2). 줄마다 쓰기 시작 시각과 걸린 시간을 jsonl 로 남긴다
import os, sys, time, json
out, secs = sys.argv[1], float(sys.argv[2])
log = open(out, "w")
start, i, body = time.time(), 0, "x" * 100
while time.time() - start < secs:
    t0 = time.time()
    s = f"{i:06d} {body}\r\n"
    if i % 50 == 0:
        s = f"\x1b]2;stream {i}\x07" + s
    os.write(1, s.encode())
    log.write(json.dumps({"i": i, "t0": round(t0, 4), "dt": round(time.time() - t0, 4)}) + "\n")
    i += 1
    time.sleep(0.0025)
log.close()
os.write(1, f"\r\nSTREAM-DONE {i}\r\n".encode())
