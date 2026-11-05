#!/usr/bin/env python3
# 장난감 심은 git — 출발 준비·지난 실행 이어 가기의 git 읽기 받침(다듬기 ⑬-1 ④, 규칙 apPrepGitCheck) 재현기. 모델 비용 0, 바깥 요청 0.
# 쓰는 법: python3 -I "장난감 심은 git.py" <tmp 밖 빈 폴더> --temp <임시 자리 폴더(예: 세션 scratchpad 아래)>
#   그 아래 저장소를 만든다. 심은 명령은 <폴더> 에 표지 파일(MARK-<꼴>-<pid>)을 만드는 것만이다(바깥으로 아무것도 보내지 않음).
#   cfg/        저장소 자체 .git/config 의 core.fsmonitor 명령
#   gl/         색인에 gitlink sub(mode 160000) + sub/.git 파일이 작업 트리 안 자리(.cache/m/.git)를 가리키고 그 config 에 fsmonitor(⑫-8 꼴 A).
#               gl 은 하위 모듈로, gl/sub 는 다른 저장소 작업 트리 안이라 막혀야 한다
#   nest/       작업 트리 하위 폴더 app/.git(중첩 저장소, 추적하지 않음)의 config 에 fsmonitor·clean filter(⑫-8 꼴 B). nest/app 은 막히고 nest 는 보통으로 읽힘
#   top/        .git 파일이 --temp 아래 git 폴더를 가리키고 그 config 에 clean filter(보안 검토 고침 1·2)
#   own/        저장소 자체 .git/config 의 clean filter(보안 검토 고침 3). diff.external·textconv·pager·sshCommand 도 심음(보드 git 이 부르지 않아 표지 없음)
#   inc/        .git/config 의 include.path 가 저장소 밖 파일(<폴더>/inc.cfg)을 끌어오고 그 파일에 clean filter·fsmonitor(보안 검토 고침 4)
# 보드가 filter 를 부를 꼴(크기 같고 시각만 다른 추적 파일)로 두려면 출발 준비에 넣기 전에 각 저장소에서 `touch -t 202601010000 README.md .gitattributes`.
# 표지 수는 `ls <폴더>/MARK-*` 로 센다. fsmonitor 스크립트는 표준 입력을 읽지 않는다(cat 이면 git 이 기다리며 멈춤)
import argparse, os, subprocess

ap = argparse.ArgumentParser()
ap.add_argument("root")
ap.add_argument("--temp", required=True)
a = ap.parse_args()
root, temp = os.path.abspath(a.root), os.path.abspath(a.temp)
os.makedirs(root, exist_ok=True)
os.makedirs(temp, exist_ok=True)
env = dict(os.environ, GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null", GIT_AUTHOR_NAME="toy", GIT_AUTHOR_EMAIL="toy@example.com",
           GIT_COMMITTER_NAME="toy", GIT_COMMITTER_EMAIL="toy@example.com")

def git(cwd, *args, out=False):
    r = subprocess.run(["git", "-c", "core.fsmonitor=false", *args], cwd=cwd, env=env, check=True, capture_output=True, text=True)
    return r.stdout.strip() if out else None

def script(name, reads):
    p = os.path.join(root, "plant-%s.sh" % name)
    with open(p, "w") as f:
        f.write('#!/bin/sh\n: > "%s/MARK-%s-$$"\n%s\n' % (root, name, "cat" if reads else "exit 1"))
    os.chmod(p, 0o755)
    return p

def repo(d, attrs=False):
    os.makedirs(d, exist_ok=True)
    git(d, "init", "-q", "-b", "main")
    with open(os.path.join(d, "README.md"), "w") as f: f.write("toy\n")
    if attrs:
        with open(os.path.join(d, ".gitattributes"), "w") as f: f.write("* filter=x\n")
    git(d, "add", "-A")
    git(d, "commit", "-q", "-m", "first")

# cfg
d = os.path.join(root, "cfg"); repo(d)
git(d, "config", "core.fsmonitor", script("cfg", False))

# gl (꼴 A)
d = os.path.join(root, "gl"); repo(d)
m = os.path.join(d, ".cache", "m"); repo(m)
git(m, "config", "core.fsmonitor", script("gl", False))
head = git(m, "rev-parse", "HEAD", out=True)
os.makedirs(os.path.join(d, "sub"), exist_ok=True)
with open(os.path.join(d, "sub", ".git"), "w") as f: f.write("gitdir: ../.cache/m/.git\n")
git(d, "update-index", "--add", "--cacheinfo", "160000,%s,sub" % head)
git(d, "commit", "-q", "-m", "gitlink")

# nest (꼴 B)
d = os.path.join(root, "nest"); repo(d)
n = os.path.join(d, "app"); repo(n, attrs=True)
git(n, "config", "core.fsmonitor", script("nest-fsm", False))
git(n, "config", "filter.x.clean", script("nest-flt", True))

# top: .git 파일 → 임시 자리 git 폴더
g = os.path.join(temp, "planted-sep"); repo(g, attrs=True)
git(g, "config", "filter.x.clean", script("sep", True))
t = os.path.join(root, "top"); os.makedirs(t, exist_ok=True)
with open(os.path.join(t, ".git"), "w") as f: f.write("gitdir: %s/.git\n" % g)
for name, text in (("README.md", "toy\n"), (".gitattributes", "* filter=x\n")):
    with open(os.path.join(t, name), "w") as f: f.write(text)

# own: 저장소 자체 설정의 명령 값 키
d = os.path.join(root, "own"); repo(d, attrs=True)
git(d, "config", "filter.x.clean", script("own-flt", True))
for k, name in (("diff.external", "own-ext"), ("diff.y.textconv", "own-tcv"), ("core.pager", "own-pager"), ("core.sshCommand", "own-ssh")):
    git(d, "config", k, script(name, True))

# inc: include 로 끌어온 저장소 밖 파일
d = os.path.join(root, "inc"); repo(d, attrs=True)
with open(os.path.join(root, "inc.cfg"), "w") as f:
    f.write('[filter "x"]\n\tclean = %s\n[core]\n\tfsmonitor = %s\n' % (script("inc-flt", True), script("inc-fsm", False)))
git(d, "config", "include.path", os.path.join(root, "inc.cfg"))
print(root)
