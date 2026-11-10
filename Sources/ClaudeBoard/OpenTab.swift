// MARK: - 바깥에서 탭 열기

// 다른 프로그램(다른 Claude Code 세션의 스킬 등)이 보드 탭에 claude 를 띄우는 입구 — 규칙(인자 읽기·검사·이미 있는 탭 가리기)은 BoardRules/OpenTab.swift.
//   "/Applications/Claude Board.app/Contents/MacOS/ClaudeBoard" --open-tab --cwd <폴더> [--title <탭 이름>] [--launch] [--no-focus] [--timeout <초>] -- claude <인자…>
// --no-focus 면 보드를 앞으로 가져오지 않고 탭 선택·키보드 포커스도 그대로 둔 채 뒤에서 연다(reveal 을 부르지 않는다).
// 그 탭을 일이 끝난 뒤 닫는 입구(--close-tab)는 아래 "바깥에서 탭 닫기" 절
// 같은 실행 파일이 명령줄 도구로 돌며(openTabCommandLine) 요청을 사용자만 쓰는 폴더(openTabDir)에 <요청 id>.req.json 으로 남기고 분산 알림
// <instanceName>.open-tab 으로 떠 있는 보드를 깨운 뒤, 보드가 쓴 <요청 id>.reply.json 을 기다려 한 줄 JSON 으로 찍는다(성공 0, 실패 1).
// 알림에는 내용을 싣지 않는다 — 보드(OpenTabInbox)는 폴더와 파일이 이 사용자 것일 때만 읽는다. 보드는 보드가 연 탭과 같은 길(TerminalHub.run)로
// 띄우므로 세션 목록·알림·쥔 탭 목록(다시 뜰 때 되살리기)이 같다
import AppKit
import BoardRules
import Foundation

// 명령줄 도구 — 앱을 띄우지 않고 요청만 넘기고 끝난다(main.swift 맨 앞에서 부른다)
func openTabCommandLine(_ args: [String]) -> Never {
    func finish(_ r: OpenTabReply) -> Never {
        FileHandle.standardOutput.write(Data((r.line + "\n").utf8))
        exit(r.ok ? 0 : 1)
    }
    let fm = FileManager.default
    let opts: OpenTabOptions
    switch parseOpenTabArgs(args, here: fm.currentDirectoryPath) {
    case .success(let o): opts = o
    case .failure(let e): finish(e.reply)
    }
    let cwd = (opts.cwd as NSString).standardizingPath
    // 보드도 다시 본다 — 여기서는 보드를 깨우기 전에 빨리 알리려고
    if case .failure(let e) = openTabPrepare(cwd: cwd, title: opts.title, argv: opts.argv, newID: UUID().uuidString.lowercased()) { finish(e.reply) }
    var isDir: ObjCBool = false
    guard fm.fileExists(atPath: cwd, isDirectory: &isDir), isDir.boolValue else { finish(OpenTabFailure.badArgs("폴더가 없습니다: \(cwd)").reply) }

    // 이 실행 파일이 든 앱의 보드(설치판·시험 빌드)에 넘긴다 — 잠금 파일은 부른 쪽 TMPDIR 에 따라 자리가 달라질 수 있어 떠 있는 앱으로 본다
    let bundleID = Bundle.main.bundleIdentifier
    let instance = bundleID == boardDevBundleID ? "claude-board-dev" : "claude-board"
    let running = boardRunning(bundleID)
    let appURL = Bundle.main.bundleURL
    if !running {
        guard opts.launch else { finish(OpenTabFailure.boardOff("보드가 꺼져 있습니다. --launch 를 주면 켜고 엽니다").reply) }
        guard appURL.pathExtension == "app" else { finish(OpenTabFailure.boardOff("보드가 꺼져 있고 앱 안의 실행 파일이 아니라 켤 수 없습니다").reply) }
    }

    let dir = openTabDir(bundleID: bundleID, home: NSHomeDirectory())
    if let e = prepareRequestDir(dir) { finish(e.reply) }

    let id = UUID().uuidString.lowercased()
    // --no-focus 는 켤 때만 칸을 싣는다(끄면 nil — 3.3.3 까지와 같은 요청 JSON)
    let req = OpenTabRequest(id: id, cwd: cwd, title: opts.title, argv: opts.argv, at: Date().timeIntervalSince1970, noFocus: opts.noFocus ? true : nil)
    let reqPath = "\(dir)/\(id).req.json"
    guard let data = try? JSONEncoder().encode(req), writeRequest(dir: dir, id: id, data) else {
        finish(OpenTabFailure.refused("요청을 쓰지 못했습니다: \(dir)").reply)
    }

    if !running {
        // --no-focus 면 앞으로 가져오지 않고 켠다(open -g). 보드가 뜰 때 스스로 창을 앞으로 가져오는 동작은 그대로다
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        p.arguments = (opts.noFocus ? ["-g"] : []) + ["-a", appURL.path]
        do { try p.run(); p.waitUntilExit() } catch {}
        if p.terminationStatus != 0 {
            unlink(reqPath)
            finish(OpenTabFailure.boardOff("보드를 켜지 못했습니다(open 종료 코드 \(p.terminationStatus))").reply)
        }
    }

    finish(awaitReply(dir: dir, id: id, notification: "\(instance).open-tab", timeout: opts.timeout, taken: "탭이 열렸을 수 있습니다"))
}

