// 바깥에서 보드 탭 열기(OpenTab.swift) 시험 — 명령줄 읽기·요청 검사·claude 인자 읽기·이미 있는 탭 가리기·답 꼴.
// shlexQuote 의 기대값은 Python shlex.quote 로 만든 글자(state.py 의 resume_cmd 와 같은 꼴), 보드 EmbeddedTerminal.swift 에서 옮기기 전 함수와 같은 식이다
import Foundation
import Testing
import BoardRules

struct OpenTabTests {
    @Test("shlexQuote 는 Python shlex.quote 와 같다", arguments: [
        ("claude", "claude"), ("", "''"), ("a b", "'a b'"), ("it's", #"'it'"'"'s'"#), ("한글", "'한글'"),
        ("/Users/x/.claude/notifier/plugins/md-render", "/Users/x/.claude/notifier/plugins/md-render"),
        ("--name=x y", "'--name=x y'"), ("줄\n바꿈", "'줄\n바꿈'"), ("$HOME", "'$HOME'"), (#"a"b"#, #"'a"b'"#),
    ])
    func quote(_ input: String, _ want: String) {
        #expect(shlexQuote(input) == want)
    }

    @Test("shellWords 는 shlex.quote 꼴과 흔한 셸 꼴을 푼다")
    func words() {
        // Python shlex.split 과 같은 결과
        #expect(shellWords(#"claude --model opus --plugin-dir '/a b/plugins/md-render' --name 'it'"'"'s' --resume 0f0e"#)
                == ["claude", "--model", "opus", "--plugin-dir", "/a b/plugins/md-render", "--name", "it's", "--resume", "0f0e"])
        #expect(shellWords(#"claude --name "역할 \"2\"" a\ b"#) == ["claude", "--name", #"역할 "2""#, "a b"])
        #expect(shellWords("  claude\t-n  x  ") == ["claude", "-n", "x"])
        #expect(shellWords("claude ''") == ["claude", ""])
        #expect(shellWords("claude '열린") == nil)
        #expect(shellWords(#"claude "열린"#) == nil)
        // 인자마다 따옴표로 묶어 이은 글자는 다시 같은 인자로 풀린다
        let argv = ["claude", "--name", "it's", "줄\n바꿈", "$HOME", "", #"a"b\c"#]
        #expect(shellWords(argv.map(shlexQuote).joined(separator: " ")) == argv)
    }

    struct ArgsRow: Sendable, CustomTestStringConvertible {
        let note: String
        let args: [String]
        let want: OpenTabOptions?
        let why: String?   // 실패면 why 에 든 글자
        var testDescription: String { note }
    }

    @Test("명령줄 읽기", arguments: [
        ArgsRow(note: "보통", args: ["--cwd", "/w", "--title", "역할", "--", "claude", "--resume", "x"],
                want: OpenTabOptions(cwd: "/w", title: "역할", launch: false, timeout: 20, argv: ["claude", "--resume", "x"]), why: nil),
        ArgsRow(note: "= 꼴·--launch 는 기다림이 길다", args: ["--cwd=/w", "--launch", "--", "claude"],
                want: OpenTabOptions(cwd: "/w", title: nil, launch: true, timeout: 40, argv: ["claude"]), why: nil),
        ArgsRow(note: "--timeout", args: ["--cwd", "/w", "--timeout", "5", "--", "claude"],
                want: OpenTabOptions(cwd: "/w", title: nil, launch: false, timeout: 5, argv: ["claude"]), why: nil),
        ArgsRow(note: "상대 경로는 부른 폴더 기준", args: ["--cwd", "sub/dir", "--", "claude"],
                want: OpenTabOptions(cwd: "/here/sub/dir", title: nil, launch: false, timeout: 20, argv: ["claude"]), why: nil),
        ArgsRow(note: "-- 뒤의 --cwd 는 claude 인자", args: ["--cwd", "/w", "--", "claude", "--cwd", "x", "--"],
                want: OpenTabOptions(cwd: "/w", title: nil, launch: false, timeout: 20, argv: ["claude", "--cwd", "x", "--"]), why: nil),
        ArgsRow(note: "-- 없음", args: ["--cwd", "/w", "claude"], want: nil, why: "모르는 옵션"),
        ArgsRow(note: "-- 로 끝나지 않음", args: ["--cwd", "/w"], want: nil, why: "-- 가 없습니다"),
        ArgsRow(note: "--cwd 없음", args: ["--", "claude"], want: nil, why: "--cwd 가 없습니다"),
        ArgsRow(note: "값 없는 --cwd", args: ["--cwd", "--", "claude"], want: nil, why: "값이 없습니다"),
        ArgsRow(note: "--timeout 범위", args: ["--cwd", "/w", "--timeout", "0", "--", "claude"], want: nil, why: "1~600"),
        ArgsRow(note: "--launch=값", args: ["--cwd", "/w", "--launch=1", "--", "claude"], want: nil, why: "값을 받지 않습니다"),
    ])
    func args(_ r: ArgsRow) {
        switch parseOpenTabArgs(r.args, here: "/here") {
        case .success(let o): #expect(o == r.want)
        case .failure(let e):
            #expect(r.want == nil)
            #expect(e.reply.status == "bad-args")
            #expect(e.reply.why?.contains(r.why ?? "\u{0}") == true)
        }
    }

    static let sid = "0f0e1d2c-3b4a-5968-7766-554433221100"
    static let newID = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"

    struct PrepRow: Sendable, CustomTestStringConvertible {
        let note: String
        let title: String?
        let argv: [String]
        let argvOut: [String]?
        let session: String?
        let name: String?
        let why: String?
        var testDescription: String { note }
    }

    @Test("요청 검사와 띄울 꼴", arguments: [
        PrepRow(note: "새 세션은 claude 바로 뒤에 --session-id", title: nil, argv: ["claude", "--model", "opus", "첫 요청"],
                argvOut: ["claude", "--session-id", newID, "--model", "opus", "첫 요청"], session: newID, name: nil, why: nil),
        PrepRow(note: "이어 열기는 그 세션(대문자 id 는 소문자로)", title: nil, argv: ["claude", "--name", "역할-2", "--resume", sid.uppercased()],
                argvOut: ["claude", "--name", "역할-2", "--resume", sid.uppercased()], session: sid, name: "역할-2", why: nil),
        PrepRow(note: "--resume=", title: "역할", argv: ["claude", "--resume=\(sid)"], argvOut: ["claude", "--resume=\(sid)"], session: sid, name: "역할", why: nil),
        PrepRow(note: "-r 값", title: nil, argv: ["claude", "-r", sid, "-n", "역할"], argvOut: ["claude", "-r", sid, "-n", "역할"], session: sid, name: "역할", why: nil),
        PrepRow(note: "고르기 --resume 은 세션을 모른다", title: nil, argv: ["claude", "--resume", "--model", "opus"],
                argvOut: ["claude", "--resume", "--model", "opus"], session: nil, name: nil, why: nil),
        PrepRow(note: "--continue 는 세션을 모른다", title: nil, argv: ["claude", "-c"], argvOut: ["claude", "-c"], session: nil, name: nil, why: nil),
        PrepRow(note: "갈래(--fork-session)는 새 세션이라 이어 연 id 가 아니다", title: nil, argv: ["claude", "--resume", sid, "--fork-session"],
                argvOut: ["claude", "--resume", sid, "--fork-session"], session: nil, name: nil, why: nil),
        PrepRow(note: "--session-id 를 주면 그 id", title: nil, argv: ["claude", "--session-id", sid], argvOut: ["claude", "--session-id", sid], session: sid, name: nil, why: nil),
        PrepRow(note: "이름 꼴 --resume 은 id 를 모른다", title: nil, argv: ["claude", "--resume", "역할"], argvOut: ["claude", "--resume", "역할"], session: nil, name: nil, why: nil),
        PrepRow(note: "첫 요청의 줄바꿈·탭은 된다", title: nil, argv: ["claude", "한 줄\n두 줄\t탭"],
                argvOut: ["claude", "--session-id", newID, "한 줄\n두 줄\t탭"], session: newID, name: nil, why: nil),
        PrepRow(note: "claude 가 아니면 거절", title: nil, argv: ["bash", "-c", "rm -rf ~"], argvOut: nil, session: nil, name: nil, why: "claude 명령만"),
        PrepRow(note: "경로 꼴 claude 도 거절", title: nil, argv: ["/tmp/x/claude"], argvOut: nil, session: nil, name: nil, why: "claude 명령만"),
        PrepRow(note: "빈 명령", title: nil, argv: [], argvOut: nil, session: nil, name: nil, why: "(없음)"),
        PrepRow(note: "ESC(붙여넣기 끝 꼴)는 거절", title: nil, argv: ["claude", "a\u{1B}[201~; rm -rf ~"], argvOut: nil, session: nil, name: nil, why: "제어 문자"),
        PrepRow(note: "CR 은 거절", title: nil, argv: ["claude", "a\rb"], argvOut: nil, session: nil, name: nil, why: "제어 문자"),
        PrepRow(note: "C1 제어 문자는 거절", title: nil, argv: ["claude", "a\u{9B}b"], argvOut: nil, session: nil, name: nil, why: "제어 문자"),
        PrepRow(note: "탭 이름과 세션 이름이 다르면 거절", title: "가", argv: ["claude", "--name", "나"], argvOut: nil, session: nil, name: nil, why: "다릅니다"),
        PrepRow(note: "같으면 됨", title: "가", argv: ["claude", "--name", "가"], argvOut: ["claude", "--session-id", newID, "--name", "가"], session: newID, name: "가", why: nil),
        PrepRow(note: "여러 줄 탭 이름은 거절", title: "가\n나", argv: ["claude"], argvOut: nil, session: nil, name: nil, why: "한 줄"),
        PrepRow(note: "여러 줄 세션 이름은 거절", title: nil, argv: ["claude", "--name", "가\n나"], argvOut: nil, session: nil, name: nil, why: "한 줄"),
        PrepRow(note: "너무 긴 요청", title: nil, argv: ["claude", String(repeating: "가", count: 100_000)], argvOut: nil, session: nil, name: nil, why: "너무 깁니다"),
    ])
    func prepare(_ r: PrepRow) {
        switch openTabPrepare(cwd: "/w", title: r.title, argv: r.argv, newID: Self.newID) {
        case .success(let p):
            #expect(p.argv == r.argvOut)
            #expect(p.sessionId == r.session)
            #expect(p.name == r.name)
            #expect(p.resumes == (r.session != nil && r.argv.contains { $0 == "--resume" || $0 == "-r" || $0.hasPrefix("--resume=") }))
            #expect(p.command == p.argv.map(shlexQuote).joined(separator: " "))
            #expect(shellWords(p.command) == p.argv)
        case .failure(let e):
            #expect(r.argvOut == nil)
            #expect(e.reply.why?.contains(r.why ?? "\u{0}") == true)
        }
    }

    @Test("--cwd 는 절대 경로")
    func cwd() {
        guard case .failure(let e) = openTabPrepare(cwd: "w", title: nil, argv: ["claude"], newID: Self.newID) else { Issue.record("받음"); return }
        #expect(e.reply.status == "bad-args")
        guard case .failure = openTabPrepare(cwd: "/w\n/x", title: nil, argv: ["claude"], newID: Self.newID) else { Issue.record("받음"); return }
    }

    static let a = UUID(uuidString: "00000000-0000-0000-0000-00000000000A")!
    static let b = UUID(uuidString: "00000000-0000-0000-0000-00000000000B")!

    struct PlanRow: Sendable, CustomTestStringConvertible {
        let note: String
        let session: String?
        let name: String?
        let screens: [OpenTabScreen]
        let live: [LiveClaude]
        let want: OpenTabPlan
        var testDescription: String { note }
    }

    static func screen(_ id: UUID, session: String? = nil, cwd: String = "/w", pid: Int32? = nil, names: [String] = [], busy: Bool = false,
                       takeover: Bool = false) -> OpenTabScreen {
        OpenTabScreen(id: id, sessionId: session, cwd: cwd, pid: pid, names: names, busy: busy, takeover: takeover)
    }

    @Test("이미 있는 탭 가리기", arguments: [
        PlanRow(note: "아무것도 없으면 새 탭", session: sid, name: "역할", screens: [], live: [], want: .open),
        PlanRow(note: "같은 세션이 도는 화면은 앞으로만", session: sid, name: nil,
                screens: [screen(a), screen(b, session: sid, pid: 10, busy: true)], live: [LiveClaude(pid: 10, session: sid, name: nil)], want: .exists(b)),
        PlanRow(note: "같은 세션인데 claude 가 끝나 셸만 남은 화면은 그 화면에", session: sid, name: nil,
                screens: [screen(b, session: sid)], live: [], want: .reuse(b)),
        PlanRow(note: "같은 이름(대소문자 무시)이 도는 화면은 앞으로만", session: newID, name: "Role-2",
                screens: [screen(a, session: sid, pid: 10, names: ["role-2"], busy: true)], live: [LiveClaude(pid: 10, session: nil, name: "role-2")], want: .exists(a)),
        PlanRow(note: "세션 id 가 이름보다 먼저", session: sid, name: "역할",
                screens: [screen(a, names: ["역할"], busy: true), screen(b, session: sid)], live: [], want: .reuse(b)),
        PlanRow(note: "셸만 남은 같은 이름의 화면이 다른 폴더면 새 탭", session: newID, name: "역할",
                screens: [screen(a, cwd: "/other", names: ["역할"])], live: [], want: .open),
        PlanRow(note: "다른 폴더여도 claude 가 돌면 앞으로만", session: sid, name: nil,
                screens: [screen(a, session: sid, cwd: "/other", pid: 10, busy: true)], live: [], want: .exists(a)),
        PlanRow(note: "이어받기 화면은 셸만 남아도 다른 명령을 넣지 않는다", session: sid, name: nil,
                screens: [screen(a, session: sid, takeover: true)], live: [], want: .exists(a)),
        PlanRow(note: "보드 탭 밖에서 같은 세션이 돌면 거절", session: sid, name: nil,
                screens: [screen(a, pid: 11, busy: true)], live: [LiveClaude(pid: 20, session: sid, name: nil)], want: .refuse("이 세션이 보드 탭 밖(pid 20)에서 돌고 있습니다")),
        PlanRow(note: "보드 탭 밖에서 같은 이름이 돌면 거절", session: newID, name: "역할",
                screens: [], live: [LiveClaude(pid: 21, session: nil, name: "역할")], want: .refuse("같은 이름(역할)의 세션이 보드 탭 밖(pid 21)에서 돌고 있습니다")),
        PlanRow(note: "이름이 아직 화면 값에 없어도 그 claude 가 든 보드 탭 화면이면 앞으로만", session: newID, name: "역할",
                screens: [screen(a, pid: 20, busy: true), screen(b, pid: 21, busy: true)], live: [LiveClaude(pid: 21, session: nil, name: "역할")], want: .exists(b)),
        PlanRow(note: "세션도 마찬가지", session: sid, name: nil,
                screens: [screen(a, pid: 21, busy: true)], live: [LiveClaude(pid: 21, session: sid, name: nil)], want: .exists(a)),
        PlanRow(note: "이름도 세션도 없으면 새 탭", session: nil, name: nil, screens: [screen(a, busy: true)], live: [LiveClaude(pid: 1, session: nil, name: "x")], want: .open),
        PlanRow(note: "세션 id 없는 화면은 세션으로 맞지 않는다", session: nil, name: nil, screens: [screen(a)], live: [], want: .open),
    ])
    func plan(_ r: PlanRow) {
        #expect(planOpenTab(sessionId: r.session, name: r.name, cwd: "/w", screens: r.screens, live: r.live) == r.want)
    }

    @Test("claude 인자 읽기")
    func argInfo() {
        #expect(claudeArgInfo(["claude", "--name=가", "--resume", Self.sid, "-c"]) == ClaudeArgInfo(name: "가", resume: Self.sid, continues: true))
        #expect(claudeArgInfo(["claude", "-r"]) == ClaudeArgInfo(picker: true))
        // 첫 요청(위치 인자)에 든 옵션 꼴 글자는 한 인자라 옵션이 아니다
        #expect(claudeArgInfo(["claude", "--name 가"]) == ClaudeArgInfo())
        // 첫 인자(claude)는 보지 않는다
        #expect(claudeArgInfo(["--name", "가"]) == ClaudeArgInfo())
    }

    @Test("답은 한 줄 JSON — 키 차례 고정, 빗금·한글 그대로")
    func replyLine() {
        let r = OpenTabReply(ok: true, status: "opened", tab: "역할/2", session: Self.sid)
        #expect(r.line == #"{"ok":true,"session":"0f0e1d2c-3b4a-5968-7766-554433221100","status":"opened","tab":"역할/2"}"#)
        #expect(OpenTabFailure.boardOff("꺼짐").reply.line == #"{"ok":false,"status":"board-off","why":"꺼짐"}"#)
        #expect(OpenTabReply(ok: true, status: "exists", pid: 42).line == #"{"ok":true,"pid":42,"status":"exists"}"#)
    }

    @Test("요청 폴더는 설치판·시험 빌드가 따로, 오래된 요청은 받지 않는다")
    func placeAndStale() {
        #expect(openTabDir(bundleID: "com.claude.board", home: "/Users/a") == "/Users/a/Library/Application Support/Claude Board/open-tab")
        #expect(openTabDir(bundleID: nil, home: "/Users/a") == "/Users/a/Library/Application Support/Claude Board/open-tab")
        #expect(openTabDir(bundleID: "com.claude.board.dev", home: "/Users/a") == "/Users/a/Library/Application Support/Claude Board Dev/open-tab")
        #expect(!openTabStale(at: 1000, now: 1000))
        #expect(!openTabStale(at: 1000, now: 1600))
        #expect(openTabStale(at: 1000, now: 1601))
        #expect(!openTabStale(at: 1060, now: 1000))
        #expect(openTabStale(at: 1061, now: 1000))
    }
}
