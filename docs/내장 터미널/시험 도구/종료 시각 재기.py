import os, sys, time, json
# watch.py <출력> <이름=pid> ... — 0.02초마다 kill -0, 사라진 시각(절대)을 남긴다. 최대 40초
out = sys.argv[1]; pids = dict(a.split('=') for a in sys.argv[2:])
gone = {}; t0 = time.time()
while time.time() - t0 < 40 and len(gone) < len(pids):
    for n, p in pids.items():
        if n in gone: continue
        try: os.kill(int(p), 0)
        except ProcessLookupError: gone[n] = time.time()
        except PermissionError: pass
    time.sleep(0.02)
json.dump({'start': t0, 'gone': gone, 'alive': [n for n in pids if n not in gone]}, open(out, 'w'))