// 요청 폴더를 만들고(0700) 이 사용자의 폴더인지 본다 — 아니면 그 까닭
private func prepareRequestDir(_ dir: String) -> OpenTabFailure? {
    do {
        try FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    } catch {
        return .refused("요청 폴더를 만들지 못했습니다: \(dir) \(error.localizedDescription)")
    }
    var st = stat()
    guard lstat(dir, &st) == 0, st.st_mode & S_IFMT == S_IFDIR, st.st_uid == getuid() else {
        return .refused("요청 폴더가 이 사용자의 폴더가 아닙니다: \(dir)")
    }
    if st.st_mode & 0o077 != 0 { chmod(dir, 0o700) }
    return nil
}

// 요청을 <id>.tmp 에 새로 쓰고 <id>.req.json 으로 바꾼다(보드가 쓰다 만 요청을 읽지 않게)
private func writeRequest(dir: String, id: String, _ data: Data) -> Bool {
    guard writeNew("\(dir)/\(id).tmp", data), rename("\(dir)/\(id).tmp", "\(dir)/\(id).req.json") == 0 else {
        unlink("\(dir)/\(id).tmp")
        return false
    }
    return true
}

// 답을 기다린다 — 알림은 1초마다 다시 보낸다(보드가 막 뜨는 중이면 앞 알림을 놓칠 수 있다. 보드는 되살린 뒤에도 폴더를 한 번 본다).
// 시간 안에 답이 없으면 보드가 아직 가져가지 않은 요청은 거둬 나중에 처리되지 않게 한다(taken: 가져갔지만 답이 없을 때 덧붙일 말)
private func awaitReply(dir: String, id: String, notification: String, timeout: Double, taken: String) -> OpenTabReply {
    let reqPath = "\(dir)/\(id).req.json", replyPath = "\(dir)/\(id).reply.json"
    let deadline = Date().addingTimeInterval(timeout)
    var posted = Date.distantPast
    while Date() < deadline {
        if let d = FileManager.default.contents(atPath: replyPath), let r = try? JSONDecoder().decode(OpenTabReply.self, from: d) {
            unlink(replyPath)
            return r
        }
        if Date().timeIntervalSince(posted) >= 1 {
            DistributedNotificationCenter.default().postNotificationName(.init(notification), object: nil, userInfo: nil, deliverImmediately: true)
            posted = Date()
        }
        usleep(100_000)
    }
    let withdrawn = unlink(reqPath) == 0
    return OpenTabFailure.timeout(withdrawn
        ? "보드가 \(Int(timeout))초 안에 요청을 가져가지 않았습니다(요청을 거뒀습니다)"
        : "보드가 요청을 가져갔지만 \(Int(timeout))초 안에 답하지 않았습니다(\(taken))").reply
}

// 떠 있는 보드(이 실행 파일이 든 앱 — 설치판·시험 빌드)가 있는가. 잠금 파일은 부른 쪽 TMPDIR 에 따라 자리가 달라질 수 있어 떠 있는 앱으로 본다
private func boardRunning(_ bundleID: String?) -> Bool {
    NSRunningApplication.runningApplications(withBundleIdentifier: bundleID ?? "com.claude.board")
        .contains { $0.processIdentifier != getpid() && !$0.isTerminated }
}

