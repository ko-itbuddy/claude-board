// 바깥에서 보드 탭 닫기(CloseTab.swift) 시험 — 명령줄 읽기·대상 찾기와 판정·답 꼴·요청 폴더·요청 꼴.
// 판정 표는 OpenTabTests 의 "이미 있는 탭 가리기" 꼴(화면 값 목록 → 판정)
import Foundation
import Testing
import BoardRules

struct CloseTabTests {
    static let sid = "0f0e1d2c-3b4a-5968-7766-554433221100"
    static let sid2 = "11111111-2222-3333-4444-555555555555"

    struct ArgsRow: Sendable, CustomTestStringConvertible {
        let note: String
        let args: [String]
        let want: CloseTabOptions?
        let why: String?   // 실패면 why 에 든 글자
        var testDescription: String { note }
    }

    @Test("명령줄 읽기", arguments: [
        ArgsRow(note: "세션 id", args: ["--session", sid], want: CloseTabOptions(target: .session(sid), stop: false, timeout: 30), why: nil),
        ArgsRow(note: "대문자 세션 id 는 소문자로", args: ["--session", sid.uppercased()],
                want: CloseTabOptions(target: .session(sid), stop: false, timeout: 30), why: nil),
        ArgsRow(note: "탭 이름", args: ["--title", "역할 2"], want: CloseTabOptions(target: .title("역할 2"), stop: false, timeout: 30), why: nil),
        ArgsRow(note: "= 꼴과 --stop", args: ["--session=\(sid)", "--stop"], want: CloseTabOptions(target: .session(sid), stop: true, timeout: 30), why: nil),
        ArgsRow(note: "--title= 꼴", args: ["--stop", "--title=exp-close-1"], want: CloseTabOptions(target: .title("exp-close-1"), stop: true, timeout: 30), why: nil),
        ArgsRow(note: "--timeout", args: ["--title", "a", "--timeout", "600"], want: CloseTabOptions(target: .title("a"), stop: false, timeout: 600), why: nil),
        ArgsRow(note: "--timeout= 꼴", args: ["--title", "a", "--timeout=1"], want: CloseTabOptions(target: .title("a"), stop: false, timeout: 1), why: nil),
        ArgsRow(note: "대상 없음", args: [], want: nil, why: "하나가 필요합니다"),
        ArgsRow(note: "--stop 만", args: ["--stop"], want: nil, why: "하나가 필요합니다"),
        ArgsRow(note: "둘 다 있음", args: ["--session", sid, "--title", "a"], want: nil, why: "하나만"),
        ArgsRow(note: "세션 id 꼴이 아님", args: ["--session", "역할"], want: nil, why: "UUID"),
        ArgsRow(note: "모르는 옵션", args: ["--title", "a", "--launch"], want: nil, why: "모르는 옵션"),
        ArgsRow(note: "자리 인자는 받지 않음", args: ["--title", "a", "b"], want: nil, why: "모르는 옵션"),
        ArgsRow(note: "-- 도 받지 않음", args: ["--title", "a", "--"], want: nil, why: "모르는 옵션"),
        ArgsRow(note: "값 없는 --title", args: ["--title"], want: nil, why: "값이 없습니다"),
        ArgsRow(note: "다음 인자가 옵션이면 값 없음", args: ["--title", "--stop"], want: nil, why: "값이 없습니다"),
        ArgsRow(note: "값 없는 --session", args: ["--session"], want: nil, why: "값이 없습니다"),
        ArgsRow(note: "값 없는 --timeout", args: ["--title", "a", "--timeout"], want: nil, why: "값이 없습니다"),
        ArgsRow(note: "--timeout 0", args: ["--title", "a", "--timeout", "0"], want: nil, why: "1~600"),
        ArgsRow(note: "--timeout 601", args: ["--title", "a", "--timeout", "601"], want: nil, why: "1~600"),
        ArgsRow(note: "--timeout 숫자 아님", args: ["--title", "a", "--timeout", "x"], want: nil, why: "1~600"),
        ArgsRow(note: "--stop=값", args: ["--title", "a", "--stop=1"], want: nil, why: "값을 받지 않습니다"),
        ArgsRow(note: "여러 줄 탭 이름", args: ["--title", "가\n나"], want: nil, why: "한 줄"),
        ArgsRow(note: "빈 탭 이름", args: ["--title="], want: nil, why: "한 줄"),
        ArgsRow(note: "101자 탭 이름", args: ["--title", String(repeating: "가", count: 101)], want: nil, why: "100자"),
        ArgsRow(note: "제어 문자 탭 이름", args: ["--title", "a\u{1B}b"], want: nil, why: "한 줄"),
    ])
    func args(_ r: ArgsRow) {
        switch parseCloseTabArgs(r.args) {
        case .success(let o): #expect(o == r.want)
        case .failure(let e):
            #expect(r.want == nil)
            #expect(e.reply.status == "bad-args")
            #expect(e.reply.why?.contains(r.why ?? "\u{0}") == true)
        }
    }

