// 바깥에서 보드 탭 열기(ClaudeBoard --open-tab) 규칙 — 명령줄 읽기, 요청 검사, claude 인자에서 세션 id·이름 읽기, 이미 있는 탭 가리기.
// 다른 프로그램(다른 Claude Code 세션의 스킬 등)이 보드 탭에 claude 를 띄우는 입구다. 명령줄 도구(같은 실행 파일)가 요청을 사용자만 쓰는 폴더에
// 파일로 남기고 분산 알림으로 보드를 깨우면, 보드가 그 파일을 읽어 보드가 직접 연 탭과 같은 길(TerminalHub.run)로 연다.
// 분산 알림에는 내용을 싣지 않는다 — 알림은 같은 로그인의 어떤 프로세스든 보낼 수 있지만 요청 파일은 그 사용자만 쓸 수 있는 폴더에 있어야 읽는다.
// 셸 명령이 아니라 claude 인자 목록만 받는다(첫 인자가 claude 가 아니면 거절, 보드가 따옴표로 묶어 셸에 치고 다시 읽어 확인한다).
// 파일·시각·프로세스는 앱이 읽어 값으로 넘긴다(이 파일은 Foundation 만)
import Foundation

// 셸 따옴표(shlexQuote)·낱말 읽기(shellWords)·낱말 단위 고치기는 ShellCommand.swift

// MARK: - 명령줄

/// ClaudeBoard --open-tab 의 옵션(-- 앞)과 띄울 claude 인자(-- 뒤)
public struct OpenTabOptions: Equatable, Sendable {
    public var cwd: String
    public var title: String?
    public var launch: Bool        // 보드가 꺼져 있으면 켜고 연다(없으면 "보드 꺼짐" 으로 실패)
    public var timeout: Double     // 보드의 답을 기다리는 초
    public var argv: [String]
    public var noFocus: Bool       // --no-focus: 보드를 앞으로 가져오지 않고 탭 선택·키보드 포커스도 그대로 둔 채 뒤에서 연다
    public init(cwd: String, title: String?, launch: Bool, timeout: Double, argv: [String], noFocus: Bool = false) {
        self.cwd = cwd; self.title = title; self.launch = launch; self.timeout = timeout; self.argv = argv; self.noFocus = noFocus
    }
}

public let openTabUsage = "쓰는 법: ClaudeBoard --open-tab --cwd <폴더> [--title <탭 이름>] [--launch] [--no-focus] [--timeout <초>] -- claude <인자…>"

/// 답을 기다리는 기본 초 — 보드가 꺼져 있어 켜고 열 때(--launch)는 되살리기(최대 5초 기다림)까지 더 기다린다
public func openTabDefaultTimeout(launch: Bool) -> Double { launch ? 40 : 20 }

/// --open-tab 뒤의 인자를 읽는다. 상대 경로 --cwd 는 here(명령을 부른 폴더) 기준으로 붙인다(.·.. 는 앱이 풀어 견준다)
public func parseOpenTabArgs(_ args: [String], here: String) -> Result<OpenTabOptions, OpenTabFailure> {
    var cwd: String?, title: String?, launch = false, noFocus = false, timeout: Double?
    var i = 0
    func value(_ flag: String) -> Result<String, OpenTabFailure> {
        guard i + 1 < args.count, args[i + 1] != "--" else { return .failure(.badArgs("\(flag) 뒤에 값이 없습니다. \(openTabUsage)")) }
        i += 1
        return .success(args[i])
    }
    while i < args.count, args[i] != "--" {
        let a = args[i]
        let flag = a.split(separator: "=", maxSplits: 1).first.map(String.init) ?? a
        let inline: String? = a.contains("=") && a.hasPrefix("--") ? String(a.dropFirst(flag.count + 1)) : nil
        func take() -> Result<String, OpenTabFailure> { inline.map { .success($0) } ?? value(flag) }
        switch flag {
        case "--cwd":
            switch take() { case .success(let v): cwd = v; case .failure(let e): return .failure(e) }
        case "--title":
            switch take() { case .success(let v): title = v; case .failure(let e): return .failure(e) }
        case "--timeout":
            switch take() {
            case .success(let v):
                guard let t = Double(v), t.isFinite, t >= 1, t <= 600 else { return .failure(.badArgs("--timeout 은 1~600 초입니다: \(v)")) }
                timeout = t
            case .failure(let e): return .failure(e)
            }
        case "--launch":
            guard inline == nil else { return .failure(.badArgs("--launch 는 값을 받지 않습니다")) }
            launch = true
        case "--no-focus":
            guard inline == nil else { return .failure(.badArgs("--no-focus 는 값을 받지 않습니다")) }
            noFocus = true
        default:
            return .failure(.badArgs("모르는 옵션입니다: \(a). \(openTabUsage)"))
        }
        i += 1
    }
    guard i < args.count else { return .failure(.badArgs("띄울 명령 앞에 -- 가 없습니다. \(openTabUsage)")) }
    guard let cwd, !cwd.isEmpty else { return .failure(.badArgs("--cwd 가 없습니다. \(openTabUsage)")) }
    let abs = cwd.hasPrefix("/") ? cwd : (here.hasSuffix("/") ? here : here + "/") + cwd
    return .success(OpenTabOptions(cwd: abs, title: title, launch: launch,
                                   timeout: timeout ?? openTabDefaultTimeout(launch: launch), argv: Array(args[(i + 1)...]), noFocus: noFocus))
}