// 화면 경로와 요청 경로를 견줄 꼴 — 바로 가기를 풀고(/tmp → /private/tmp) 끝 빗금을 뗀다
func realDir(_ path: String) -> String { URL(fileURLWithPath: path).resolvingSymlinksInPath().path }

// 새 파일로만 쓴다(있거나 바로 가기면 실패) — 0600
private func writeNew(_ path: String, _ data: Data) -> Bool {
    let fd = open(path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0o600)
    guard fd >= 0 else { return false }
    defer { close(fd) }
    return data.withUnsafeBytes { write(fd, $0.baseAddress, $0.count) } == data.count
}

// 보드 쪽 요청 읽기(열기·닫기가 함께 씀) — 폴더가 이 사용자 것이고 다른 사용자가 쓸 수 없을 때만, <요청 id>.req.json 을 모두 읽고 지운다(한 번만).
// 이 사용자 것인 보통 파일만 읽고(바로 가기는 따라가지 않는다) 풀지 못한 요청·오래된 요청은 기록만 남기고 버린다.
// .req.json 이 아닌 파일(떠난 명령줄 도구가 남긴 답·쓰다 만 파일)은 10분 지나면 치운다. 만든 차례(at)로 돌려준다
@MainActor func takeRequests<R: Decodable>(_ type: R.Type, in dir: String, label: String, maxBytes: Int,
                                          id: (R) -> String, at: (R) -> Double) -> [R] {
    var st = stat()
    guard lstat(dir, &st) == 0, st.st_mode & S_IFMT == S_IFDIR, st.st_uid == getuid(), st.st_mode & 0o077 == 0 else { return [] }
    let now = Date().timeIntervalSince1970
    var reqs: [R] = []
    for f in (try? FileManager.default.contentsOfDirectory(atPath: dir)) ?? [] {
        let path = "\(dir)/\(f)"
        if !f.hasSuffix(".req.json") {
            if lstat(path, &st) == 0, now - Double(st.st_mtimespec.tv_sec) > 600 { unlink(path) }
            continue
        }
        let data = readOwned(path, maxBytes: maxBytes)
        unlink(path)   // 한 번만
        guard let data else { continue }   // 명령줄 도구가 그 사이 거뒀거나 이 사용자 것인 보통 파일이 아니다
        guard let r = try? JSONDecoder().decode(R.self, from: data), "\(id(r)).req.json" == f, isSessionID(id(r)) else {
            boardLog("\(label): 읽을 수 없는 요청을 버림 \(f)")
            continue
        }
        if openTabStale(at: at(r), now: now) { boardLog("\(label): 오래된 요청을 버림 \(f)"); continue }
        reqs.append(r)
    }
    return reqs.sorted { at($0) < at($1) }
}

// 이 사용자 것인 보통 파일만 읽는다(바로 가기는 따라가지 않는다, 크기 상한)
private func readOwned(_ path: String, maxBytes: Int) -> Data? {
    let fd = open(path, O_RDONLY | O_NOFOLLOW)
    guard fd >= 0 else { return nil }
    defer { close(fd) }
    var st = stat()
    guard fstat(fd, &st) == 0, st.st_mode & S_IFMT == S_IFREG, st.st_uid == getuid(), st.st_size <= off_t(maxBytes) else { return nil }
    return FileHandle(fileDescriptor: fd, closeOnDealloc: false).readDataToEndOfFile()
}

// 답을 <요청 id>.reply.json 으로 쓴다(명령줄 도구가 읽고 지운다)
private func writeReply(_ reply: OpenTabReply, dir: String, id: String) {
    if let d = try? JSONEncoder().encode(reply) { try? d.write(to: URL(fileURLWithPath: "\(dir)/\(id).reply.json"), options: .atomic) }
}

// 보드 쪽 — 요청 폴더를 읽어 탭을 연다. 분산 알림을 받을 때와 탭을 되살린 뒤(restoreTabs) 부른다
@MainActor final class OpenTabInbox {
    static let shared = OpenTabInbox()
    private let dir = openTabDir(bundleID: Bundle.main.bundleIdentifier, home: NSHomeDirectory())

    func drain() {
        let hub = TerminalHub.shared
        guard hub.isRestored, hub.area != nil else { return }   // 되살리기 전이면 restoreTabs 가 끝에 다시 부른다
        for r in takeRequests(OpenTabRequest.self, in: dir, label: "바깥 탭 열기", maxBytes: openTabMaxBytes * 2, id: \.id, at: \.at) {
            let reply = handle(r)
            boardLog("바깥 탭 열기: \(reply.status) \(reply.tab ?? "") \(reply.session ?? "") \(reply.why ?? "")\(r.reveals ? "" : " (뒤에서)") — \(r.cwd)")
            writeReply(reply, dir: dir, id: r.id)
        }
    }

