import os, subprocess, sys
# 자식에서 제어 터미널을 떼고(setsid) claude -p 를 띄운다 — 부모 python 은 기다리며 남아 계보(탭 zsh 아래)를 잇는다
pid = os.fork()
if pid == 0:
    os.setsid()
    subprocess.run(["claude", "-p", "--session-id", sys.argv[1], "--dangerously-skip-permissions", "Use the Bash tool to run: sleep 30. Then reply ok."],
                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    os._exit(0)
os.waitpid(pid, 0)
