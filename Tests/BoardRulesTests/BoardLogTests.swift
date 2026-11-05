// 보드 기록 파일 자리(BoardLog.swift) 시험 — 옮기기 전 두 곳(boardLog 의 NSHomeDirectory() + "/.claude/notifier/board.log",
// AppleScript 오류 기록의 (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/board.log"))이 만들던 글자와 같은지 본다.
// 시험 빌드(com.claude.board.dev)만 따로 자리(다듬기 ⑬-1), 설치판은 번들 id 가 무엇이든(없어도) 옛 자리 그대로
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

    struct PlaceRow: Sendable, CustomTestStringConvertible {
        let note: String
        let bundleID: String?
        let path: String
        let dev: Bool
        var testDescription: String { note }
    }

    @Test("설치판은 옛 자리 그대로, 시험 빌드만 따로 자리", arguments: [
        PlaceRow(note: "설치판", bundleID: "com.claude.board", path: "/Users/a/.claude/notifier/board.log", dev: false),
        PlaceRow(note: "번들 id 없음(swift run 등)", bundleID: nil, path: "/Users/a/.claude/notifier/board.log", dev: false),
        PlaceRow(note: "다른 번들 id", bundleID: "com.example.other", path: "/Users/a/.claude/notifier/board.log", dev: false),
        PlaceRow(note: "대소문자만 다른 번들 id 는 시험 빌드가 아니다", bundleID: "com.claude.board.DEV", path: "/Users/a/.claude/notifier/board.log", dev: false),
        PlaceRow(note: "시험 빌드", bundleID: "com.claude.board.dev", path: "/Users/a/Library/Application Support/Claude Board Dev/board.log", dev: true),
    ])
    func place(_ r: PlaceRow) {
        let p = boardLogPlace(bundleID: r.bundleID, home: "/Users/a")
        #expect(p.path == r.path)
        #expect(p.dev == r.dev)
        if !r.dev { #expect(p.path == boardLogPath(home: "/Users/a")) }
    }
}