    private func handle(_ r: OpenTabRequest) -> OpenTabReply {
        let hub = TerminalHub.shared
        let cwd = (r.cwd as NSString).standardizingPath
        let prep: OpenTabPlanInput
        switch openTabPrepare(cwd: cwd, title: r.title, argv: r.argv, newID: UUID().uuidString.lowercased()) {
        case .success(let p): prep = p
        case .failure(let e): return e.reply
        }
        var isDir: ObjCBool = false
        guard FileManager.default.fileExists(atPath: cwd, isDirectory: &isDir), isDir.boolValue else {
            return OpenTabFailure.badArgs("폴더가 없습니다: \(cwd)").reply
        }
        let off = isMdRenderOff(prep.sessionId)
        // 셸에 칠 명령은 요청 인자에서 고쳐 묶는다(답변 꾸미기를 빼고 붙이기) — 묶은 글자 위에서 고치지 않는다(BoardRules openTabCommand, 3.3.3 보안 고침)
        let command: String
        switch openTabCommand(prep.argv, mdRenderDir: off ? nil : mdRenderDir) {
        case .success(let c): command = c
        case .failure(let e): return e.reply
        }
        switch planOpenTab(sessionId: prep.sessionId, name: prep.name, cwd: realDir(cwd), screens: hub.openTabScreens,
                           live: liveClaudes(session: prep.sessionId)) {
        case .refuse(let why):
            return OpenTabFailure.refused(why).reply
        case .exists(let id):
            if r.reveals { hub.reveal(id) }   // --no-focus 면 답만 돌려주고 화면은 그대로
            let s = hub.surface(id)
            return OpenTabReply(ok: true, status: "exists", tab: s?.tab?.shownName, session: s?.session?.id ?? s?.sessionId, pid: s?.session?.pid)
        case .reuse(let id):
            if let why = noTranscript(prep) { return OpenTabFailure.refused(why).reply }
            guard let s = hub.surface(id) else { return OpenTabFailure.refused("탭이 그 사이 닫혔습니다").reply }
            // 화면 하나인 탭은 탭 이름을 세션 이름으로 맞춘다(다시 뜰 때 되살린 탭도 이 이름으로 이어 연다)
            if let n = prep.name, let t = s.tab, t.surfaces.count == 1 { t.customName = n }
            hub.run(command, in: s, sessionId: prep.sessionId, mdRenderOff: off)
            if r.reveals { hub.reveal(s.id) }
            return OpenTabReply(ok: true, status: "reopened", tab: s.tab?.name, session: prep.sessionId)
        case .open:
            if let why = noTranscript(prep) { return OpenTabFailure.refused(why).reply }
            guard let t = hub.openExternal(command, in: cwd, title: prep.name, sessionId: prep.sessionId, mdRenderOff: off, focus: r.reveals) else {
                return OpenTabFailure.refused("보드 터미널을 열 수 없습니다").reply
            }
            return OpenTabReply(ok: true, status: "opened", tab: t.name, session: prep.sessionId)
        }
    }

    // 이어 열 세션의 대화 기록이 어디에도 없다(요청을 한 번도 보내지 않고 끝난 세션 — 보드가 그 세션 기록도 치운다) — claude --resume 이
    // 바로 끝나므로 띄우지 않고 까닭을 돌려준다. 세션 기록의 transcript_path, 없으면 ~/.claude/projects 의 폴더마다 <세션 id>.jsonl 을 본다
    private func noTranscript(_ p: OpenTabPlanInput) -> String? {
        guard p.resumes, let id = p.sessionId else { return nil }
        let fm = FileManager.default
        if let t = sessionRecord(id)?["transcript_path"] as? String, !t.isEmpty, fm.fileExists(atPath: t) { return nil }
        let projects = NSHomeDirectory() + "/.claude/projects"
        if ((try? fm.contentsOfDirectory(atPath: projects)) ?? []).contains(where: { fm.fileExists(atPath: "\(projects)/\($0)/\(id).jsonl") }) { return nil }
        return "이어 열 대화 기록이 없습니다(요청을 한 번도 보내지 않았거나 지워진 세션): \(id)"
    }