    static let a = UUID(uuidString: "00000000-0000-0000-0000-00000000000A")!
    static let b = UUID(uuidString: "00000000-0000-0000-0000-00000000000B")!

    static func screen(_ id: UUID, tab: String = "역할", session: String? = nil, names: [String] = [], external: Bool = true, takeover: Bool = false,
                       split: Bool = false, stopping: Bool = false, fg: CloseTabForeground = .prompt) -> CloseTabScreen {
        CloseTabScreen(id: id, tab: tab, sessionId: session, names: names, external: external, takeover: takeover, split: split, stopping: stopping,
                       foreground: fg)
    }

    enum Want: Equatable, Sendable {
        case close(UUID)
        case stopThenClose(UUID)
        case status(String, String)   // 답의 status 와 why 에 든 글자
    }

    struct PlanRow: Sendable, CustomTestStringConvertible {
        let note: String
        let target: CloseTabTarget
        let stop: Bool
        let screens: [CloseTabScreen]
        let want: Want
        var testDescription: String { note }
    }

    @Test("대상 찾기와 판정", arguments: [
        PlanRow(note: "세션 id 로 맞음 → 닫기", target: .session(sid), stop: false,
                screens: [screen(a, session: sid2), screen(b, session: sid)], want: .close(b)),
        PlanRow(note: "화면 세션 id 가 대문자여도 맞음", target: .session(sid), stop: false,
                screens: [screen(a, session: sid.uppercased())], want: .close(a)),
        PlanRow(note: "이름(대소문자 무시)으로 맞음 → 닫기", target: .title("Exp-Close-1"), stop: false,
                screens: [screen(a, names: ["exp-close-2"]), screen(b, names: ["exp-close-1"])], want: .close(b)),
        PlanRow(note: "셸만 남은 화면이 탭 이름으로만 맞음 → 닫기", target: .title("exp-close-1"), stop: false,
                screens: [screen(a, tab: "exp-close-1", session: sid, names: ["exp-close-1"])], want: .close(a)),
        PlanRow(note: "나눈 탭의 화면은 탭 이름으로 맞지 않음(이름 목록에 탭 이름이 없음) → not-found", target: .title("역할"), stop: false,
                screens: [screen(a, names: [], split: true), screen(b, names: ["다른 세션"], split: true)], want: .status("not-found", "이 이름")),
        PlanRow(note: "맞는 것 없음(세션) → not-found", target: .session(sid), stop: true, screens: [screen(a, session: sid2)],
                want: .status("not-found", sid)),
        PlanRow(note: "화면이 없음 → not-found", target: .title("역할"), stop: false, screens: [], want: .status("not-found", "역할")),
        PlanRow(note: "둘 이상 맞음 → refused(아무것도 닫지 않음)", target: .title("exp-close-5"), stop: false,
                screens: [screen(a, names: ["exp-close-5"]), screen(b, names: ["EXP-CLOSE-5"])], want: .status("refused", "여러 탭")),
        PlanRow(note: "둘 이상 맞으면 하나가 표지 없는 탭이어도 refused(여러 탭)", target: .title("x"), stop: true,
                screens: [screen(a, names: ["x"], external: false), screen(b, names: ["x"], fg: .claude)], want: .status("refused", "여러 탭")),
        PlanRow(note: "표지 없는 탭 → refused", target: .title("exp-close-3"), stop: false,
                screens: [screen(a, names: ["exp-close-3"], external: false)], want: .status("refused", "바깥 열기")),
        PlanRow(note: "표지 없는 탭은 --stop 이어도 claude 를 끝내지 않음", target: .session(sid), stop: true,
                screens: [screen(a, session: sid, external: false, fg: .claude)], want: .status("refused", "바깥 열기")),
        PlanRow(note: "이어받기 화면 → refused", target: .session(sid), stop: true,
                screens: [screen(a, session: sid, takeover: true)], want: .status("refused", "이어받은")),
        PlanRow(note: "이어받기가 표지보다 먼저", target: .session(sid), stop: false,
                screens: [screen(a, session: sid, external: false, takeover: true)], want: .status("refused", "이어받은")),
        PlanRow(note: "나눈 탭 → refused", target: .session(sid), stop: false,
                screens: [screen(a, session: sid, split: true), screen(b, split: true)], want: .status("refused", "나눈 탭")),
        PlanRow(note: "나눈 탭은 --stop 이어도 refused", target: .session(sid), stop: true,
                screens: [screen(a, session: sid, split: true, fg: .claude)], want: .status("refused", "나눈 탭")),
        PlanRow(note: "셸이 뜨는 중·막 넣음 → busy", target: .session(sid), stop: true,
                screens: [screen(a, session: sid, fg: .starting)], want: .status("busy", "막 뜨는 중")),
        PlanRow(note: "claude 도는 중(stop 없음) → busy", target: .session(sid), stop: false,
                screens: [screen(a, session: sid, fg: .claude)], want: .status("busy", "--stop")),
        PlanRow(note: "claude 도는 중(stop 있음) → 끝낸 뒤 닫기", target: .title("역할"), stop: true,
                screens: [screen(a, names: ["역할"], fg: .claude)], want: .stopThenClose(a)),
        PlanRow(note: "다른 프로그램 도는 중(stop 있어도) → busy", target: .session(sid), stop: true,
                screens: [screen(a, session: sid, fg: .other)], want: .status("busy", "다른 프로그램")),
        PlanRow(note: "셸 프롬프트는 --stop 이어도 바로 닫기", target: .session(sid), stop: true,
                screens: [screen(a, session: sid, fg: .prompt)], want: .close(a)),
        PlanRow(note: "이미 끝내는 중인 화면 → busy", target: .session(sid), stop: true,
                screens: [screen(a, session: sid, stopping: true, fg: .claude)], want: .status("busy", "끝내는 중")),
    ])
    func plan(_ r: PlanRow) {
        let p = planCloseTab(r.target, stop: r.stop, screens: r.screens)
        switch r.want {
        case .close(let id):
            #expect(p == .close(id))
            #expect(closeTabReply(p, screens: r.screens) == nil)
        case .stopThenClose(let id):
            #expect(p == .stopThenClose(id))
            #expect(closeTabReply(p, screens: r.screens) == nil)
        case .status(let status, let why):
            let reply = closeTabReply(p, screens: r.screens)
            #expect(reply?.ok == false)
            #expect(reply?.status == status)
            #expect(reply?.why?.contains(why) == true)
        }
    }

