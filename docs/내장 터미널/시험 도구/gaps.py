#!/usr/bin/env python3
# stream.py 기록에서 구간별 최대 쓰기 간격을 본다: gaps.py <jsonl> [표지 시각 파일]
import json, sys
rows = [json.loads(l) for l in open(sys.argv[1])]
marks = []
if len(sys.argv) > 2:
    for l in open(sys.argv[2]):
        t, name = l.split(" ", 1); marks.append((float(t), name.strip()))
def seg(a, b):
    r = [x for x in rows if a <= x["t0"] < b]
    if len(r) < 2: return None
    g = max(r[k + 1]["t0"] - r[k]["t0"] for k in range(len(r) - 1))
    return len(r), round(g, 3), round(max(x["dt"] for x in r), 3)
print("전체 줄", len(rows), "처음", rows[0]["t0"], "끝", rows[-1]["t0"])
print("전체:", seg(rows[0]["t0"], rows[-1]["t0"] + 1))
for k, (t, name) in enumerate(marks):
    end = marks[k + 1][0] if k + 1 < len(marks) else rows[-1]["t0"] + 1
    print(f"{name}: 줄수·최대 간격(초)·최대 write 시간 =", seg(t, end))
