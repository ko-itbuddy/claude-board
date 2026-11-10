// 바깥에서 보드 탭 닫기(ClaudeBoard --close-tab) 규칙 — 명령줄 읽기, 요청·답 꼴, 요청 폴더, 닫을 화면 찾기와 판정.
// 바깥 열기(--open-tab, OpenTab.swift)로 연 역할 탭을 그 일이 끝난 뒤 닫는 입구다. 열기와 같은 요청·답 꼴이지만 폴더·알림은 따로다
// (열기 처리부는 폴더의 *.req.json 을 모두 열기 요청으로 읽으므로 섞으면 먹어 버린다).
// 범위: 바깥 열기로 연 탭(앱의 BoardTab.external 표지)이고 화면이 하나인 탭만 닫는다. 사람이 연 탭·나눈 탭·자율주행 이어받기 화면은 거절하고,
// 보드 탭 밖의 claude 는 찾지도 않는다(신호를 보내지 않는다). 셸 위에서 claude 가 돌면 --stop 일 때만 그 화면에 /exit 를 넣어 끝낸 뒤 닫는다.
// 파일·시각·프로세스는 앱이 읽어 값으로 넘긴다(이 파일은 Foundation 만)
import Foundation

// MARK: - 명령줄

/// 닫을 대상 — 세션 id(화면의 지금 세션 id) 또는 탭 이름(그 화면의 이름들, 대소문자 무시)
public enum CloseTabTarget: Equatable, Sendable {
    case session(String)
    case title(String)
}

/// ClaudeBoard --close-tab 의 옵션
public struct CloseTabOptions: Equatable, Sendable {
    public var target: CloseTabTarget
    public var stop: Bool          // claude 가 돌면 /exit 로 끝낸 뒤 닫는다(없으면 busy)
    public var timeout: Double     // 보드의 답을 기다리는 초
    public init(target: CloseTabTarget, stop: Bool, timeout: Double) {
        self.target = target; self.stop = stop; self.timeout = timeout
    }
}

public let closeTabUsage = "쓰는 법: ClaudeBoard --close-tab (--session <세션 id> | --title <탭 이름>) [--stop] [--timeout <초>]"

/// 답을 기다리는 기본 초 — --stop 의 끝나기 기다림(closeTabStopWait)보다 넉넉히
public let closeTabDefaultTimeout: Double = 30
/// --stop 으로 /exit 를 넣은 뒤 claude 가 끝나기를 기다리는 상한(초). 넘기면 닫지 않고 busy — 강제 종료는 하지 않는다
public let closeTabStopWait: Double = 10

/// 대상 검사 — 세션 id 와 탭 이름 가운데 하나만. 세션 id 는 UUID 꼴(소문자로 고침), 탭 이름은 --open-tab 의 --title 과 같은 꼴.
/// 명령줄 도구와 보드(요청 파일을 다시 본다)가 함께 쓴다
public func closeTabTarget(session: String?, title: String?) -> Result<CloseTabTarget, OpenTabFailure> {
    switch (session, title) {
    case (nil, nil): return .failure(.badArgs("--session 이나 --title 가운데 하나가 필요합니다. \(closeTabUsage)"))
    case (.some, .some): return .failure(.badArgs("--session 과 --title 은 하나만 줍니다. \(closeTabUsage)"))
    case (.some(let s), nil):
        guard isSessionID(s) else { return .failure(.badArgs("--session 은 세션 id(UUID) 꼴이어야 합니다: \(s)")) }
        return .success(.session(s.lowercased()))
    case (nil, .some(let t)):
        guard tabTitleOK(t) else { return .failure(.badArgs("--title 은 한 줄, 100자까지입니다")) }
        return .success(.title(t))
    }
}