    // 살아 있는 claude — 이름은 ~/.claude/sessions(pid → 이름), 요청한 세션은 세션 기록의 pid·~/.claude/sessions 의 sessionId 로
    private func liveClaudes(session: String?) -> [LiveClaude] {
        var out = ClaudeNames.shared.live().map { LiveClaude(pid: $0.key, session: nil, name: $0.value) }
        if let session {
            let pids = [(sessionRecord(session)?["pid"] as? NSNumber)?.int32Value, ClaudeNames.shared.pid(forSession: session)]
            for p in Set(pids.compactMap { $0 }) where isLiveClaude(p) { out.append(LiveClaude(pid: p, session: session, name: nil)) }
        }
        return out
    }
}

// MARK: - 바깥에서 탭 닫기

// 바깥 열기로 연 역할 탭을 그 일이 끝난 뒤 닫는 입구 — 규칙(인자 읽기·대상 찾기·판정)은 BoardRules/CloseTab.swift.
//   "/Applications/Claude Board.app/Contents/MacOS/ClaudeBoard" --close-tab (--session <세션 id> | --title <탭 이름>) [--stop] [--timeout <초>]
// 열기와 같은 요청·답 꼴이지만 요청 폴더(closeTabDir)·분산 알림(<instanceName>.close-tab)은 따로다(열기 처리부가 닫기 요청을 먹지 않게).
// 보드가 꺼져 있으면 켜지 않고 board-off(--launch 는 두지 않는다). 쥔 탭 목록(UserDefaults)은 명령줄 도구가 고치지 않는다.
// 닫기는 TerminalHub.close(confirm: false) — 확인 창을 띄우지 않고, 보드를 앞으로 가져오거나 다른 앱의 포커스를 빼앗지 않는다(reveal·showBoard 를 부르지 않음)
func closeTabCommandLine(_ args: [String]) -> Never {
    func finish(_ r: OpenTabReply) -> Never {
        FileHandle.standardOutput.write(Data((r.line + "\n").utf8))
        exit(r.ok ? 0 : 1)
    }
    let opts: CloseTabOptions
    switch parseCloseTabArgs(args) {
    case .success(let o): opts = o
    case .failure(let e): finish(e.reply)
    }
    let bundleID = Bundle.main.bundleIdentifier
    let instance = bundleID == boardDevBundleID ? "claude-board-dev" : "claude-board"
    guard boardRunning(bundleID) else { finish(OpenTabFailure.boardOff("보드가 꺼져 있습니다(닫으려고 보드를 켜지 않습니다)").reply) }

    let dir = closeTabDir(bundleID: bundleID, home: NSHomeDirectory())
    if let e = prepareRequestDir(dir) { finish(e.reply) }
    let id = UUID().uuidString.lowercased()
    let req = CloseTabRequest(id: id, target: opts.target, stop: opts.stop, at: Date().timeIntervalSince1970)
    guard let data = try? JSONEncoder().encode(req), writeRequest(dir: dir, id: id, data) else {
        finish(OpenTabFailure.refused("요청을 쓰지 못했습니다: \(dir)").reply)
    }
    finish(awaitReply(dir: dir, id: id, notification: "\(instance).close-tab", timeout: opts.timeout, taken: "탭이 닫혔을 수 있습니다"))
}

// 보드 쪽 — 닫기 요청 폴더를 읽어 바깥에서 연 탭을 닫는다. 분산 알림을 받을 때와 탭을 되살린 뒤(restoreTabs) 부른다.
// --stop 은 그 화면에 끝내기 단계(SessionSteps.exit)를 직접 넣고 끝나기를 기다린 뒤 닫으므로 답이 늦게 간다(비동기)
@MainActor final class CloseTabInbox {
    static let shared = CloseTabInbox()
    private let dir = closeTabDir(bundleID: Bundle.main.bundleIdentifier, home: NSHomeDirectory())
    private var stopping: Set<UUID> = []   // --stop 으로 claude 를 끝내는 중인 화면 — 같은 화면의 두 번째 닫기 요청은 busy