// MARK: - 요청·답

/// 요청 파일(<요청 id>.req.json) — 명령줄 도구가 쓰고 보드가 읽은 뒤 지운다
public struct OpenTabRequest: Codable, Equatable, Sendable {
    public var id: String
    public var cwd: String
    public var title: String?
    public var argv: [String]
    public var at: Double          // 만든 시각(1970 기준 초)
    /// --no-focus 로 뒤에서 열기 — 켤 때만 true, 끄면 nil 이라 인코딩한 JSON 에 키가 없다(3.3.3 까지의 요청과 같은 JSON).
    /// 이 칸이 없는 옛 명령줄 도구의 요청은 nil(지금 동작), 옛 보드(3.3.2·3.3.3)는 모르는 키를 무시해 요청은 받고 선택지만 듣지 않는다
    public var noFocus: Bool?
    public init(id: String, cwd: String, title: String?, argv: [String], at: Double, noFocus: Bool? = nil) {
        self.id = id; self.cwd = cwd; self.title = title; self.argv = argv; self.at = at; self.noFocus = noFocus
    }

    /// 보드를 앞으로 가져오고 그 탭을 골라 키보드 포커스를 옮기는가(opened·exists·reopened 모두) — --no-focus 가 아니면
    public var reveals: Bool { noFocus != true }
}

/// 답(<요청 id>.reply.json, 명령줄 도구가 한 줄 JSON 으로 그대로 찍는다). status 는 opened(새 탭)·exists(이미 도는 탭을 앞으로)·
/// reopened(claude 가 끝나 셸만 남았거나 되살린 뒤 아직 들어가지 않은 같은 세션·이름의 탭에 다시 띄움), 실패는 bad-args·board-off·refused·timeout
public struct OpenTabReply: Codable, Equatable, Sendable {
    public var ok: Bool
    public var status: String
    public var tab: String?        // 탭 이름
    public var session: String?    // 세션 id(아는 때만 — --continue·고르기 --resume 은 띄운 뒤에야 안다)
    public var pid: Int32?         // 이미 도는 claude 의 pid(exists 일 때만 — 새로 띄운 claude 는 셸이 뜬 뒤에 생긴다)
    public var why: String?
    public init(ok: Bool, status: String, tab: String? = nil, session: String? = nil, pid: Int32? = nil, why: String? = nil) {
        self.ok = ok; self.status = status; self.tab = tab; self.session = session; self.pid = pid; self.why = why
    }

    /// 한 줄 JSON — 키 차례 고정, 빗금·한글 그대로
    public var line: String {
        let e = JSONEncoder()
        e.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return (try? e.encode(self)).map { String(decoding: $0, as: UTF8.self) } ?? #"{"ok":false,"status":"error"}"#
    }
}

public enum OpenTabFailure: Error, Equatable, Sendable {
    case badArgs(String)
    case boardOff(String)
    case refused(String)
    case timeout(String)

    public var reply: OpenTabReply {
        switch self {
        case .badArgs(let w): return OpenTabReply(ok: false, status: "bad-args", why: w)
        case .boardOff(let w): return OpenTabReply(ok: false, status: "board-off", why: w)
        case .refused(let w): return OpenTabReply(ok: false, status: "refused", why: w)
        case .timeout(let w): return OpenTabReply(ok: false, status: "timeout", why: w)
        }
    }
}

