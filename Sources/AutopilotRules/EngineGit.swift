// 자율주행 규칙(AutopilotRules) — 엔진이 샌드박스 밖에서 부르는 git 의 보호(다듬기 ⑬-2 ⑥ 에서 엔진 main.swift 의 gitHardenEnv·gitHardenC 를 그대로 옮김, 옮기기 커밋).
// 보안 검토 2026-10-22 둘째: 저장소 설정의 fsmonitor·hooks·attributesFile·gpgsign 을 쓰지 않고, 시스템·전역 설정과 전역 attributes·replace 객체도 읽지 않는다 —
// 모델이 저장소 설정·attributes·replace 에 명령을 심어도 밖의 git 이 실행하지 않게.
// safe.bareRepository=explicit(2026-10-30 5-4 보안): 작업 폴더가 bare 저장소 꼴(HEAD·objects·refs)이어도 git 이 스스로 그것을 저장소로 쓰지 않게.
// diff.ignoreSubmodules=all(2026-10-30 ⑦ 보안): 모델이 작업 폴더에 심은 중첩 git(gitlink)으로 status·diff·switch 가 들어가 그 저장소의 filter 를
// 돌리지 않게(Gitlinks.swift). 자율주행은 git 하위 모듈을 쓰지 않아 통째로 무시해도 된다. add·stash 는 경로를 빼는 pathspec 으로 따로 막는다
import Foundation

public let apEngineGitHardenEnv = ["GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null", "GIT_ATTR_NOSYSTEM=1", "GIT_NO_REPLACE_OBJECTS=1"]
public let apEngineGitHardenC = ["-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "-c", "core.attributesFile=/dev/null",
                                 "-c", "safe.bareRepository=explicit", "-c", "diff.ignoreSubmodules=all"]