    func drain() {
        let hub = TerminalHub.shared
        guard hub.isRestored, hub.area != nil else { return }   // 되살리기 전이면 restoreTabs 가 끝에 다시 부른다
        let dir = dir
        for r in takeRequests(CloseTabRequest.self, in: dir, label: "바깥 탭 닫기", maxBytes: closeTabMaxBytes, id: \.id, at: \.at) {
            handle(r) { reply in
                boardLog("바깥 탭 닫기: \(reply.status) \(reply.tab ?? "") \(reply.session ?? "") \(reply.why ?? "")\(r.stop ? " (--stop)" : "")")
                writeReply(reply, dir: dir, id: r.id)
            }
        }
    }

    private func handle(_ r: CloseTabRequest, reply: @escaping @MainActor (OpenTabReply) -> Void) {
        let target: CloseTabTarget
        switch closeTabTarget(session: r.session, title: r.title) {
        case .success(let t): target = t
        case .failure(let e): return reply(e.reply)
        }
        let screens = TerminalHub.shared.closeTabScreens(stopping: stopping)
        let plan = planCloseTab(target, stop: r.stop, screens: screens)
        switch plan {
        case .close(let id): reply(close(id))
        case .stopThenClose(let id): stopThenClose(id, reply: reply)
        default: reply(closeTabReply(plan, screens: screens) ?? OpenTabFailure.refused("닫지 않았습니다").reply)
        }
    }

    // 셸 프롬프트로 남은 화면 하나인 탭을 닫는다(탭째 — 셸도 끝난다, 쥔 탭 목록도 고쳐진다)
    private func close(_ id: UUID) -> OpenTabReply {
        let hub = TerminalHub.shared
        guard let s = hub.surface(id), let info = hub.closeTabScreen(id, stopping: false) else {
            return OpenTabReply(ok: false, status: "not-found", why: "탭이 그 사이 닫혔습니다")
        }
        hub.close(s, confirm: false)
        return OpenTabReply(ok: true, status: "closed", tab: info.tab, session: info.sessionId)
    }

    // --stop: 그 화면의 claude 에 Esc → /exit → Enter 를 넣고(Store 의 세션 목록을 거치지 않는다 — 짝짓기가 1초 늦을 수 있음)
    // closeTabStopWait 초까지 끝나기를 기다린다. 끝나면 다시 보고 닫고, 끝나지 않으면 닫지 않고 busy. 강제 종료(신호)는 하지 않는다
    private func stopThenClose(_ id: UUID, reply: @escaping @MainActor (OpenTabReply) -> Void) {
        let hub = TerminalHub.shared
        guard let s = hub.surface(id), let pid = s.view.foregroundPid, pid > 0, let info = hub.closeTabScreen(id, stopping: false) else {
            return reply(OpenTabReply(ok: false, status: "busy", why: "끝낼 claude 를 찾지 못했습니다"))
        }
        stopping.insert(id)
        s.drive(SessionSteps.exit) { _ in
            waitExit(pid, timeout: closeTabStopWait) { exited in
                guard exited else {
                    self.stopping.remove(id)
                    return reply(OpenTabReply(ok: false, status: "busy", tab: info.tab, session: info.sessionId,
                                              why: "claude 가 \(Int(closeTabStopWait))초 안에 끝나지 않아 닫지 않았습니다(탭은 그대로 둡니다)"))
                }
                self.settle(id, info, tries: 20, reply: reply)
            }
        }
    }

    // claude 가 끝난 뒤 셸 프롬프트로 돌아오기를 잠깐(0.1초씩 tries 번) 기다리며 다시 판정한다 — 닫을 수 있으면 닫고, 끝까지 아니면 그 판정의 답
    private func settle(_ id: UUID, _ info: CloseTabScreen, tries: Int, reply: @escaping @MainActor (OpenTabReply) -> Void) {
        guard let now = TerminalHub.shared.closeTabScreen(id, stopping: false) else {
            stopping.remove(id)
            return reply(OpenTabReply(ok: false, status: "not-found", tab: info.tab, session: info.sessionId, why: "claude 를 끝내는 사이 탭이 닫혔습니다"))
        }
        let plan = closeTabDecide(now, stop: false)
        if case .close = plan {
            stopping.remove(id)
            return reply(close(id))
        }
        if tries > 0 {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) { self.settle(id, info, tries: tries - 1, reply: reply) }
            return
        }
        stopping.remove(id)
        reply(closeTabReply(plan, screens: [now]) ?? OpenTabReply(ok: false, status: "busy", tab: info.tab, session: info.sessionId,
                                                                  why: "claude 는 끝났지만 셸 프롬프트로 돌아오지 않아 닫지 않았습니다"))
    }
}