/// 요청 폴더 — 사용자만 쓰는 ~/Library/Application Support/<Claude Board|Claude Board Dev>/open-tab. 설치판·시험 빌드가 따로다(잠금 파일·분산 알림 이름처럼).
/// Claude Code 샌드박스·자율주행 모델 세션의 쓰기 범위 밖이라 그 안에서 돌린 명령은 요청을 남기지 못한다
public func openTabDir(bundleID: String?, home: String) -> String {
    home + "/Library/Application Support/" + (bundleID == boardDevBundleID ? "Claude Board Dev" : "Claude Board") + "/open-tab"
}

/// 보드가 받지 않을 만큼 오래된 요청인가 — 명령줄 도구가 기다림을 그만두고 떠난 요청을 보드가 나중에 켜져 띄우지 않게(기다림 상한 600초)
public func openTabStale(at: Double, now: Double) -> Bool { now - at > 600 || at - now > 60 }

// MARK: - 요청 검사

/// 요청 하나의 상한 — 첫 요청 글이 길어도 넉넉하게, 셸에 붙여넣을 수 있는 만큼
public let openTabMaxBytes = 256 * 1024

/// claude 인자에서 읽은 것
public struct ClaudeArgInfo: Equatable, Sendable {
    public var name: String?       // --name·-n
    public var resume: String?     // --resume·-r 의 값(없이 주면 고르기 — nil 이고 picker)
    public var picker: Bool        // 값 없는 --resume·-r
    public var continues: Bool     // --continue·-c
    public var sessionFlag: String?   // --session-id
    public var fork: Bool          // --fork-session
    public init(name: String? = nil, resume: String? = nil, picker: Bool = false, continues: Bool = false, sessionFlag: String? = nil, fork: Bool = false) {
        self.name = name; self.resume = resume; self.picker = picker; self.continues = continues; self.sessionFlag = sessionFlag; self.fork = fork
    }
}

/// claude 인자(첫 인자 claude 뒤)를 훑어 이름·이어 열기·세션 id 를 읽는다. 값은 다음 인자(-로 시작하지 않을 때) 또는 --옵션=값
public func claudeArgInfo(_ argv: [String]) -> ClaudeArgInfo {
    var info = ClaudeArgInfo()
    var i = 1
    while i < argv.count {
        let a = argv[i]
        let (flag, inline): (String, String?) = a.hasPrefix("--") && a.contains("=")
            ? (String(a[..<a.firstIndex(of: "=")!]), String(a[a.index(after: a.firstIndex(of: "=")!)...])) : (a, nil)
        func next() -> String? {
            if let inline { return inline }
            guard i + 1 < argv.count, !argv[i + 1].hasPrefix("-") else { return nil }
            i += 1
            return argv[i]
        }
        switch flag {
        case "--name", "-n": if let v = next() { info.name = v }
        case "--resume", "-r":
            if let v = next() { info.resume = v; info.picker = false } else { info.picker = true }
        case "--continue", "-c": info.continues = true
        case "--session-id": if let v = next() { info.sessionFlag = v }
        case "--fork-session": info.fork = true
        default: break
        }
        i += 1
    }
    return info
}

/// Claude Code 세션 id 꼴(UUID)인가
public func isSessionID(_ s: String) -> Bool {
    s.range(of: "^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$", options: .regularExpression) != nil
}

/// 보드가 띄울 꼴로 고친 요청
public struct OpenTabPlanInput: Equatable, Sendable {
    public var argv: [String]      // 새 세션이면 claude 바로 뒤에 --session-id <새 id> 를 넣은 인자
    public var sessionId: String?  // 이 탭의 세션 id(이어 여는 세션·--session-id·새로 붙인 id), 모르면 nil
    public var resumes = false     // sessionId 가 --resume 으로 이어 여는 세션이다(대화 기록이 있어야 뜬다)
    public var name: String?       // 탭 이름이자 세션 이름(--name 이 없으면 --title)
    public var command: String     // 셸에 칠 명령(인자마다 shellArg, 다시 읽어 argv 그대로인 것만)
}

