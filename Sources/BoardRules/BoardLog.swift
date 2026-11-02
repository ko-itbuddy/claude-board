// 보드 기록 파일(board.log) 자리 — 보드는 그대로 동작해야 하는 문제(단축키 등록 실패·AppleScript 오류 등)를 한 줄씩 남긴다.
// 쓰는 곳이 둘(EmbeddedTerminal.swift 의 boardLog, SettingsScreen.swift 의 AppleScript 오류 기록)이라 자리를 여기 한 곳에서 정한다.
// 두 곳의 파일이 없을 때 동작은 그대로 다르다: boardLog 는 파일이 없으면 쓰지 않고, AppleScript 오류 기록은 파일을 만든다
import Foundation

/// 보드 기록 파일 자리 — 홈 폴더 아래 ~/.claude/notifier/board.log(hook 스크립트가 쓰는 폴더와 같은 자리)
public func boardLogPath(home: String) -> String {
    home + "/.claude/notifier/board.log"
}
