// MARK: - 바깥에서 탭 열기

// 다른 프로그램(다른 Claude Code 세션의 스킬 등)이 보드 탭에 claude 를 띄우는 입구 — 규칙(인자 읽기·검사·이미 있는 탭 가리기)은 BoardRules/OpenTab.swift.
//   "/Applications/Claude Board.app/Contents/MacOS/ClaudeBoard" --open-tab --cwd <폴더> [--title <탭 이름>] [--launch] [--timeout <초>] -- claude <인자…>
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
    let running = NSRunningApplication.runningApplications(withBundleIdentifier: bundleID ?? "com.claude.board")
        .contains { $0.processIdentifier != getpid() && !$0.isTerminated }
    let appURL = Bundle.main.bundleURL
    if !running {
        guard opts.launch else { finish(OpenTabFailure.boardOff("보드가 꺼져 있습니다. --launch 를 주면 켜고 엽니다").reply) }
        guard appURL.pathExtension == "app" else { finish(OpenTabFailure.boardOff("보드가 꺼져 있고 앱 안의 실행 파일이 아니라 켤 수 없습니다").reply) }
    }

    let dir = openTabDir(bundleID: bundleID, home: NSHomeDirectory())
    do {
        try fm.createDirectory(atPath: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
    } catch {
        finish(OpenTabFailure.refused("요청 폴더를 만들지 못했습니다: \(dir) \(error.localizedDescription)").reply)
    }
    var st = stat()
    guard lstat(dir, &st) == 0, st.st_mode & S_IFMT == S_IFDIR, st.st_uid == getuid() else {
        finish(OpenTabFailure.refused("요청 폴더가 이 사용자의 폴더가 아닙니다: \(dir)").reply)
    }
    if st.st_mode & 0o077 != 0 { chmod(dir, 0o700) }

    let id = UUID().uuidString.lowercased()
    let req = OpenTabRequest(id: id, cwd: cwd, title: opts.title, argv: opts.argv, at: Date().timeIntervalSince1970)
    let reqPath = "\(dir)/\(id).req.json", replyPath = "\(dir)/\(id).reply.json"
    guard let data = try? JSONEncoder().encode(req), writeNew("\(dir)/\(id).tmp", data), rename("\(dir)/\(id).tmp", reqPath) == 0 else {
        unlink("\(dir)/\(id).tmp")
        finish(OpenTabFailure.refused("요청을 쓰지 못했습니다: \(dir)").reply)
    }

    if !running {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/open")
        p.arguments = ["-a", appURL.path]
        do { try p.run(); p.waitUntilExit() } catch {}
        if p.terminationStatus != 0 {
            unlink(reqPath)
            finish(OpenTabFailure.boardOff("보드를 켜지 못했습니다(open 종료 코드 \(p.terminationStatus))").reply)
        }
    }

    // 답을 기다린다 — 알림은 1초마다 다시 보낸다(보드가 막 뜨는 중이면 앞 알림을 놓칠 수 있다. 보드는 되살린 뒤에도 폴더를 한 번 본다)
    let deadline = Date().addingTimeInterval(opts.timeout)
    var posted = Date.distantPast
    while Date() < deadline {
        if let d = fm.contents(atPath: replyPath), let r = try? JSONDecoder().decode(OpenTabReply.self, from: d) {
            unlink(replyPath)
            finish(r)
        }
        if Date().timeIntervalSince(posted) >= 1 {
            DistributedNotificationCenter.default().postNotificationName(.init("\(instance).open-tab"), object: nil, userInfo: nil,
                                                                         deliverImmediately: true)
            posted = Date()
        }
        usleep(100_000)
    }
    // 기다림을 그만둔다 — 보드가 아직 가져가지 않은 요청은 거둬 나중에 뜨지 않게 한다
    let withdrawn = unlink(reqPath) == 0
    finish(OpenTabFailure.timeout(withdrawn
        ? "보드가 \(Int(opts.timeout))초 안에 요청을 가져가지 않았습니다(요청을 거뒀습니다)"
        : "보드가 요청을 가져갔지만 \(Int(opts.timeout))초 안에 답하지 않았습니다(탭이 열렸을 수 있습니다)").reply)
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

// 보드 쪽 — 요청 폴더를 읽어 탭을 연다. 분산 알림을 받을 때와 탭을 되살린 뒤(restoreTabs) 부른다
@MainActor final class OpenTabInbox {
    static let shared = OpenTabInbox()
    private let dir = openTabDir(bundleID: Bundle.main.bundleIdentifier, home: NSHomeDirectory())

    func drain() {
        let hub = TerminalHub.shared
        guard hub.isRestored, hub.area != nil else { return }   // 되살리기 전이면 restoreTabs 가 끝에 다시 부른다
        var st = stat()
        // 폴더가 이 사용자 것이고 다른 사용자가 쓸 수 없을 때만 — 아니면 요청을 믿지 않는다
        guard lstat(dir, &st) == 0, st.st_mode & S_IFMT == S_IFDIR, st.st_uid == getuid(), st.st_mode & 0o077 == 0 else { return }
        let fm = FileManager.default
        let now = Date().timeIntervalSince1970
        var reqs: [OpenTabRequest] = []
        for f in (try? fm.contentsOfDirectory(atPath: dir)) ?? [] {
            let path = "\(dir)/\(f)"
            if !f.hasSuffix(".req.json") {
                // 떠난 명령줄 도구가 남긴 답·쓰다 만 파일 — 10분 지나면 치운다
                if lstat(path, &st) == 0, now - Double(st.st_mtimespec.tv_sec) > 600 { unlink(path) }
                continue
            }
            let data = readOwned(path)
            unlink(path)   // 한 번만
            guard let data else { continue }   // 명령줄 도구가 그 사이 거뒀거나 이 사용자 것인 보통 파일이 아니다
            guard let r = try? JSONDecoder().decode(OpenTabRequest.self, from: data), "\(r.id).req.json" == f, isSessionID(r.id) else {
                boardLog("바깥 탭 열기: 읽을 수 없는 요청을 버림 \(f)")
                continue
            }
            if openTabStale(at: r.at, now: now) { boardLog("바깥 탭 열기: 오래된 요청을 버림 \(f)"); continue }
            reqs.append(r)
        }
        for r in reqs.sorted(by: { $0.at < $1.at }) {
            let reply = handle(r)
            boardLog("바깥 탭 열기: \(reply.status) \(reply.tab ?? "") \(reply.session ?? "") \(reply.why ?? "") — \(r.cwd)")
            if let d = try? JSONEncoder().encode(reply) { try? d.write(to: URL(fileURLWithPath: "\(dir)/\(r.id).reply.json"), options: .atomic) }
        }
    }

    // 이 사용자 것인 보통 파일만 읽는다(바로 가기는 따라가지 않는다)
    private func readOwned(_ path: String) -> Data? {
        let fd = open(path, O_RDONLY | O_NOFOLLOW)
        guard fd >= 0 else { return nil }
        defer { close(fd) }
        var st = stat()
        guard fstat(fd, &st) == 0, st.st_mode & S_IFMT == S_IFREG, st.st_uid == getuid(), st.st_size <= off_t(openTabMaxBytes * 2) else { return nil }
        return FileHandle(fileDescriptor: fd, closeOnDealloc: false).readDataToEndOfFile()
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
        let command = mdRenderCommand(prep.command, off: off)
        switch planOpenTab(sessionId: prep.sessionId, name: prep.name, cwd: realDir(cwd), screens: hub.openTabScreens,
                           live: liveClaudes(session: prep.sessionId)) {
        case .refuse(let why):
            return OpenTabFailure.refused(why).reply
        case .exists(let id):
            hub.reveal(id)
            let s = hub.surface(id)
            return OpenTabReply(ok: true, status: "exists", tab: s?.tab?.shownName, session: s?.session?.id ?? s?.sessionId, pid: s?.session?.pid)
        case .reuse(let id):
            if let why = noTranscript(prep) { return OpenTabFailure.refused(why).reply }
            guard let s = hub.surface(id) else { return OpenTabFailure.refused("탭이 그 사이 닫혔습니다").reply }
            // 화면 하나인 탭은 탭 이름을 세션 이름으로 맞춘다(다시 뜰 때 되살린 탭도 이 이름으로 이어 연다)
            if let n = prep.name, let t = s.tab, t.surfaces.count == 1 { t.customName = n }
            hub.run(command, in: s, sessionId: prep.sessionId, mdRenderOff: off)
            hub.reveal(s.id)
            return OpenTabReply(ok: true, status: "reopened", tab: s.tab?.name, session: prep.sessionId)
        case .open:
            if let why = noTranscript(prep) { return OpenTabFailure.refused(why).reply }
            guard let t = hub.openExternal(command, in: cwd, title: prep.name, sessionId: prep.sessionId, mdRenderOff: off) else {
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