/// --close-tab 뒤의 인자를 읽는다. 값이 있는 옵션은 다음 인자 또는 --옵션=값(다음 인자가 -- 로 시작하면 값이 없는 것으로 본다 —
/// `--title --stop` 이 "--stop" 이라는 탭을 닫지 않게)
public func parseCloseTabArgs(_ args: [String]) -> Result<CloseTabOptions, OpenTabFailure> {
    var session: String?, title: String?, stop = false, timeout: Double?
    var i = 0
    while i < args.count {
        let a = args[i]
        let flag = a.split(separator: "=", maxSplits: 1).first.map(String.init) ?? a
        let inline: String? = a.contains("=") && a.hasPrefix("--") ? String(a.dropFirst(flag.count + 1)) : nil
        func take() -> Result<String, OpenTabFailure> {
            if let inline { return .success(inline) }
            guard i + 1 < args.count, !args[i + 1].hasPrefix("--") else { return .failure(.badArgs("\(flag) 뒤에 값이 없습니다. \(closeTabUsage)")) }
            i += 1
            return .success(args[i])
        }
        switch flag {
        case "--session":
            switch take() { case .success(let v): session = v; case .failure(let e): return .failure(e) }
        case "--title":
            switch take() { case .success(let v): title = v; case .failure(let e): return .failure(e) }
        case "--timeout":
            switch take() {
            case .success(let v):
                guard let t = Double(v), t.isFinite, t >= 1, t <= 600 else { return .failure(.badArgs("--timeout 은 1~600 초입니다: \(v)")) }
                timeout = t
            case .failure(let e): return .failure(e)
            }
        case "--stop":
            guard inline == nil else { return .failure(.badArgs("--stop 은 값을 받지 않습니다")) }
            stop = true
        default:
            return .failure(.badArgs("모르는 옵션입니다: \(a). \(closeTabUsage)"))
        }
        i += 1
    }
    switch closeTabTarget(session: session, title: title) {
    case .success(let target): return .success(CloseTabOptions(target: target, stop: stop, timeout: timeout ?? closeTabDefaultTimeout))
    case .failure(let e): return .failure(e)
    }
}

// MARK: - 요청·답

/// 요청 파일(<요청 id>.req.json, 닫기 폴더) — 명령줄 도구가 쓰고 보드가 읽은 뒤 지운다. session 과 title 가운데 하나만 있다
public struct CloseTabRequest: Codable, Equatable, Sendable {
    public var id: String
    public var session: String?
    public var title: String?
    public var stop: Bool
    public var at: Double          // 만든 시각(1970 기준 초)
    public init(id: String, target: CloseTabTarget, stop: Bool, at: Double) {
        self.id = id
        switch target {
        case .session(let s): session = s; title = nil
        case .title(let t): session = nil; title = t
        }
        self.stop = stop; self.at = at
    }
}

// 답은 바깥 열기의 답(OpenTabReply)을 그대로 쓴다 — 키 ok·status·tab·session·pid·why, 한 줄·키 차례 고정.
// status: closed(닫음, --stop 이면 claude 를 끝낸 뒤) · not-found · busy · refused · bad-args · board-off · timeout

/// 요청 폴더 — 열기 폴더(open-tab) 옆의 close-tab. 설치판·시험 빌드가 따로다
public func closeTabDir(bundleID: String?, home: String) -> String {
    home + "/Library/Application Support/" + (bundleID == boardDevBundleID ? "Claude Board Dev" : "Claude Board") + "/close-tab"
}

/// 닫기 요청 하나의 상한 — 세션 id 나 100자 탭 이름이라 작다
public let closeTabMaxBytes = 16 * 1024

// MARK: - 판정

/// 보드 탭 화면 하나의 지금 모양(앞 프로그램)
public enum CloseTabForeground: Equatable, Sendable {
    case starting   // 셸이 아직 뜨는 중이거나 보드가 claude 명령을 막 넣음(셸이 뜨기를 기다림·넣은 지 3초 안)
    case prompt     // 셸 프롬프트(셸 위에 도는 프로그램이 없음)
    case claude     // 앞 프로그램이 살아 있는 claude
    case other      // 셸에서 띄운 다른 프로그램(앞 프로그램을 읽지 못한 때도)
}

/// 닫기 판정에 넘길 화면 값 — 앱이 BoardSurface·BoardTab 에서 떠 넘긴다
public struct CloseTabScreen: Equatable, Sendable {
    public let id: UUID
    public let tab: String          // 탭 이름(답의 tab)
    public let sessionId: String?   // 화면의 지금 세션 id(짝지은 세션, 없으면 화면이 기억한 세션 id)
    public let names: [String]      // 바깥 열기의 이름 찾기와 같은 이름들(화면 하나인 탭만 탭 이름이 든다)
    public let external: Bool       // 바깥 열기로 연 탭(BoardTab.external)
    public let takeover: Bool       // 자율주행을 이어받은 화면
    public let split: Bool          // 탭의 화면이 둘 이상(사람이 나눈 탭)
    public let stopping: Bool       // 앞 닫기 요청(--stop)이 이 화면의 claude 를 끝내는 중
    public let foreground: CloseTabForeground
    public init(id: UUID, tab: String, sessionId: String?, names: [String], external: Bool, takeover: Bool, split: Bool, stopping: Bool,
                foreground: CloseTabForeground) {
        self.id = id; self.tab = tab; self.sessionId = sessionId; self.names = names; self.external = external; self.takeover = takeover
        self.split = split; self.stopping = stopping; self.foreground = foreground
    }
}

