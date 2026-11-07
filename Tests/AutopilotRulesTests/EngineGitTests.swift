// 엔진 git 보호(Sources/AutopilotRules/EngineGit.swift) 시험 — 다듬기 ⑬-2 ⑥.
// 옮기기: 엔진 main.swift 의 gitHardenEnv·gitHardenC 를 규칙으로 옮긴 값이 옮기기 전 글자와 같은지(덤프 대조, 고치기 뒤에는 앞부분이 그대로인지).
// 고치기: 늦은 받기·전송 끄기·서명 확인 끄기를 더했고, 물려받은 GIT_* 를 뺀 프로세스 환경(⑬-1 보드 출발 준비 읽기와 같은 규칙)
import Foundation
import Testing
import AutopilotRules

@Suite("자율주행 엔진 git 보호") struct EngineGitTests {
    @Test("옮기기: 엔진 gitHardenEnv·gitHardenC 와 같은 글자(41927b1 의 main.swift 317·321줄), 고치기는 뒤에 더하기만")
    func 옮기기() {
        #expect(Array(apEngineGitHardenEnv.prefix(4)) == ["GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null", "GIT_ATTR_NOSYSTEM=1", "GIT_NO_REPLACE_OBJECTS=1"])
        #expect(Array(apEngineGitHardenC.prefix(12)) == ["-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "-c", "core.attributesFile=/dev/null",
                                                        "-c", "safe.bareRepository=explicit", "-c", "diff.ignoreSubmodules=all"])
    }

    @Test("고치기: 늦은 받기·전송 프로토콜·서명 확인을 끈다(⑬-1 보안 검토 고침 6 의 엔진 몫)")
    func 늦은받기() {
        #expect(apEngineGitHardenEnv.contains("GIT_NO_LAZY_FETCH=1"))
        #expect(apEngineGitHardenC.suffix(4) == ["-c", "protocol.allow=never", "-c", "log.showSignature=false"])
    }

    @Test("고치기: 물려받은 GIT_* 는 모두 빠지고 보호 값만 남는다(⑬-1 보안 검토 고침 7 의 엔진 몫)", arguments: [
        "GIT_CONFIG_PARAMETERS", "GIT_CONFIG_COUNT", "GIT_CONFIG_KEY_0", "GIT_CONFIG_VALUE_0", "GIT_SSH", "GIT_SSH_COMMAND", "GIT_ALLOW_PROTOCOL",
        "GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY", "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_EXEC_PATH", "GIT_ASKPASS", "GIT_PROTOCOL_FROM_USER",
    ])
    func 물려받음(_ k: String) {
        let e = apEngineGitEnv(inherited: ["PATH": "/usr/bin:/bin", "HOME": "/Users/toy", k: "planted", "GIT_CONFIG_GLOBAL": "/w/planted.cfg"])
        #expect(e[k] == nil)
        #expect(e["PATH"] == "/usr/bin:/bin" && e["HOME"] == "/Users/toy")
        #expect(e["GIT_CONFIG_GLOBAL"] == "/dev/null" && e["GIT_CONFIG_NOSYSTEM"] == "1" && e["GIT_NO_LAZY_FETCH"] == "1" && e["GIT_NO_REPLACE_OBJECTS"] == "1")
        #expect(Set(e.keys.filter { $0.hasPrefix("GIT_") }) == ["GIT_CONFIG_NOSYSTEM", "GIT_CONFIG_GLOBAL", "GIT_ATTR_NOSYSTEM", "GIT_NO_REPLACE_OBJECTS", "GIT_NO_LAZY_FETCH"])
        let id = apEngineGitIdentityEnv(inherited: ["HOME": "/Users/toy", k: "planted"])
        #expect(id == ["HOME": "/Users/toy"])
    }
}