/// 글자가 셸에 붙여넣기 안전한가 — 제어 문자(줄바꿈·탭은 됨)는 붙여넣기를 끊고 키로 들어갈 수 있어 거절한다
func pasteSafe(_ s: String) -> Bool {
    !s.unicodeScalars.contains { v in
        (v.value < 0x20 && v != "\n" && v != "\t") || v.value == 0x7F || (0x80...0x9F).contains(v.value)
            || v.value == 0x2028 || v.value == 0x2029
    }
}

/// 탭 이름 꼴인가 — 비지 않은 한 줄, 100자까지, 제어 문자 없음(--open-tab·--close-tab 의 --title 이 함께 쓴다)
public func tabTitleOK(_ title: String) -> Bool {
    !title.isEmpty && pasteSafe(title) && !title.contains("\n") && !title.contains("\t") && title.count <= 100
}

/// 요청을 검사하고 띄울 꼴로 고친다. newID 는 새 세션에 붙일 id(앱이 UUID 로 만들어 넘긴다)
public func openTabPrepare(cwd: String, title: String?, argv: [String], newID: String) -> Result<OpenTabPlanInput, OpenTabFailure> {
    guard cwd.hasPrefix("/"), pasteSafe(cwd), !cwd.contains("\n"), !cwd.contains("\t") else { return .failure(.badArgs("--cwd 는 절대 경로여야 합니다: \(cwd)")) }
    guard argv.first == "claude" else {
        return .failure(.badArgs("claude 명령만 띄웁니다(-- 뒤 첫 인자가 claude 여야 함): \(argv.first ?? "(없음)")"))
    }
    guard argv.allSatisfy(pasteSafe) else { return .failure(.badArgs("인자에 제어 문자가 있습니다(줄바꿈·탭만 됨)")) }
    if let title {
        guard tabTitleOK(title) else { return .failure(.badArgs("--title 은 한 줄, 100자까지입니다")) }
    }
    let size = argv.reduce(0) { $0 + $1.utf8.count + 1 } + cwd.utf8.count + (title?.utf8.count ?? 0)
    guard size <= openTabMaxBytes else { return .failure(.badArgs("요청이 너무 깁니다(\(size) 바이트, 상한 \(openTabMaxBytes))")) }
    let info = claudeArgInfo(argv)
    if let n = info.name {
        guard !n.isEmpty, pasteSafe(n), !n.contains("\n"), !n.contains("\t") else { return .failure(.badArgs("--name 은 한 줄이어야 합니다")) }
    }
    // 보드는 탭 이름을 그 탭 claude 의 세션 이름으로 쓴다(다시 뜰 때 되살린 탭에도 탭 이름을 --name 으로 붙인다) — 둘이 다르면 다시 뜬 뒤 이름이 바뀐다
    if let n = info.name, let title, n != title {
        return .failure(.badArgs("--title(\(title))과 --name(\(n))이 다릅니다. 보드는 탭 이름을 세션 이름으로 씁니다. 하나만 주거나 같게 주세요"))
    }
    var out = argv
    var sessionId: String?, resumes = false
    if let r = info.resume, isSessionID(r), !info.fork {
        sessionId = r.lowercased()
        resumes = true
    } else if let s = info.sessionFlag, isSessionID(s) {
        sessionId = s.lowercased()
    } else if info.resume == nil, !info.picker, !info.continues, info.sessionFlag == nil {
        out.insert(contentsOf: ["--session-id", newID], at: 1)
        sessionId = newID
    }
    // 인자마다 따옴표로 묶어 잇고, 다시 읽어 그 인자 그대로가 아니면 거절한다(보안 검토 고침 — 셸이 다르게 읽을 글자를 치지 않게)
    guard let command = shellCommand(out) else { return .failure(openTabUnreadable) }
    return .success(OpenTabPlanInput(argv: out, sessionId: sessionId, resumes: resumes, name: info.name ?? title, command: command))
}

private let openTabUnreadable = OpenTabFailure.refused("셸에 칠 명령이 요청한 인자 그대로 읽히지 않아 띄우지 않습니다")