public enum CloseTabPlan: Equatable, Sendable {
    case close(UUID)                 // 셸 프롬프트 — 바로 닫는다
    case stopThenClose(UUID)         // --stop: 그 화면의 claude 에 /exit 를 넣고 끝나면 닫는다
    case notFound(String)
    case busy(UUID, String)          // 탭은 그대로
    case refuse(UUID?, String)       // 범위 밖 — 아무것도 닫지 않는다
}

/// 대상과 맞는 화면을 찾아 판정한다 — 세션 id 는 화면의 세션 id 와 같은 화면, 이름은 화면의 이름들과 대소문자 무시로 같은 화면.
/// 없으면 not-found, 둘 이상이면 거절(아무것도 닫지 않음), 하나면 closeTabDecide
public func planCloseTab(_ target: CloseTabTarget, stop: Bool, screens: [CloseTabScreen]) -> CloseTabPlan {
    let match: [CloseTabScreen]
    switch target {
    case .session(let id):
        match = screens.filter { $0.sessionId?.lowercased() == id.lowercased() }
    case .title(let t):
        let key = t.lowercased()
        match = screens.filter { $0.names.contains { $0.lowercased() == key } }
    }
    guard let s = match.first else {
        switch target {
        case .session(let id): return .notFound("이 세션의 보드 탭이 없습니다(이미 닫혔을 수 있습니다): \(id)")
        case .title(let t): return .notFound("이 이름의 보드 탭이 없습니다(이미 닫혔을 수 있습니다): \(t)")
        }
    }
    guard match.count == 1 else {
        let ids = match.map { $0.sessionId ?? "모름" }.joined(separator: ", ")
        return .refuse(nil, "여러 탭이 맞습니다(\(match.count)개, 세션 \(ids)). 아무것도 닫지 않았습니다. --session 으로 하나를 고르세요")
    }
    return closeTabDecide(s, stop: stop)
}

/// 맞는 화면 하나의 판정 — 이어받기·표지 없음·나눈 탭은 거절, 끝내는 중·막 띄우는 중·다른 프로그램은 busy,
/// 셸 프롬프트면 닫기, claude 면 --stop 일 때만 끝낸 뒤 닫기(없으면 busy). --stop 뒤 다시 볼 때도 부른다(stop false)
public func closeTabDecide(_ s: CloseTabScreen, stop: Bool) -> CloseTabPlan {
    if s.takeover { return .refuse(s.id, "자율주행을 이어받은 탭은 닫지 않습니다") }
    if !s.external {
        return .refuse(s.id, "바깥 열기(--open-tab)로 연 탭이 아니라 닫지 않습니다(사람이 연 탭, 또는 이 기능 전 판에서 연 탭은 손으로 닫습니다)")
    }
    if s.split { return .refuse(s.id, "화면을 나눈 탭은 닫지 않습니다") }
    if s.stopping { return .busy(s.id, "이미 이 탭의 claude 를 끝내는 중입니다") }
    switch s.foreground {
    case .starting: return .busy(s.id, "탭이 막 뜨는 중입니다(셸이 뜨는 중이거나 claude 명령을 막 넣음). 잠시 뒤 다시 하세요")
    case .prompt: return .close(s.id)
    case .claude: return stop ? .stopThenClose(s.id) : .busy(s.id, "claude 가 돌고 있습니다. --stop 을 주면 끝낸 뒤 닫습니다")
    case .other: return .busy(s.id, "셸에서 다른 프로그램이 돌고 있어 닫지 않습니다")
    }
}

/// 판정 결과의 답 — 닫기(close·stopThenClose)는 앱이 닫은 뒤 closed 로 답한다
public func closeTabReply(_ plan: CloseTabPlan, screens: [CloseTabScreen]) -> OpenTabReply? {
    func info(_ id: UUID?) -> (String?, String?) {
        guard let id, let s = screens.first(where: { $0.id == id }) else { return (nil, nil) }
        return (s.tab, s.sessionId)
    }
    switch plan {
    case .close, .stopThenClose: return nil
    case .notFound(let why): return OpenTabReply(ok: false, status: "not-found", why: why)
    case .busy(let id, let why):
        let (tab, session) = info(id)
        return OpenTabReply(ok: false, status: "busy", tab: tab, session: session, why: why)
    case .refuse(let id, let why):
        let (tab, session) = info(id)
        return OpenTabReply(ok: false, status: "refused", tab: tab, session: session, why: why)
    }
}
