// 보드 기록 파일 자리(BoardLog.swift) 시험 — 옮기기 전 두 곳(boardLog 의 NSHomeDirectory() + "/.claude/notifier/board.log",
// AppleScript 오류 기록의 (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/board.log"))이 만들던 글자와 같은지 본다
import Foundation
import Testing
import BoardRules

struct BoardLogTests {
    @Test("옮기기 전 두 곳의 글자와 같다", arguments: ["/Users/a", "/Users/max mac", "/Users/한글 이름", "/var/root"])
    func sameAsBefore(home: String) {
        let old1 = home + "/.claude/notifier/board.log"
        let old2 = (home as NSString).appendingPathComponent(".claude/notifier/board.log")
        #expect(boardLogPath(home: home) == old1)
        #expect(boardLogPath(home: home) == old2)
    }
}
