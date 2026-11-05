// 자율주행 규칙(AutopilotRules) — 엔진이 샌드박스 밖에서 부르는 git 의 보호(다듬기 ⑬-2 ⑥ 에서 엔진 main.swift 의 gitHardenEnv·gitHardenC 를 그대로 옮김, 옮기기 커밋).
// 보안 검토 2026-10-22 둘째: 저장소 설정의 fsmonitor·hooks·attributesFile·gpgsign 을 쓰지 않고, 시스템·전역 설정과 전역 attributes·replace 객체도 읽지 않는다 —
// 모델이 저장소 설정·attributes·replace 에 명령을 심어도 밖의 git 이 실행하지 않게.
// safe.bareRepository=explicit(2026-10-30 5-4 보안): 작업 폴더가 bare 저장소 꼴(HEAD·objects·refs)이어도 git 이 스스로 그것을 저장소로 쓰지 않게.
// diff.ignoreSubmodules=all(2026-10-30 ⑦ 보안): 모델이 작업 폴더에 심은 중첩 git(gitlink)으로 status·diff·switch 가 들어가 그 저장소의 filter 를
// 돌리지 않게(Gitlinks.swift). 자율주행은 git 하위 모듈을 쓰지 않아 통째로 무시해도 된다. add·stash 는 경로를 빼는 pathspec 으로 따로 막는다.
// 다듬기 ⑬-2 ⑥(⑬-1 보드 출발 준비 읽기와 맞춤, 보안 검토 고침 6·7 의 엔진 몫):
//  · 늦은 받기·전송 끄기 — GIT_NO_LAZY_FETCH=1(부분 복제 저장소에서 빠진 객체를 받으려고 원격 명령 remote.*.uploadpack·core.sshCommand 를 부르지 않게)과
//    -c protocol.allow=never(모든 전송 프로토콜, 명령줄 무게라 저장소 설정이 덮지 못함). 엔진은 원격을 쓰는 git(fetch·push·clone·ls-remote·submodule)을
//    부르지 않는다(Spring 뼈대 받기는 git 이 아닌 curl). 부분 복제 저장소에서 빠진 객체가 필요한 걸음(가지 바꾸기 등)은 실패한다(닫힌 쪽)
//  · log.showSignature=false — git log 가 저장소 설정의 gpg.program 을 부르지 않게(되돌림 훑기, UnitCommits.swift)
//  · 물려받은 GIT_*(GIT_CONFIG_PARAMETERS·GIT_CONFIG_COUNT/KEY/VALUE·GIT_SSH*·GIT_ALLOW_PROTOCOL·GIT_DIR·GIT_INDEX_FILE 등)는 모두 빼고 박은 값만(apEngineGitEnv)
import Foundation

public let apEngineGitHardenEnv = ["GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null", "GIT_ATTR_NOSYSTEM=1", "GIT_NO_REPLACE_OBJECTS=1", "GIT_NO_LAZY_FETCH=1"]
public let apEngineGitHardenC = ["-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "-c", "core.attributesFile=/dev/null",
                                 "-c", "safe.bareRepository=explicit", "-c", "diff.ignoreSubmodules=all", "-c", "protocol.allow=never", "-c", "log.showSignature=false"]

// 엔진 git 의 프로세스 환경: 물려받은 환경에서 GIT_* 를 모두 빼고(⑬-1 의 apPrepGitEnv 와 같은 규칙) 보호 값을 넣는다.
// 엔진은 이 위에 명령마다 박는 값(GIT_DIR·GIT_WORK_TREE 등, "이름=값")을 /usr/bin/env 로 더한다
public func apEngineGitEnv(inherited: [String: String]) -> [String: String] { apPrepGitEnv(inherited: inherited, extra: apEngineGitHardenEnv) }
// 사용자 git 신원 읽기(git config --get user.name·user.email, 아무것도 실행하지 않음)는 전역 설정을 읽어야 하므로 물려받은 GIT_* 만 뺀다
public func apEngineGitIdentityEnv(inherited: [String: String]) -> [String: String] { inherited.filter { !$0.key.hasPrefix("GIT_") } }
