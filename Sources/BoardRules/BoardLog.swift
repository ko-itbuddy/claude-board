// 보드 기록 파일(board.log) 자리 — 보드는 그대로 동작해야 하는 문제(단축키 등록 실패·AppleScript 오류 등)를 한 줄씩 남긴다.
// 쓰는 곳이 둘(EmbeddedTerminal.swift 의 boardLog, SettingsScreen.swift 의 AppleScript 오류 기록)이라 자리를 여기 한 곳에서 정한다.
// 설치판은 두 곳의 파일이 없을 때 동작이 그대로 다르다: boardLog 는 파일이 없으면 쓰지 않고, AppleScript 오류 기록은 파일을 만든다.
// 시험 빌드(번들 id com.claude.board.dev, build-board.sh --dev)는 설치판 기록에 줄을 더하지 않게 시험 빌드 자율주행 뿌리 옆
// ~/Library/Application Support/Claude Board Dev/board.log 에 쓰고, 두 곳 모두 폴더·파일이 없으면 만든다(다듬기 ⑬-1)
import Foundation

/// 시험 빌드의 번들 id — 잠금 파일·분산 알림 이름·자율주행 뿌리를 따로 쓰는 것과 같은 기준
public let boardDevBundleID = "com.claude.board.dev"

/// 보드 기록 파일 자리 — 설치판은 홈 폴더 아래 ~/.claude/notifier/board.log(hook 스크립트가 쓰는 폴더와 같은 자리)
public func boardLogPath(home: String) -> String {
    home + "/.claude/notifier/board.log"
}

/// 기록 파일 자리와 없을 때 만들지 — dev 면 시험 빌드 자리(폴더·파일을 만든다)
public struct BoardLogPlace: Equatable, Sendable {
    public let path: String
    public let dev: Bool
    public init(path: String, dev: Bool) { self.path = path; self.dev = dev }
}

/// 이 앱이 쓰는 기록 파일 자리. 설치판(시험 빌드가 아닌 모든 번들 id·번들 id 없음)은 boardLogPath 그대로
public func boardLogPlace(bundleID: String?, home: String) -> BoardLogPlace {
    if bundleID == boardDevBundleID {
        return BoardLogPlace(path: home + "/Library/Application Support/Claude Board Dev/board.log", dev: true)
    }
    return BoardLogPlace(path: boardLogPath(home: home), dev: false)
}