    @Test("답에는 맞은 탭의 이름·세션을 싣는다(여러 탭·없음은 빼고)")
    func replyInfo() {
        let s = Self.screen(Self.a, tab: "exp-close-1", session: Self.sid, fg: .claude)
        let busy = closeTabReply(planCloseTab(.session(Self.sid), stop: false, screens: [s]), screens: [s])
        #expect(busy?.line == #"{"ok":false,"session":"0f0e1d2c-3b4a-5968-7766-554433221100","status":"busy","tab":"exp-close-1","why":"claude 가 돌고 있습니다. --stop 을 주면 끝낸 뒤 닫습니다"}"#)
        let two = [Self.screen(Self.a, names: ["x"]), Self.screen(Self.b, names: ["x"])]
        let many = closeTabReply(planCloseTab(.title("x"), stop: false, screens: two), screens: two)
        #expect(many?.tab == nil && many?.session == nil)
        // --stop 뒤 다시 볼 때(셸 프롬프트로 돌아옴)
        #expect(closeTabDecide(Self.screen(Self.a, fg: .prompt), stop: false) == .close(Self.a))
        #expect(closeTabDecide(Self.screen(Self.a, fg: .claude), stop: false) == .busy(Self.a, "claude 가 돌고 있습니다. --stop 을 주면 끝낸 뒤 닫습니다"))
    }