/// 보드가 그 탭 셸에 칠 명령 — 요청 인자(openTabPrepare 의 argv)에서 답변 꾸미기(md-render)의 --plugin-dir 을 빼고, 켜져 있으면(mdRenderDir)
/// 보드 경로를 끝에 붙여 인자마다 묶는다. 글자 위에서 고치지 않고 인자 목록에서 고친 뒤 묶으며, 다시 읽어 그 인자 그대로가 아니면 거절한다
public func openTabCommand(_ argv: [String], mdRenderDir: String?) -> Result<String, OpenTabFailure> {
    let drop = mdRenderArgIndices(argv)
    let out = argv.enumerated().filter { !drop.contains($0.offset) }.map(\.element) + (mdRenderDir.map { ["--plugin-dir", $0] } ?? [])
    guard let command = shellCommand(out) else { return .failure(openTabUnreadable) }
    return .success(command)
}

// MARK: - 이미 있는 탭

/// 보드 탭의 화면 하나의 지금 값 — 앱이 BoardSurface 에서 떠 넘긴다
public struct OpenTabScreen: Equatable, Sendable {
    public let id: UUID
    public let sessionId: String?
    public let cwd: String          // 화면 셸의 지금 경로(앱이 바로 가기를 풀어 넘긴다)
    public let pid: Int32?          // 짝지은 세션의 claude pid
    public let names: [String]      // 이 화면 claude 의 이름들(지금 이름·바꿀 이름·방금 붙인 이름·명령의 --name·한 화면 탭의 탭 이름)
    public let busy: Bool           // claude(또는 셸 위의 다른 프로그램)가 돌거나 막 넣는 중. 셸만 남은 화면·다시 뜬 뒤 아직 들어가지 않은 화면은 false
    public let takeover: Bool       // 자율주행을 이어받은 화면 — 다른 명령을 넣지 않는다
    public init(id: UUID, sessionId: String?, cwd: String, pid: Int32?, names: [String], busy: Bool, takeover: Bool) {
        self.id = id; self.sessionId = sessionId; self.cwd = cwd; self.pid = pid; self.names = names; self.busy = busy; self.takeover = takeover
    }
}

/// 보드 밖에서 도는 claude(살아 있는 것만) — 같은 세션·이름이 보드 탭 밖에서 돌면 두 번째 claude 를 띄우지 않는다
public struct LiveClaude: Equatable, Sendable {
    public let pid: Int32
    public let session: String?
    public let name: String?
    public init(pid: Int32, session: String?, name: String?) { self.pid = pid; self.session = session; self.name = name }
}

public enum OpenTabPlan: Equatable, Sendable {
    case open                        // 새 탭
    case exists(UUID)                // 이미 도는 화면 — 앞으로만
    case reuse(UUID)                 // 같은 세션·이름의 화면에 claude 가 없다(셸만 남음·되살린 뒤 들어가지 않음) — 그 화면에 띄운다
    case refuse(String)
}

/// 같은 세션 id(먼저)나 같은 이름(대소문자 무시)의 보드 탭 화면이 있으면 그 화면으로(claude 가 돌면 앞으로만, 셸만 남았고 같은 폴더면 그 화면에 띄움 —
/// 보드의 끝난 세션 이어 열기(promptScreen)처럼. 다른 폴더의 셸이면 새 탭).
/// 없는데 같은 세션·이름의 claude 가 살아 있으면 그 claude 가 든 보드 탭 화면을 앞으로, 보드 탭 밖이면 거절
public func planOpenTab(sessionId: String?, name: String?, cwd: String, screens: [OpenTabScreen], live: [LiveClaude]) -> OpenTabPlan {
    let key = name?.lowercased()
    let match = screens.first { s in sessionId != nil && s.sessionId == sessionId }
        ?? screens.first { s in key != nil && s.names.contains { $0.lowercased() == key } }
    if let s = match {
        if s.busy || s.takeover { return .exists(s.id) }
        if s.cwd == cwd { return .reuse(s.id) }
    }
    // 화면 값에 아직 없는 이름·세션(짝짓기는 1초마다)도 살아 있는 claude 로 본다 — 그 pid 가 보드 탭 화면의 claude 면 그 화면, 아니면 거절
    for c in live {
        let bySession = sessionId != nil && c.session == sessionId
        guard bySession || (key != nil && c.name?.lowercased() == key) else { continue }
        if let s = screens.first(where: { $0.pid == c.pid }) { return .exists(s.id) }
        return .refuse(bySession ? "이 세션이 보드 탭 밖(pid \(c.pid))에서 돌고 있습니다"
                                 : "같은 이름(\(c.name ?? ""))의 세션이 보드 탭 밖(pid \(c.pid))에서 돌고 있습니다")
    }
    return .open
}
