// 엔진 git 보호(Sources/AutopilotRules/EngineGit.swift) 시험 — 다듬기 ⑬-2 ⑥.
// 옮기기: 엔진 main.swift 의 gitHardenEnv·gitHardenC 를 규칙으로 옮긴 값이 옮기기 전 글자와 같은지(덤프 대조)
import Foundation
import Testing
import AutopilotRules

@Suite("자율주행 엔진 git 보호") struct EngineGitTests {
    @Test("옮기기: 엔진 gitHardenEnv·gitHardenC 와 같은 글자(41927b1 의 main.swift 317·321줄)")
    func 옮기기() {
        #expect(apEngineGitHardenEnv == ["GIT_CONFIG_NOSYSTEM=1", "GIT_CONFIG_GLOBAL=/dev/null", "GIT_ATTR_NOSYSTEM=1", "GIT_NO_REPLACE_OBJECTS=1"])
        #expect(apEngineGitHardenC == ["-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgsign=false", "-c", "core.attributesFile=/dev/null",
                                       "-c", "safe.bareRepository=explicit", "-c", "diff.ignoreSubmodules=all"])
    }
}