    @Test("답은 한 줄 JSON — 키 차례 고정, 빗금·한글 그대로")
    func replyLine() {
        let r = OpenTabReply(ok: true, status: "closed", tab: "역할/2", session: Self.sid)
        #expect(r.line == #"{"ok":true,"session":"0f0e1d2c-3b4a-5968-7766-554433221100","status":"closed","tab":"역할/2"}"#)
        #expect(!r.line.contains("\n"))
        #expect(OpenTabReply(ok: false, status: "not-found", why: "없음/닫힘").line == #"{"ok":false,"status":"not-found","why":"없음/닫힘"}"#)
    }

    @Test("요청 폴더는 설치판·시험 빌드가 따로, 열기 폴더와 다른 자리")
    func place() {
        #expect(closeTabDir(bundleID: "com.claude.board", home: "/Users/a") == "/Users/a/Library/Application Support/Claude Board/close-tab")
        #expect(closeTabDir(bundleID: nil, home: "/Users/a") == "/Users/a/Library/Application Support/Claude Board/close-tab")
        #expect(closeTabDir(bundleID: "com.claude.board.dev", home: "/Users/a") == "/Users/a/Library/Application Support/Claude Board Dev/close-tab")
        for id in ["com.claude.board", "com.claude.board.dev"] {
            #expect(closeTabDir(bundleID: id, home: "/Users/a") != openTabDir(bundleID: id, home: "/Users/a"))
        }
    }

    @Test("요청 꼴 — 대상은 session·title 가운데 하나만 싣고, 다시 풀면 같다")
    func request() throws {
        let e = JSONEncoder()
        e.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        let bySession = CloseTabRequest(id: Self.sid2, target: .session(Self.sid), stop: true, at: 1000)
        #expect(String(decoding: try e.encode(bySession), as: UTF8.self)
                == #"{"at":1000,"id":"11111111-2222-3333-4444-555555555555","session":"0f0e1d2c-3b4a-5968-7766-554433221100","stop":true}"#)
        let byTitle = CloseTabRequest(id: Self.sid2, target: .title("역할/2"), stop: false, at: 1000)
        #expect(String(decoding: try e.encode(byTitle), as: UTF8.self) == #"{"at":1000,"id":"11111111-2222-3333-4444-555555555555","stop":false,"title":"역할/2"}"#)
        #expect(try JSONDecoder().decode(CloseTabRequest.self, from: try e.encode(byTitle)) == byTitle)
        // 보드는 요청 파일의 대상을 다시 본다 — 둘 다·둘 다 없음·꼴이 틀린 세션은 bad-args
        #expect(closeTabTarget(session: Self.sid, title: nil) == .success(.session(Self.sid)))
        #expect(closeTabTarget(session: nil, title: "역할") == .success(.title("역할")))
        for (s, t) in [(Self.sid, "역할"), (nil, nil), ("x", nil)] as [(String?, String?)] {
            guard case .failure(let f) = closeTabTarget(session: s, title: t) else { Issue.record("받음: \(String(describing: s)) \(String(describing: t))"); continue }
            #expect(f.reply.status == "bad-args")
        }
    }
}
