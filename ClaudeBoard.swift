// Claude Code 세션 보드 — 작업 경로별 카드로 세션 상태를 보여 주는 네이티브 창
// 상태 파일: ~/.claude/notifier/sessions/*.json (state.py 가 hook 에서 기록)
import AppKit
import Carbon.HIToolbox
import Combine
import CryptoKit
import GhosttyTerminal
import GhosttyTheme
import SwiftUI

// MARK: - 데이터

// 파일 수정 시각 — FileManager.attributesOfItem 은 확장 속성까지 읽어 느리므로(1초 주기 reload 의 가장 큰 몫이었다) stat 만 부른다
func mtime(_ path: String) -> Date? {
    var st = stat()
    guard stat(path, &st) == 0 else { return nil }
    return Date(timeIntervalSince1970: TimeInterval(st.st_mtimespec.tv_sec) + TimeInterval(st.st_mtimespec.tv_nsec) / 1e9)
}

struct Session: Identifiable, Equatable {
    let id: String
    let cwd: String
    let state: String
    let since: Date
    let updated: Date
    let headless: Bool
    let tty: String
    let topic: String
    let message: String
    let resumeCmd: String
    let file: String
    let ctx: Ctx?
    let brain: Brain?       // 마지막 응답의 모델·effort
    let pid: Int32
    let subagents: [Sub]
    let shells: Int         // 세션에 붙어 도는 백그라운드 셸 수
    let paused: Bool        // 보드에서 일시중지한 세션 — sessions/<id>.paused 가 있다
    let muted: Bool         // 보드에서 알림을 끈 세션 — sessions/<id>.mute 가 있으면 notify.py 가 패널을 띄우지 않는다
    let limitAt: Date?      // 사용 한도에 걸려 응답이 끊긴 시각 — state.py 가 기록하고, 세션이 다시 움직이면 지운다

    var rank: Int { ["waiting": 0, "working": 1, "paused": 2, "idle": 2, "ended": 3][state] ?? 9 }
    var title: String { topic.isEmpty ? (message.isEmpty ? "요청 전" : message) : topic }
}

// 세션이 호출한 서브에이전트 — state.py 가 SubagentStart·SubagentStop·도구 hook 으로 기록한다
struct Sub: Identifiable, Equatable {
    let id: String
    let type: String
    let desc: String
    let state: String       // running | waiting | done
    let started: Date
    let ended: Date?
    let tool: String
    let background: Bool
    var brain: Brain? = nil  // 서브에이전트 대화 기록의 마지막 응답 모델·effort

    // 보드 상태 이름으로 바꿔 아이콘·색을 세션과 같이 쓴다
    var boardState: String { ["running": "working", "waiting": "waiting", "done": "idle"][state] ?? "working" }

    static func parse(_ j: [String: Any]?, alive: Bool, now: Date) -> [Sub] {
        guard alive, let j else { return [] }
        return j.compactMap { id, v -> Sub? in
            guard let v = v as? [String: Any] else { return nil }
            let ended = (v["ended"] as? Double).map { Date(timeIntervalSince1970: $0) }
            // 끝난 서브에이전트는 10초 동안만 보여 준다
            if let ended, now.timeIntervalSince(ended) > 10 { return nil }
            return Sub(id: id,
                       type: v["type"] as? String ?? "",
                       desc: v["description"] as? String ?? "",
                       state: v["state"] as? String ?? "running",
                       started: Date(timeIntervalSince1970: v["started"] as? Double ?? 0),
                       ended: ended,
                       tool: v["tool"] as? String ?? "",
                       background: v["background"] as? Bool ?? false)
        }
        .sorted { ($0.state == "done" ? 1 : 0, $0.started) < ($1.state == "done" ? 1 : 0, $1.started) }
    }
}

struct Group: Identifiable, Equatable {
    let id: String          // cwd
    let sessions: [Session]
    var name: String { (id as NSString).lastPathComponent }
    var path: String { id.replacingOccurrences(of: NSHomeDirectory(), with: "~") }
    var urgent: String { sessions.min { $0.rank < $1.rank }?.state ?? "idle" }
    // 작업 중·승인 대기 세션 — sessions 는 급한 순이라 첫째가 대표다
    var active: [Session] { sessions.filter { $0.state == "waiting" || $0.state == "working" } }
}

// 세션이 지금 하는 일 — 승인 대기면 요청 내용, 아니면 세션 제목. 접힌 카드와 미니 화면이 쓴다
func doing(_ s: Session) -> String { s.state == "waiting" && !s.message.isEmpty ? s.message : s.title }

// 컨텍스트 사용량: 마지막 요청에 실린 입력 토큰 / 모델의 컨텍스트 크기
struct Ctx: Equatable {
    let used: Int
    let size: Int
    var pct: Int { size > 0 ? min(100, used * 100 / size) : 0 }
}

// 응답을 만든 모델과 effort — 대화 기록의 assistant 줄에 message.model 과 effort 가 실린다
struct Brain: Equatable {
    let model: String       // claude-opus-5-5 등
    let effort: String      // low | medium | high | xhigh | max, 모르면 ""

    // 모델 계열 이름 — 라벨에 짧게 쓴다
    var family: String {
        for f in ["opus", "sonnet", "haiku", "fable"] where model.contains(f) { return f.prefix(1).uppercased() + f.dropFirst() }
        return model.replacingOccurrences(of: "claude-", with: "")
    }
    // 계열 뒤 버전 숫자 — claude-opus-5-5 → 5.5, claude-fable-5-1 → 5.1
    var version: String {
        guard let r = model.range(of: "-(\\d+)-(\\d+)", options: .regularExpression) else { return "" }
        return model[r].dropFirst().replacingOccurrences(of: "-", with: ".")
    }
    static let efforts = ["low", "medium", "high", "xhigh", "max"]
    var effortLevel: Int { (Self.efforts.firstIndex(of: effort) ?? -1) + 1 }   // 0 = 모름
}

// 대화 기록 끝에서 마지막 응답의 모델·effort 를 읽는다 — 파일이 바뀌어도 3초에 한 번만 다시 읽는다
final class ModelReader {
    private var parsed: [String: (mtime: Date, checked: Date, brain: Brain?)] = [:]

    func read(_ path: String, fallbackModel: String = "", now: Date) -> Brain? {
        let changed = mtime(path) ?? .distantPast
        if let c = parsed[path], c.mtime == changed || now.timeIntervalSince(c.checked) < 3 {
            return c.brain ?? (fallbackModel.isEmpty ? nil : Brain(model: fallbackModel, effort: ""))
        }
        var brain: Brain? = nil
        if let h = FileHandle(forReadingAtPath: path) {
            let end = (try? h.seekToEnd()) ?? 0
            try? h.seek(toOffset: end > 400_000 ? end - 400_000 : 0)
            let data = (try? h.readToEnd()) ?? Data()
            try? h.close()
            let marker = Data("\"assistant\"".utf8)
            for line in data.split(separator: UInt8(ascii: "\n")).reversed() where line.range(of: marker) != nil {
                guard let j = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any],
                      j["type"] as? String == "assistant",
                      let msg = j["message"] as? [String: Any],
                      let m = msg["model"] as? String, m != "<synthetic>" else { continue }
                brain = Brain(model: m, effort: j["effort"] as? String ?? "")
                break
            }
        }
        parsed[path] = (changed, now, brain)
        return brain ?? (fallbackModel.isEmpty ? nil : Brain(model: fallbackModel, effort: ""))
    }

    // 목록에서 사라진 기록은 잊는다
    func prune(keep: Set<String>) { parsed = parsed.filter { keep.contains($0.key) } }
}

// 세션별 컨텍스트 사용량을 읽는다.
// 1순위: 상태줄 스크립트가 남긴 ~/.claude/notifier/context/<세션 ID>.json (Claude Code 화면의 값과 같다)
// 2순위: 상태줄이 없는 백그라운드 세션은 대화 기록 끝의 마지막 응답 usage 로 계산한다
final class ContextReader {
    private let dir = (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/context")
    private var parsed: [String: (mtime: Date, checked: Date, ctx: Ctx?)] = [:]
    // 상태줄에서 배운 모델별 컨텍스트 크기 — 보드를 다시 켜도 쓰도록 파일에 남긴다
    private let windowsPath = (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/context-windows.json")
    private var windows: [String: Int] = [:]

    init() {
        if let data = FileManager.default.contents(atPath: windowsPath),
           let j = try? JSONSerialization.jsonObject(with: data) as? [String: Int] { windows = j }
    }

    private func learn(_ model: String, _ size: Int) {
        guard windows[model] != size else { return }
        windows[model] = size
        if let data = try? JSONSerialization.data(withJSONObject: windows) {
            try? data.write(to: URL(fileURLWithPath: windowsPath), options: .atomic)
        }
    }

    // 배운 값이 없을 때의 기본 크기 — 5 세대 모델(Opus·Sonnet·Haiku 5.5, Fable 5.1)은 1M(2026-10-14 실측), 그 밖은 200K
    private func defaultWindow(_ model: String) -> Int {
        if model.contains("[1m]") { return 1_000_000 }
        let oneM = ["claude-opus-5", "claude-sonnet-5", "claude-haiku-5", "claude-fable-5"]
        return oneM.contains { model.hasPrefix($0) } ? 1_000_000 : 200_000
    }

    // 상태줄 기록 — 매초 세션마다 읽어 풀던 것을, 파일이 바뀔 때만 다시 읽는다
    private var lines: [String: (mtime: Date, j: [String: Any]?)] = [:]

    private func statusLine(_ sid: String) -> [String: Any]? {
        let path = (dir as NSString).appendingPathComponent("\(sid).json")
        guard let m = mtime(path) else { lines[sid] = nil; return nil }
        if let c = lines[sid], c.mtime == m { return c.j }
        let j = FileManager.default.contents(atPath: path).flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
        lines[sid] = (m, j)
        return j
    }

    func read(_ sid: String, transcript: String, model: String, now: Date) -> Ctx? {
        let tMtime = mtime(transcript) ?? .distantPast
        if let j = statusLine(sid),
           let cw = j["context_window"] as? [String: Any],
           let size = (cw["context_window_size"] as? NSNumber)?.intValue, size > 0 {
            if let m = j["model"] as? String, !m.isEmpty { learn(m, size) }
            let at = Date(timeIntervalSince1970: (j["at"] as? NSNumber)?.doubleValue ?? 0)
            // 상태줄 기록 뒤에 대화가 더 진행됐으면(백그라운드로 다시 열린 경우 등) 대화 기록 쪽을 쓴다
            if tMtime.timeIntervalSince(at) < 60 {
                let u = cw["current_usage"] as? [String: Any] ?? [:]
                let used = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                    .reduce(0) { $0 + ((u[$1] as? NSNumber)?.intValue ?? 0) }
                if used > 0 { return Ctx(used: used, size: size) }
                if let p = (cw["used_percentage"] as? NSNumber)?.intValue { return Ctx(used: size * p / 100, size: size) }
            }
        }
        return fromTranscript(transcript, mtime: tMtime, model: model, now: now)
    }

    // 상태줄이 남긴 세션의 현재 effort — /effort 로 바꾸면 다음 응답 전에도 바로 바뀐다. 컨텍스트와 같은 기준으로 신선할 때만 쓴다
    func liveEffort(_ sid: String, transcript: String) -> String? {
        guard let j = statusLine(sid), let e = j["effort"] as? String, !e.isEmpty else { return nil }
        let tMtime = mtime(transcript) ?? .distantPast
        let at = Date(timeIntervalSince1970: (j["at"] as? NSNumber)?.doubleValue ?? 0)
        return tMtime.timeIntervalSince(at) < 60 ? e : nil
    }

    private func fromTranscript(_ path: String, mtime: Date, model: String, now: Date) -> Ctx? {
        if let c = parsed[path], c.mtime == mtime || now.timeIntervalSince(c.checked) < 5 { return c.ctx }
        var used = 0, lastModel = model
        if let h = FileHandle(forReadingAtPath: path) {
            let end = (try? h.seekToEnd()) ?? 0
            try? h.seek(toOffset: end > 1_500_000 ? end - 1_500_000 : 0)
            let data = (try? h.readToEnd()) ?? Data()
            try? h.close()
            for line in data.split(separator: UInt8(ascii: "\n")).reversed() {
                guard let j = try? JSONSerialization.jsonObject(with: Data(line)) as? [String: Any],
                      j["type"] as? String == "assistant", (j["isSidechain"] as? Bool) != true,
                      let msg = j["message"] as? [String: Any], msg["model"] as? String != "<synthetic>",
                      let u = msg["usage"] as? [String: Any] else { continue }
                used = ["input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens"]
                    .reduce(0) { $0 + ((u[$1] as? NSNumber)?.intValue ?? 0) }
                if lastModel.isEmpty, let m = msg["model"] as? String { lastModel = m }
                break
            }
        }
        var ctx: Ctx? = nil
        if used > 0 {
            var size = windows[lastModel] ?? defaultWindow(lastModel)
            if used > size { size = 1_000_000 }
            ctx = Ctx(used: used, size: size)
        }
        parsed[path] = (mtime, now, ctx)
        return ctx
    }
}

// 요금제 사용 한도: 5시간·주간(전체)·주간(모델별 — 예: Fable)
struct Limit: Equatable, Identifiable {
    let id: String
    let label: String
    let pct: Double
    let resets: Date?

    // 간략 모드 이름 — 5시간 · 주간 · 모델 이름(Fable). 더 좁으면 5h · 7d · 첫 글자
    var short: String {
        id == "5h" ? "5시간" : id == "7d" ? "주간" : (label.components(separatedBy: "·").last ?? label).trimmingCharacters(in: .whitespaces)
    }
    var tiny: String { id == "5h" ? "5h" : id == "7d" ? "7d" : String(short.prefix(1)) }
}

final class UsageStore: ObservableObject {
    @Published var limits: [Limit] = []
    @Published var failed = false
    // 로그인 토큰이 만료된 채로 있다(계정을 막 바꿨거나 한동안 세션이 없었다) — 세션이 다음 요청을 보내며 갱신해야 사용량을 받을 수 있다
    @Published var tokenExpired = false
    // 상태줄 스크립트와 같은 캐시 파일을 함께 쓴다 — 사용량 API 호출을 줄인다
    private let cache = "/tmp/.claude-usage-cache.json"
    private var fetching = false
    private var generation = 0
    // 계정을 바꾼 직후 이 시각 전에 쓰인 캐시는 믿지 않는다 — 상태줄이 앞 계정 토큰으로 받던 값을 늦게 써 넣을 수 있다
    private var trustCacheAfter = Date.distantPast
    private var timer: Timer?

    init() {
        refresh()
        timer = Timer.scheduledTimer(withTimeInterval: 30, repeats: true) { [weak self] _ in self?.refresh() }
    }

    // 계정을 바꾼 뒤: 앞 계정 값이 든 캐시를 버리고 바로 새로 받는다 — 상태줄도 같은 캐시를 쓰므로 함께 새 계정 값이 된다
    func reset() {
        generation += 1
        trustCacheAfter = Date().addingTimeInterval(5)
        try? FileManager.default.removeItem(atPath: cache)
        failed = false
        withAnimation(.easeOut(duration: 0.2)) { limits = [] }
        refresh()
    }

    func refresh() {
        let written = mtime(cache).flatMap { $0 > trustCacheAfter ? $0 : nil }
        let age = Date().timeIntervalSince(written ?? .distantPast)
        if written != nil, let data = FileManager.default.contents(atPath: cache) { apply(data) }
        guard age > 120, !fetching else { return }
        fetching = true
        let gen = generation
        DispatchQueue.global(qos: .utility).async { [weak self] in
            guard let self else { return }
            let (data, expired) = Self.fetch()
            DispatchQueue.main.async {
                self.fetching = false
                // 받는 사이 계정을 바꿨으면 앞 계정 값이므로 버리고 다시 받는다
                guard gen == self.generation else { self.refresh(); return }
                self.tokenExpired = expired
                if let data, self.apply(data) {
                    try? data.write(to: URL(fileURLWithPath: self.cache), options: .atomic)
                    self.trustCacheAfter = .distantPast   // 새 계정 토큰으로 직접 받은 값이다
                    self.failed = false
                } else if self.limits.isEmpty {
                    self.failed = true
                }
            }
        }
    }

    // 키체인의 Claude Code 로그인 토큰으로 사용량 API 를 부른다 — 토큰은 메모리에만 두고 기록하지 않는다
    static func fetch() -> (data: Data?, expired: Bool) {
        guard let raw = Keychain.read(credentialsService()),
              let j = try? JSONSerialization.jsonObject(with: raw) as? [String: Any],
              let oauth = j["claudeAiOauth"] as? [String: Any],
              let token = oauth["accessToken"] as? String else { return (nil, false) }
        if let exp = (oauth["expiresAt"] as? NSNumber)?.doubleValue, exp / 1000 < Date().timeIntervalSince1970 { return (nil, true) }
        var req = URLRequest(url: URL(string: "https://api.anthropic.com/api/oauth/usage")!, timeoutInterval: 10)
        req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        req.setValue("oauth-2025-04-20", forHTTPHeaderField: "anthropic-beta")
        let sem = DispatchSemaphore(value: 0)
        var result: Data?
        URLSession.shared.dataTask(with: req) { data, resp, _ in
            if (resp as? HTTPURLResponse)?.statusCode == 200 { result = data }
            sem.signal()
        }.resume()
        _ = sem.wait(timeout: .now() + 12)
        return (result, false)
    }

    @discardableResult
    func apply(_ data: Data) -> Bool {
        guard let j = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let five = j["five_hour"] as? [String: Any] else { return false }
        var out: [Limit] = []
        func add(_ id: String, _ label: String, _ o: [String: Any]?, _ key: String) {
            guard let o, let v = (o[key] as? NSNumber)?.doubleValue else { return }
            out.append(Limit(id: id, label: label, pct: v, resets: isoDate(o["resets_at"] as? String)))
        }
        add("5h", "5시간", five, "utilization")
        add("7d", "주간 · 전체", j["seven_day"] as? [String: Any], "utilization")
        for l in j["limits"] as? [[String: Any]] ?? [] where l["kind"] as? String == "weekly_scoped" {
            let name = ((l["scope"] as? [String: Any])?["model"] as? [String: Any])?["display_name"] as? String ?? "모델"
            add("7d-\(name)", "주간 · \(name)", l, "percent")
        }
        if out != limits { withAnimation(.spring(response: 0.6, dampingFraction: 0.85)) { limits = out } }
        return true
    }
}

// 날짜 형식기는 만들기가 비싸다 — 매초 다시 그리는 줄마다 새로 만들지 않게 한 번만 만든다
private let isoFractional: ISO8601DateFormatter = {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return f
}()
private let isoPlain = ISO8601DateFormatter()

func isoDate(_ s: String?) -> Date? {
    guard let s else { return nil }
    return isoFractional.date(from: s) ?? isoPlain.date(from: s)
}

// MARK: - 계정 전환

// Claude Code 로그인 정보는 키체인 일반 암호 항목(서비스 "Claude Code-credentials", 계정 $USER)에 JSON 으로 있다.
// /usr/bin/security 로 읽고 쓴다 — Claude Code 도 security 로 만들고 고치므로 허용 창이 뜨지 않는다.
// 쓸 값은 명령줄 대신 표준 입력(security -i)으로 넘겨 ps 에 드러나지 않게 한다
enum Keychain {
    // Claude Code 와 같은 계정 이름 규칙
    static let user: String = {
        let u = ProcessInfo.processInfo.environment["USER"] ?? NSUserName()
        return u.range(of: "^[a-zA-Z0-9._-]+$", options: .regularExpression) != nil ? u : "claude-code-user"
    }()

    @discardableResult
    private static func security(_ args: [String], input: String? = nil) -> (status: Int32, out: Data) {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/usr/bin/security")
        p.arguments = args
        let out = Pipe(), inp = Pipe()
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        if input != nil { p.standardInput = inp }
        guard (try? p.run()) != nil else { return (-1, Data()) }
        if let input {
            inp.fileHandleForWriting.write(Data(input.utf8))
            try? inp.fileHandleForWriting.close()
        }
        let data = out.fileHandleForReading.readDataToEndOfFile()
        p.waitUntilExit()
        return (p.terminationStatus, data)
    }

    static func read(_ service: String, account: String = user) -> Data? {
        let r = security(["find-generic-password", "-a", account, "-s", service, "-w"])
        guard r.status == 0 else { return nil }
        var d = r.out
        while d.last == 0x0a { d.removeLast() }
        // ASCII 가 아닌 바이트가 섞인 값은 16진수로 나온다 — JSON 은 { 로 시작하므로 16진수 글자만 있으면 풀어서 돌려준다
        let hexDigits = Set("0123456789abcdef".utf8)
        if d.first != UInt8(ascii: "{"), d.count % 2 == 0, d.allSatisfy(hexDigits.contains) {
            let s = Array(d)
            d = Data(stride(from: 0, to: s.count, by: 2).map { UInt8(String(decoding: s[$0..<$0 + 2], as: UTF8.self), radix: 16)! })
        }
        return d.isEmpty ? nil : d
    }

    static func exists(_ service: String, account: String = user) -> Bool {
        security(["find-generic-password", "-a", account, "-s", service]).status == 0
    }

    static func write(_ service: String, account: String = user, _ data: Data) -> Bool {
        let hex = data.map { String(format: "%02x", $0) }.joined()
        let line = "add-generic-password -U -a \"\(account)\" -s \"\(service)\" -X \"\(hex)\"\n"
        // security -i 는 한 줄 4KB 까지 받는다 — 넘으면 Claude Code 처럼 명령줄로 넘긴다
        if line.utf8.count <= 4032 { return security(["-i"], input: line).status == 0 }
        return security(["add-generic-password", "-U", "-a", account, "-s", service, "-X", hex]).status == 0
    }

    static func delete(_ service: String, account: String = user) {
        security(["delete-generic-password", "-a", account, "-s", service])
    }

    // Claude Code 는 security -w 출력을 그대로 JSON 으로 읽는다 — 16진수로 나오지 않게 ASCII 밖 글자는 \uXXXX 로 쓴다
    static func asciiJSON(_ obj: Any) throws -> Data {
        let s = String(decoding: try JSONSerialization.data(withJSONObject: obj, options: [.withoutEscapingSlashes]), as: UTF8.self)
        var out = ""
        for u in s.utf16 {
            if u < 0x80 { out.unicodeScalars.append(Unicode.Scalar(UInt8(u))) } else { out += String(format: "\\u%04x", u) }
        }
        return Data(out.utf8)
    }

    static func json(_ service: String, account: String = user) -> [String: Any]? {
        read(service, account: account).flatMap { try? JSONSerialization.jsonObject(with: $0) as? [String: Any] }
    }
}

// Claude Code 의 로그인 정보 키체인 서비스 이름 — CLAUDE_CONFIG_DIR 을 정해 띄우면 "-<그 경로(NFC) sha256 앞 8자>" 가 붙는다 (2.1.300 실측)
func credentialsService(_ configDir: String? = nil) -> String {
    guard let dir = configDir else { return "Claude Code-credentials" }
    let hex = SHA256.hash(data: Data(dir.precomposedStringWithCanonicalMapping.utf8)).map { String(format: "%02x", $0) }.joined()
    return "Claude Code-credentials-\(hex.prefix(8))"
}

struct AccountError: LocalizedError {
    let errorDescription: String?
    init(_ m: String) { errorDescription = m }
}

// Claude Code 와 같은 잠금 — proper-lockfile 처럼 "<파일>.lock" 폴더를 만들어 잡는다. 15초 넘게 갱신되지 않은 잠금은 버려진 것으로 본다
func withFileLock<T>(_ file: String, _ body: () throws -> T) throws -> T {
    let lock = file + ".lock"
    let deadline = Date().addingTimeInterval(5)
    while mkdir(lock, 0o755) != 0 {
        guard errno == EEXIST else { throw AccountError("잠금을 만들지 못했습니다: \(lock)") }
        if let m = mtime(lock), Date().timeIntervalSince(m) > 15 { rmdir(lock); continue }
        guard Date() < deadline else { throw AccountError("Claude Code 가 로그인 정보를 쓰는 중입니다 — 잠시 뒤 다시 시도해 주세요") }
        Thread.sleep(forTimeInterval: 0.1)
    }
    defer { rmdir(lock) }
    return try body()
}

// JSON 설정 파일의 몇 키만 바꿔 쓴다 — Claude Code 와 같은 잠금 아래에서 읽고 고친 뒤 임시 파일을 거쳐 바꿔 넣는다. 권한(0600)은 그대로 둔다
func patchJSON(_ path: String, _ change: (inout [String: Any]) -> Void) throws {
    try withFileLock(path) {
        guard let data = FileManager.default.contents(atPath: path),
              var j = try JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            throw AccountError("\((path as NSString).lastPathComponent) 를 읽지 못했습니다")
        }
        change(&j)
        let out = try JSONSerialization.data(withJSONObject: j, options: [.prettyPrinted, .withoutEscapingSlashes])
        let perms = (try? FileManager.default.attributesOfItem(atPath: path)[.posixPermissions]) ?? 0o600
        try out.write(to: URL(fileURLWithPath: path), options: .atomic)
        try? FileManager.default.setAttributes([.posixPermissions: perms], ofItemAtPath: path)
    }
}

// 저장해 둔 Claude 계정 — 토큰은 키체인(서비스 "Claude Board-account", 계정 이름 = id)에만 두고 여기에는 화면에 보일 정보만 담는다
struct Account: Identifiable, Equatable {
    let id: String                  // accountUuid|organizationUuid — 같은 사람도 조직이 다르면 토큰이 따로다
    let oauth: [String: Any]        // ~/.claude.json 의 oauthAccount 그대로 — 전환할 때 이 값으로 바꿔 넣는다
    var refreshExpires: Date?       // 이 시각이 지나면 다시 로그인해야 한다

    init?(oauth: [String: Any], refreshExpires: Date? = nil) {
        guard let uuid = oauth["accountUuid"] as? String, !uuid.isEmpty else { return nil }
        id = uuid + "|" + (oauth["organizationUuid"] as? String ?? "")
        self.oauth = oauth
        self.refreshExpires = refreshExpires
    }

    var email: String { oauth["emailAddress"] as? String ?? "?" }
    var org: String { oauth["organizationName"] as? String ?? "" }
    // 요금제 — organizationType claude_max + 한도 등급 …_5x → "Max 5x"
    var plan: String {
        let t = (oauth["organizationType"] as? String ?? "").replacingOccurrences(of: "claude_", with: "")
        var p = t.prefix(1).uppercased() + t.dropFirst()
        if let tier = oauth["organizationRateLimitTier"] as? String,
           let r = tier.range(of: "_\\d+x$", options: .regularExpression) { p += " " + tier[r].dropFirst() }
        return p
    }
    var expired: Bool { refreshExpires.map { $0 < Date() } ?? false }

    static func == (a: Account, b: Account) -> Bool {
        a.id == b.id && a.email == b.email && a.org == b.org && a.plan == b.plan && a.refreshExpires == b.refreshExpires
    }
}

// 계정 전환: 계정마다 브라우저 로그인을 한 번 받아 토큰을 저장해 두고, 고르면 Claude Code 로그인 항목의 토큰만 바꿔 넣는다.
// 실행 중인 세션은 요청을 보낼 때마다 키체인을 다시 읽으므로(최대 30초 캐시) 모든 세션이 다음 요청부터 새 계정으로 이어 간다
final class AccountStore: ObservableObject {
    struct Notice: Equatable { let text: String; let error: Bool }

    @Published private(set) var current: Account?          // 지금 로그인된 계정 — ~/.claude.json 의 oauthAccount
    @Published private(set) var saved: [Account] = []      // 전환할 수 있게 저장해 둔 계정
    @Published private(set) var adding = false              // 계정 추가 — 브라우저 로그인을 기다리는 중
    @Published private(set) var switching = false
    @Published private(set) var notice: Notice?
    // 사용 한도가 차면 다른 계정으로 자동 전환(AutoSwitcher) — 계정 메뉴에서 켜고 끈다. 기본은 꺼짐
    @Published var autoSwitch = UserDefaults.standard.bool(forKey: "autoSwitch") {
        didSet { UserDefaults.standard.set(autoSwitch, forKey: "autoSwitch") }
    }
    var onSwitch: (() -> Void)?

    static let vault = "Claude Board-account"
    // Claude Code 가 /login 으로 다른 계정에 들어갈 때 ~/.claude.json 에서 비우는 계정별 캐시(2.1.300 기준) —
    // 같이 비워야 새 계정의 모델 목록·조직 기본 모델·사용량 표시를 다시 받는다
    static let accountCaches = ["additionalModelOptionsCache", "additionalModelOptionsAnsweredAt", "additionalModelCostsCache",
                                "modelAccessCache", "orgModelDefaultCache", "cachedArtifactRoster", "artifactRosterDenied",
                                "lastSeenOrgDefaultUpdatedAt", "clientDataCache", "clientDataCacheSlots", "autoCompactWindowsCache",
                                "cachedUsageUtilization", "metricsStatusCache", "metricsStatusCacheByPrincipal",
                                "githubWebConnectionStatusCache", "startupPrefetchedAt"]
    private let configDir = NSHomeDirectory() + "/.claude"
    private let globalConfig = NSHomeDirectory() + "/.claude.json"
    private let metaPath = NSHomeDirectory() + "/.claude/notifier/accounts.json"
    private let loginRoot = NSHomeDirectory() + "/.claude/notifier/accounts"
    private let queue = DispatchQueue(label: "com.claude.board.accounts")
    private var currentMtime: Date?
    private var timer: Timer?
    private var login: (slot: String, started: Date)?
    private var tokenOnlySince: Date?       // 로그인 토큰은 생겼는데 계정 정보가 아직 없는 때
    private var polling = false
    private var noticeWork: DispatchWorkItem?

    init() {
        loadSaved()
        loadCurrent()
        // 터미널에서 /login 으로 바꿔도 따라가도록 계정 정보 파일을 지켜본다 — 바뀔 때만 다시 읽는다
        timer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in
            self?.loadCurrent()
            self?.pollLogin()
        }
        // 지난번에 끝나지 않은 로그인 자리를 치운다
        queue.async { [loginRoot] in
            for name in (try? FileManager.default.contentsOfDirectory(atPath: loginRoot)) ?? [] where name.hasPrefix("login-") {
                Self.dropSlot(loginRoot + "/" + name)
            }
        }
    }

    static func oauthAccount(at path: String) -> [String: Any]? {
        guard let data = FileManager.default.contents(atPath: path),
              let j = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return nil }
        return j["oauthAccount"] as? [String: Any]
    }

    func loadCurrent(force: Bool = false) {
        let m = mtime(globalConfig)
        guard force || m != currentMtime else { return }
        currentMtime = m
        let a = Self.oauthAccount(at: globalConfig).flatMap { Account(oauth: $0) }
        if a != current { current = a }
    }

    private static func readMeta(_ path: String) -> [[String: Any]] {
        guard let d = FileManager.default.contents(atPath: path),
              let j = try? JSONSerialization.jsonObject(with: d) as? [String: Any] else { return [] }
        return j["accounts"] as? [[String: Any]] ?? []
    }

    private static func metaId(_ e: [String: Any]) -> String? {
        (e["oauthAccount"] as? [String: Any]).flatMap { Account(oauth: $0) }?.id
    }

    private func writeMeta(_ list: [[String: Any]]) throws {
        let data = try JSONSerialization.data(withJSONObject: ["accounts": list], options: [.prettyPrinted, .withoutEscapingSlashes])
        try data.write(to: URL(fileURLWithPath: metaPath), options: .atomic)
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: metaPath)   // 이메일·조직 이름이 들어 있다
    }

    private func loadSaved() {
        let list = Self.readMeta(metaPath).compactMap { e -> Account? in
            guard let oa = e["oauthAccount"] as? [String: Any] else { return nil }
            let exp = (e["refreshTokenExpiresAt"] as? NSNumber).map { Date(timeIntervalSince1970: $0.doubleValue / 1000) }
            return Account(oauth: oa, refreshExpires: exp)
        }
        if list != saved { saved = list }
    }

    // 계정 토큰(claudeAiOauth)은 키체인에, 보일 정보는 accounts.json 에 넣는다 — 같은 계정은 제자리에서 바꾼다. queue 에서 부른다
    private func store(_ oa: [String: Any], _ oauth: [String: Any]) throws {
        guard let a = Account(oauth: oa) else { throw AccountError("계정 정보가 없습니다") }
        guard Keychain.write(Self.vault, account: a.id, try Keychain.asciiJSON(["claudeAiOauth": oauth])) else {
            throw AccountError("키체인에 저장하지 못했습니다")
        }
        var e: [String: Any] = ["oauthAccount": oa, "savedAt": Date().timeIntervalSince1970]
        if let r = oauth["refreshTokenExpiresAt"] { e["refreshTokenExpiresAt"] = r }
        var list = Self.readMeta(metaPath)
        if let i = list.firstIndex(where: { Self.metaId($0) == a.id }) { list[i] = e } else { list.append(e) }
        try writeMeta(list)
    }

    // 지금 로그인된 계정의 최신 토큰을 저장한다 — 쓰는 동안 갱신되며 바뀐 토큰이라, 처음 저장했던 것은 이미 못 쓸 수 있다.
    // .storage-write 잠금 안에서 부른다
    private func saveCurrent(except skip: String? = nil) throws {
        guard let oa = Self.oauthAccount(at: globalConfig), Account(oauth: oa)?.id != skip,
              let co = Keychain.json(credentialsService())?["claudeAiOauth"] as? [String: Any] else { return }
        try store(oa, co)
    }

    // note: 전환했을 때 띄울 안내(자동 전환), done: 전환했는지 — 자동 전환이 결과를 받는다
    func switchTo(_ a: Account, note: String? = nil, done: ((Bool) -> Void)? = nil) {
        guard !switching else { done?(false); return }
        switching = true
        queue.async { [weak self] in
            guard let self else { return }
            let r = Result { try self.performSwitch(a) }
            DispatchQueue.main.async {
                self.switching = false
                self.loadCurrent(force: true)
                self.loadSaved()
                switch r {
                case .success(let warning):
                    self.onSwitch?()
                    self.flash(warning ?? note ?? "\(a.email) 계정으로 전환했습니다 — 실행 중인 세션도 다음 요청부터 이 계정을 씁니다",
                               error: warning != nil)
                    done?(true)
                case .failure(let e):
                    self.flash("전환하지 못했습니다 — \(e.localizedDescription)", error: true)
                    done?(false)
                }
            }
        }
    }

    // queue 에서 부른다. 고칠 수 없었던 부분이 있으면 그 안내를 돌려준다
    private func performSwitch(_ target: Account) throws -> String? {
        guard let oauth = Keychain.json(Self.vault, account: target.id)?["claudeAiOauth"] as? [String: Any] else {
            throw AccountError("저장된 로그인 정보가 없습니다 — 계정을 다시 추가해 주세요")
        }
        // CLAUDE_CONFIG_DIR 을 따로 정해 띄운 세션은 이름 뒤에 경로 해시가 붙은 다른 항목을 쓴다 — 그 항목은 건드리지 않는다.
        // 같은 토큰을 두 항목에 넣으면 한쪽이 갱신(회전)할 때 다른 쪽 토큰이 무효가 되기 때문
        let main = credentialsService()
        let previous = Self.oauthAccount(at: globalConfig).flatMap { Account(oauth: $0) }?.id
        // Claude Code 가 토큰을 갱신해 쓸 때 잡는 잠금 — 그 사이에 끼어 앞 계정 토큰으로 덮이지 않게 한다
        try withFileLock(configDir + "/.storage-write") {
            var cur: [String: Any] = [:]
            if let raw = Keychain.read(main) {
                guard let j = try? JSONSerialization.jsonObject(with: raw) as? [String: Any] else {
                    throw AccountError("지금 로그인 정보를 읽지 못했습니다")
                }
                cur = j
            } else if Keychain.exists(main) {
                throw AccountError("지금 로그인 정보를 읽지 못했습니다")
            }
            try saveCurrent(except: target.id)
            // 계정 토큰만 바꿔 넣는다 — MCP 서버 로그인(mcpOAuth) 같은 다른 값은 그대로 둔다
            cur["claudeAiOauth"] = oauth
            guard Keychain.write(main, try Keychain.asciiJSON(cur)) else { throw AccountError("키체인에 쓰지 못했습니다") }
        }
        // 계정 정보 — /status·claude auth status·보드 계정 표시가 새 계정으로 보이게. 다른 계정이면 앞 계정의 캐시도 비운다
        do {
            try patchJSON(globalConfig) { j in
                j["oauthAccount"] = target.oauth
                if previous != target.id { for k in Self.accountCaches { j[k] = nil } }
            }
        } catch {
            return "토큰은 바꿨지만 계정 정보 파일을 고치지 못했습니다 — \(error.localizedDescription)"
        }
        return nil
    }

    // 계정 추가: 따로 정한 설정 폴더(CLAUDE_CONFIG_DIR)로 claude auth login 을 터미널에서 띄운다 — 지금 로그인은 건드리지 않는다.
    // 로그인이 끝나면 그 폴더 몫 키체인 항목에서 토큰을 가져와 저장하고 자리를 치운다
    func beginAdd(email: String? = nil) {
        guard login == nil else { return }
        guard let claude = findCommand("claude") else { return flash("claude 명령을 찾지 못했습니다", error: true) }
        let slot = loginRoot + "/login-" + UUID().uuidString.prefix(8).lowercased()
        do { try FileManager.default.createDirectory(atPath: slot, withIntermediateDirectories: true) } catch {
            return flash("로그인 자리를 만들지 못했습니다 — \(error.localizedDescription)", error: true)
        }
        var cmd = "CLAUDE_CONFIG_DIR=\(shellQuote(slot)) \(shellQuote(claude.path)) auth login --claudeai"
        if let email { cmd += " --email \(shellQuote(email))" }
        cmd += " && echo && echo 'Claude Board 에 계정을 추가했습니다. 이 창은 닫아도 됩니다.'"
        runInTerminal(cmd)
        login = (slot, Date())
        adding = true
        flash("터미널과 브라우저에서 추가할 계정으로 로그인해 주세요")
    }

    func cancelAdd() {
        guard let l = login else { return }
        login = nil
        tokenOnlySince = nil
        adding = false
        queue.async { Self.dropSlot(l.slot) }
    }

    private static func dropSlot(_ slot: String) {
        Keychain.delete(credentialsService(slot))
        try? FileManager.default.removeItem(atPath: slot)
    }

    // 로그인이 끝났는지 본다 — 그 설정 폴더에 계정 정보가, 키체인에 그 폴더 몫 토큰이 생기면 끝난 것
    private func pollLogin() {
        guard let l = login, !polling else { return }
        if Date().timeIntervalSince(l.started) > 15 * 60 {
            cancelAdd()
            return flash("로그인을 15분 동안 기다리다 그만뒀습니다", error: true)
        }
        polling = true
        queue.async { [weak self] in
            guard let self else { return }
            var result: Result<Account, Error>?
            var tokenOnly = false
            if let oa = Self.oauthAccount(at: l.slot + "/.claude.json"), let a = Account(oauth: oa),
               let oauth = Keychain.json(credentialsService(l.slot))?["claudeAiOauth"] as? [String: Any] {
                result = Result {
                    try self.store(oa, oauth)
                    // 지금 계정도 함께 저장해 둔다 — 목록에 나란히 보이고 언제든 돌아올 수 있게
                    try? withFileLock(self.configDir + "/.storage-write") { try self.saveCurrent(except: a.id) }
                    Self.dropSlot(l.slot)
                    return a
                }
            } else {
                tokenOnly = Keychain.exists(credentialsService(l.slot))
            }
            DispatchQueue.main.async {
                self.polling = false
                // 프로필을 받지 못하면 Claude Code 가 계정 정보를 남기지 않는다 — 30초 넘게 그러면 다시 해 보라고 알린다
                if tokenOnly, self.login?.slot == l.slot {
                    let since = self.tokenOnlySince ?? Date()
                    self.tokenOnlySince = since
                    if Date().timeIntervalSince(since) > 30 {
                        self.cancelAdd()
                        self.flash("로그인은 됐지만 계정 정보를 받지 못했습니다 — 계정 추가를 다시 해 주세요", error: true)
                    }
                    return
                }
                guard let result, self.login?.slot == l.slot else { return }
                self.login = nil
                self.tokenOnlySince = nil
                self.adding = false
                self.loadSaved()
                switch result {
                case .success(let a): self.flash("\(a.email) 계정을 추가했습니다 — 계정 메뉴에서 골라 바로 전환할 수 있습니다")
                case .failure(let e): self.flash("계정을 추가하지 못했습니다 — \(e.localizedDescription)", error: true)
                }
            }
        }
    }

    func remove(_ a: Account) {
        queue.async { [weak self] in
            guard let self else { return }
            Keychain.delete(Self.vault, account: a.id)
            try? self.writeMeta(Self.readMeta(self.metaPath).filter { Self.metaId($0) != a.id })
            DispatchQueue.main.async {
                self.loadSaved()
                self.flash("\(a.email) 계정을 목록에서 지웠습니다")
            }
        }
    }

    func flash(_ text: String, error: Bool = false) {
        withAnimation(.easeOut(duration: 0.2)) { notice = Notice(text: text, error: error) }
        noticeWork?.cancel()
        let w = DispatchWorkItem { [weak self] in withAnimation(.easeOut(duration: 0.3)) { self?.notice = nil } }
        noticeWork = w
        DispatchQueue.main.asyncAfter(deadline: .now() + 7, execute: w)
    }
}

// MARK: - 사용 한도 자동 전환

// 지금 계정의 5시간·주간 한도가 차거나 세션이 한도에 걸려 멈추면 저장한 다른 계정으로 바꾸고, 한도로 멈췄던 세션에 이어서 진행하라는 요청을 넣는다.
// 계정 메뉴에서 켜고 끄며 기본은 꺼짐. 다른 계정의 남은 한도는 미리 알 수 없어(저장한 토큰은 그 계정으로 전환해야 갱신된다)
// 그 계정이 지금 계정이었을 때 한도가 찼던 기록으로 고른다. 결정마다 ~/.claude/notifier/auto-switch.log 에 한 줄씩 남긴다
final class AutoSwitcher {
    // 실행 중인 세션은 로그인 정보를 최대 30초 동안 기억해 두고 쓴다 — 전환한 뒤 이만큼은 이전 계정으로 요청이 나갈 수 있다
    static let settle: TimeInterval = 40
    // 이보다 오래된 한도 기록은 보지 않는다 — 오래전에 멈춘 세션을 계정을 바꿀 때마다 깨우지 않게
    static let recent: TimeInterval = 6 * 3600
    static let prompt = "사용 한도로 멈췄던 세션을 다른 계정으로 전환해 재개합니다. 중단된 지점부터 직전 작업을 이어서 진행해 주세요."
    private let store: Store
    private let usage: UsageStore
    private let accounts: AccountStore
    private var timer: Timer?
    // 마지막으로 계정을 바꾼 시각(손으로 바꾼 것 포함) — 보드를 다시 켜도 이어지게 저장한다
    private var lastSwitch = Date(timeIntervalSince1970: UserDefaults.standard.double(forKey: "autoSwitchLast")) {
        didSet { UserDefaults.standard.set(lastSwitch.timeIntervalSince1970, forKey: "autoSwitchLast") }
    }
    private var pending = false              // 자동 전환을 요청하고 결과를 기다리는 중
    private var resumed = Set<String>()      // 이어 가기를 넣은 "세션 id@한도 시각" — 한 번만 넣는다
    private var noCandidate = false          // 전환할 계정이 없다고 이미 알렸다 — 계정이 바뀌기 전에는 다시 알리지 않는다
    // 한도가 찬 계정 id 와 풀리는 시각 — 그 전에는 고르지 않는다
    private var full = UserDefaults.standard.dictionary(forKey: "autoSwitchFull") as? [String: Double] ?? [:] {
        didSet { UserDefaults.standard.set(full, forKey: "autoSwitchFull") }
    }
    private let logPath = NSHomeDirectory() + "/.claude/notifier/auto-switch.log"

    init(store: Store, usage: UsageStore, accounts: AccountStore) {
        self.store = store
        self.usage = usage
        self.accounts = accounts
        timer = Timer.scheduledTimer(withTimeInterval: 3, repeats: true) { [weak self] _ in self?.tick() }
    }

    // 계정이 바뀌었다 — 이때부터 settle 동안 난 한도 오류는 이전 계정 토큰으로 나간 요청이다
    func switched() {
        lastSwitch = Date()
        noCandidate = false
    }

    private func tick() {
        guard accounts.autoSwitch else { return }
        let now = Date()
        let settled = lastSwitch.addingTimeInterval(Self.settle)
        // 승인 대기 세션은 Esc 가 승인을 거절하므로 건드리지 않는다
        let limited = store.groups.flatMap(\.sessions).filter {
            guard let at = $0.limitAt else { return false }
            return !["ended", "paused", "waiting"].contains($0.state) && now.timeIntervalSince(at) < Self.recent
        }
        // 1) 전환 — 지금 계정으로 새로 한도에 걸린 세션이 있거나, 지금 계정의 5시간·주간 한도가 찼다
        if !pending, !accounts.switching, now > settled, let cur = accounts.current {
            let fullLimit = usage.limits.first { ($0.id == "5h" || $0.id == "7d") && $0.pct >= 100 && ($0.resets ?? .distantFuture) > now }
            if let hit = limited.first(where: { $0.limitAt! > settled }) {
                switchAway(cur, because: "\((hit.cwd as NSString).lastPathComponent) 세션이 한도에 걸림(\(hit.message))", now: now)
            } else if let fullLimit {
                switchAway(cur, because: "\(fullLimit.label) 한도 \(Int(fullLimit.pct))%", now: now)
            }
        }
        // 2) 이어 가기 — 전환하고 settle 이 지났으면, 그 전(이전 계정)에 한도로 멈춘 세션에 이어서 진행하라는 요청을 넣는다
        guard now > settled else { return }
        let todo = limited.filter { $0.limitAt! <= settled && !resumed.contains("\($0.id)@\($0.limitAt!.timeIntervalSince1970)") }
        guard !todo.isEmpty else { return }
        var manual: [String] = []
        for s in todo {
            resumed.insert("\(s.id)@\(s.limitAt!.timeIntervalSince1970)")
            let name = (s.cwd as NSString).lastPathComponent
            guard canDrive(s) else {
                manual.append(name)
                log("\(name) — Ghostty 터미널 세션이 아니라 이어 가지 못함")
                continue
            }
            ghosttyQueue.async { [weak self] in
                // Esc 를 먼저 — Claude Code 가 띄운 한도 선택 창을 닫고 "한도가 풀리면 저절로 이어 가기" 예약을 취소한다.
                // 선택 창이 열린 채 Enter 를 보내면 그 창의 항목이 골라진다
                let ok = ghosttyTerminalId(s).map {
                    ghosttySend($0, [("key", "escape"), ("wait", "0.5"), ("text", Self.prompt), ("wait", "0.2"), ("key", "enter")])
                } ?? false
                DispatchQueue.main.async { self?.log("\(name) — \(ok ? "이어 가기 요청을 넣음" : "터미널을 찾지 못해 이어 가지 못함")") }
            }
        }
        let n = todo.count - manual.count
        accounts.flash((n > 0 ? "한도로 멈췄던 세션 \(n)개를 이어서 진행합니다" : "")
                       + (manual.isEmpty ? "" : (n > 0 ? " — " : "") + "\(manual.joined(separator: ", ")) 세션은 Ghostty 밖이라 직접 이어 주세요"),
                       error: !manual.isEmpty)
    }

    // 지금 계정은 한도가 풀릴 때까지 고르지 않게 적어 두고, 저장한 계정 가운데 지금 계정 다음부터 쓸 수 있는 계정으로 바꾼다
    private func switchAway(_ cur: Account, because reason: String, now: Date) {
        // 풀리는 시각은 지금 계정의 찬 한도에서 읽는다 — 모르면 1시간 뒤 다시 본다
        let resets = usage.limits.filter { $0.pct >= 100 }.compactMap(\.resets).filter { $0 > now }.max()
        var marks = full.filter { $0.value > now.timeIntervalSince1970 }
        marks[cur.id] = (resets ?? now.addingTimeInterval(3600)).timeIntervalSince1970
        full = marks
        let saved = accounts.saved
        let start = saved.firstIndex { $0.id == cur.id }.map { $0 + 1 } ?? 0
        guard let next = (saved[start...] + saved[..<start]).first(where: { $0.id != cur.id && !$0.expired && marks[$0.id] == nil }) else {
            if !noCandidate {
                noCandidate = true
                log("\(reason) — 전환할 계정 없음")
                accounts.flash("사용 한도에 도달했지만 전환할 다른 계정이 없습니다 — 저장한 계정이 모두 한도에 도달했거나 다시 로그인이 필요합니다", error: true)
            }
            return
        }
        pending = true
        log("\(reason) — \(cur.email) 에서 \(next.email) 로 전환")
        let stopped = store.groups.flatMap(\.sessions).contains { $0.limitAt != nil && $0.state != "ended" }
        let note = "사용 한도에 도달해 \(next.email) 계정으로 자동 전환했습니다"
            + (stopped ? " — 한도로 멈춘 세션은 \(Int(Self.settle))초 뒤 이어서 진행합니다" : "")
        accounts.switchTo(next, note: note) { [weak self] ok in
            self?.pending = false
            guard !ok else { return }
            self?.log("\(next.email) 로 전환하지 못함 — 1시간 동안 고르지 않음")
            self?.full[next.id] = now.addingTimeInterval(3600).timeIntervalSince1970
        }
    }

    private func log(_ text: String) {
        let line = "\(Date()) \(text)\n"
        if let h = FileHandle(forWritingAtPath: logPath) {
            h.seekToEndOfFile()
            h.write(Data(line.utf8))
            try? h.close()
        } else {
            FileManager.default.createFile(atPath: logPath, contents: Data(line.utf8))
        }
    }
}

// 앱 아이콘(AppIcon.icns) — 메뉴 막대와 보드 제목 옆에도 같은 그림을 쓴다. 크기별 그림 중 화면 배율에 맞는 것이 골라진다
let appIcon = NSImage(named: "AppIcon") ?? NSImage()

enum Palette {
    static func color(_ state: String) -> Color {
        switch state {
        case "working": return Color(red: 0.45, green: 0.66, blue: 1.00)
        case "waiting": return Color(red: 1.00, green: 0.66, blue: 0.36)
        case "idle": return Color(red: 0.70, green: 0.75, blue: 0.86)   // 유휴 — 완료로 읽히지 않게 초록 대신 차분한 회청색
        case "paused": return Color(red: 0.95, green: 0.80, blue: 0.35)
        default: return Color(white: 0.55)
        }
    }

    static func name(_ state: String) -> String {
        ["working": "작업 중", "waiting": "승인 대기", "idle": "대기", "paused": "일시중지", "ended": "종료"][state] ?? state
    }

    static let accent = Color(red: 0.85, green: 0.47, blue: 0.34)
}

final class Store: ObservableObject {
    @Published var groups: [Group] = []
    @Published var now = Date()
    // 접힌 그룹의 cwd — 앱을 다시 켜도 유지한다
    @Published var collapsed = Set(UserDefaults.standard.stringArray(forKey: "collapsed") ?? [])
    // 전체 알림 끄기 — ~/.claude/notifier/mute-all 이 있으면 notify.py 가 모든 세션의 알림을 띄우지 않는다
    @Published var allMuted = FileManager.default.fileExists(atPath: muteAllPath)
    private let dir = (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/sessions")
    private var timer: Timer?
    private let contexts = ContextReader()
    private let models = ModelReader()
    // 백그라운드 셸 수 — ps 를 메인 스레드에서 돌리면 클릭 처리가 밀리므로 뒤에서 세고 다음 reload 가 쓴다
    private var shellCounts: [Int32: Int] = [:]
    private var shellsBusy = false
    private var shellsAt = Date.distantPast
    // 세션 기록 — 매초 30여 개 파일을 다시 읽어 풀던 것을, 파일이 바뀔 때만 다시 읽는다
    private var records: [String: (mtime: Date, j: [String: Any])] = [:]

    // ps 는 프로세스 수백 개를 훑으므로 3초에 한 번만 센다
    private func refreshShells() {
        guard !shellsBusy, now.timeIntervalSince(shellsAt) >= 3 else { return }
        shellsBusy = true
        shellsAt = now
        DispatchQueue.global(qos: .utility).async {
            let counts = backgroundShells()
            DispatchQueue.main.async { [weak self] in
                self?.shellCounts = counts
                self?.shellsBusy = false
            }
        }
    }

    init() {
        reload()
        timer = Timer.scheduledTimer(withTimeInterval: 1, repeats: true) { [weak self] _ in self?.reload() }
        // 세션 줄에서 표지 파일을 바꾸면 1초 주기를 기다리지 않고 바로 다시 읽는다
        NotificationCenter.default.addObserver(forName: .boardReload, object: nil, queue: .main) { [weak self] _ in
            self?.reload(force: true)
        }
    }

    func count(_ state: String) -> Int { groups.reduce(0) { $0 + $1.sessions.filter { $0.state == state }.count } }

    // 직전 reload 때 승인 대기였던 세션 — 새로 승인 대기가 된 세션만 골라 그 그룹을 펼친다
    private var waitingIds: Set<String> = []

    func toggle(_ cwd: String) {
        if collapsed.contains(cwd) { collapsed.remove(cwd) } else { collapsed.insert(cwd) }
        UserDefaults.standard.set(Array(collapsed), forKey: "collapsed")
    }

    // 종료 세션 목록을 펼쳐 둔 그룹의 cwd — 기본은 접힘
    @Published var endedOpen = Set(UserDefaults.standard.stringArray(forKey: "endedOpen") ?? [])

    func toggleEnded(_ cwd: String) {
        if endedOpen.contains(cwd) { endedOpen.remove(cwd) } else { endedOpen.insert(cwd) }
        UserDefaults.standard.set(Array(endedOpen), forKey: "endedOpen")
    }

    // 상단에 고정한 작업경로 — 고정한 순서대로 맨 위에 둔다
    @Published var pinnedGroups = UserDefaults.standard.stringArray(forKey: "pinnedGroups") ?? []

    func togglePinGroup(_ cwd: String) {
        if let i = pinnedGroups.firstIndex(of: cwd) { pinnedGroups.remove(at: i) } else { pinnedGroups.append(cwd) }
        UserDefaults.standard.set(pinnedGroups, forKey: "pinnedGroups")
        reload(force: true)
    }

    // 목록에서 지운 작업경로와 지운 시각 — 그 뒤에 새 활동(세션 기록 갱신)이 생기면 다시 나타난다
    private var hiddenGroups = UserDefaults.standard.dictionary(forKey: "hiddenGroups") as? [String: Double] ?? [:]

    func hideGroup(_ g: Group) {
        // 종료된 세션 기록은 함께 지운다
        for s in g.sessions where s.state == "ended" { try? FileManager.default.removeItem(atPath: s.file) }
        hiddenGroups[g.id] = Date().timeIntervalSince1970
        UserDefaults.standard.set(hiddenGroups, forKey: "hiddenGroups")
        pinnedGroups.removeAll { $0 == g.id }
        UserDefaults.standard.set(pinnedGroups, forKey: "pinnedGroups")
        reload(force: true)
    }

    // 그 작업경로의 종료된 세션 기록을 한 번에 지운다 — 대화 기록(jsonl)은 남는다
    func removeEnded(_ g: Group) {
        for s in g.sessions where s.state == "ended" { try? FileManager.default.removeItem(atPath: s.file) }
        reload(force: true)
    }

    // 자동으로 접은 작업경로와 그때의 마지막 활동 시각 — 사람이 다시 펼치면 새 활동이 생기기 전까지 또 접지 않는다
    private var autoFolded = UserDefaults.standard.dictionary(forKey: "autoFolded") as? [String: Double] ?? [:]
    static let quietFoldAfter: TimeInterval = 30 * 60

    // 승인 대기·작업 중 세션이 없고 마지막 활동이 30분 넘게 지난 카드를 한 번 접는다
    private func foldQuiet(_ groups: [Group]) {
        var changed = false
        for g in groups {
            let last = (g.sessions.map(\.updated).max() ?? .distantPast).timeIntervalSince1970
            let quiet = !g.sessions.contains { $0.state == "waiting" || $0.state == "working" }
            guard quiet, now.timeIntervalSince1970 - last > Store.quietFoldAfter else { continue }
            if autoFolded[g.id] == last { continue }
            autoFolded[g.id] = last
            changed = true
            if !collapsed.contains(g.id) {
                withAnimation(foldSpring) { _ = collapsed.insert(g.id) }
                UserDefaults.standard.set(Array(collapsed), forKey: "collapsed")
            }
        }
        let ids = Set(groups.map(\.id))
        if autoFolded.keys.contains(where: { !ids.contains($0) }) {
            autoFolded = autoFolded.filter { ids.contains($0.key) }
            changed = true
        }
        if changed { UserDefaults.standard.set(autoFolded, forKey: "autoFolded") }
    }

    // 요약 칩으로 찾아갈 때: 카드와 그 안의 터미널·백그라운드 구역을 펼친다
    func reveal(_ cwd: String) {
        if collapsed.contains(cwd) {
            collapsed.remove(cwd)
            UserDefaults.standard.set(Array(collapsed), forKey: "collapsed")
        }
        let d = UserDefaults.standard
        let keys = (d.string(forKey: "kindCollapsed") ?? "").split(separator: "\n").map(String.init)
        let kept = keys.filter { !$0.hasPrefix(cwd + "|") }
        if kept.count != keys.count { d.set(kept.joined(separator: "\n"), forKey: "kindCollapsed") }
    }

    func setAll(collapse: Bool) {
        collapsed = collapse ? Set(groups.map(\.id)) : []
        UserDefaults.standard.set(Array(collapsed), forKey: "collapsed")
    }

    private func expandNewlyWaiting(_ sessions: [Session]) {
        let waiting = sessions.filter { $0.state == "waiting" }
        let opened = Set(waiting.filter { !waitingIds.contains($0.id) }.map(\.cwd)).intersection(collapsed)
        waitingIds = Set(waiting.map(\.id))
        guard !opened.isEmpty else { return }
        withAnimation(.spring(response: 0.35, dampingFraction: 0.85)) { collapsed.subtract(opened) }
        UserDefaults.standard.set(Array(collapsed), forKey: "collapsed")
    }

    // 세션별 .mute 표지는 건드리지 않으므로 다시 켜면 세션마다 정해 둔 상태가 그대로 돌아온다
    func setAllMuted(_ on: Bool) {
        if on {
            FileManager.default.createFile(atPath: muteAllPath, contents: Data())
        } else {
            try? FileManager.default.removeItem(atPath: muteAllPath)
        }
        withAnimation(.spring(response: 0.3, dampingFraction: 0.8)) { allMuted = on }
    }

    func reload(force: Bool = false) {
        now = Date()
        let fm = FileManager.default
        let shells = shellCounts
        refreshShells()
        var sessions: [Session] = []
        var brainPaths = Set<String>()
        var seen = Set<String>()
        for name in (try? fm.contentsOfDirectory(atPath: dir)) ?? [] where name.hasSuffix(".json") {
            let path = (dir as NSString).appendingPathComponent(name)
            guard let m = mtime(path) else { continue }
            seen.insert(path)
            let j: [String: Any]
            if let c = records[path], c.mtime == m {
                j = c.j
            } else {
                guard let data = fm.contents(atPath: path),
                      let parsed = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }
                records[path] = (m, parsed)
                j = parsed
            }
            var state = j["state"] as? String ?? "idle"
            let updated = Date(timeIntervalSince1970: j["updated"] as? Double ?? 0)
            if state != "ended", let pid = (j["pid"] as? NSNumber)?.int32Value, kill(pid, 0) != 0, errno == ESRCH {
                state = "ended"
            }
            // 질문 없이 끝난 세션은 대화 기록 파일이 없어 이어 열 수 없으므로 목록에서 뺀다
            let sid = j["session_id"] as? String ?? String(name.dropLast(5))
            let cwdPath = j["cwd"] as? String ?? "?"
            let transcript = transcriptPath(j["transcript_path"] as? String, cwdPath, sid)
            if state == "ended", now.timeIntervalSince(updated) > 2 * 86400
                || !fm.fileExists(atPath: transcript) {
                try? fm.removeItem(atPath: path)
                continue
            }
            // 응답은 끝났어도 서브에이전트나 백그라운드 셸이 돌고 있으면 작업 중으로 보인다
            let pid = (j["pid"] as? NSNumber)?.int32Value ?? 0
            let subDir = ((transcript.hasSuffix(".jsonl") ? String(transcript.dropLast(6)) : transcript) as NSString)
                .appendingPathComponent("subagents")
            let subs = Sub.parse(j["subagents"] as? [String: Any], alive: state != "ended", now: now).map { sub -> Sub in
                var sub = sub
                let p = (subDir as NSString).appendingPathComponent("agent-\(sub.id).jsonl")
                brainPaths.insert(p)
                sub.brain = models.read(p, now: now)
                return sub
            }
            let shellCount = state == "idle" ? shells[pid] ?? 0 : 0   // 응답 중에 도는 셸은 보통 도구 호출이라 세지 않는다
            if state == "idle", shellCount > 0 || subs.contains(where: { $0.state != "done" }) {
                state = "working"
            }
            // 일시중지 표지는 보드가 남기고, 새 요청이 들어오면 state.py 가 지운다
            let pausedFile = (dir as NSString).appendingPathComponent("\(sid).paused")
            let paused = fm.fileExists(atPath: pausedFile)
            if paused, state != "ended" { state = "paused" }
            let muted = fm.fileExists(atPath: (dir as NSString).appendingPathComponent("\(sid).mute"))
            var brain: Brain? = nil
            if state != "ended" {
                brainPaths.insert(transcript)
                brain = models.read(transcript, fallbackModel: j["model"] as? String ?? "", now: now)
                if let b = brain, let e = contexts.liveEffort(sid, transcript: transcript) { brain = Brain(model: b.model, effort: e) }
            }
            let clean = { (s: String) in
                s.replacingOccurrences(of: "[#*`]+", with: "", options: .regularExpression)
                    .trimmingCharacters(in: .whitespaces)
            }
            sessions.append(Session(id: j["session_id"] as? String ?? name,
                                    cwd: j["cwd"] as? String ?? "?",
                                    state: state,
                                    since: Date(timeIntervalSince1970: j["since"] as? Double ?? 0),
                                    updated: updated,
                                    headless: j["headless"] as? Bool ?? false,
                                    tty: j["tty"] as? String ?? "",
                                    topic: clean(j["topic"] as? String ?? ""),
                                    message: clean(j["last_message"] as? String ?? ""),
                                    resumeCmd: j["resume_cmd"] as? String ?? "",
                                    file: path,
                                    ctx: contexts.read(sid, transcript: transcript, model: j["model"] as? String ?? "", now: now),
                                    brain: brain,
                                    pid: pid,
                                    subagents: subs,
                                    shells: shellCount,
                                    paused: paused,
                                    muted: muted,
                                    limitAt: ((j["limit_hit"] as? [String: Any])?["at"] as? Double).map { Date(timeIntervalSince1970: $0) }))
        }
        models.prune(keep: brainPaths)
        records = records.filter { seen.contains($0.key) }
        MainActor.assumeIsolated { TerminalHub.shared.pair(sessions) }   // 1초 타이머·알림 관찰자 모두 메인 스레드
        expandNewlyWaiting(sessions)
        let muted = fm.fileExists(atPath: muteAllPath)
        if muted != allMuted { allMuted = muted }
        let grouped = Dictionary(grouping: sessions, by: \.cwd).map { cwd, list in
            Group(id: cwd, sessions: list.sorted { $0.rank != $1.rank ? $0.rank < $1.rank : $0.since > $1.since })
        }
        // 지운 뒤 새 활동이 없는 작업경로는 빼고, 새 활동이 생긴 작업경로는 지운 목록에서 꺼낸다
        var visible: [Group] = []
        for g in grouped {
            if let at = hiddenGroups[g.id] {
                if g.sessions.allSatisfy({ $0.updated.timeIntervalSince1970 <= at }) { continue }
                hiddenGroups[g.id] = nil
                UserDefaults.standard.set(hiddenGroups, forKey: "hiddenGroups")
            }
            visible.append(g)
        }
        foldQuiet(visible)
        let pins = pinnedGroups
        let sorted = visible.sorted {
            let pa = pins.firstIndex(of: $0.id) ?? Int.max, pb = pins.firstIndex(of: $1.id) ?? Int.max
            if pa != pb { return pa < pb }
            let a = $0.sessions.map(\.rank).min() ?? 9, b = $1.sessions.map(\.rank).min() ?? 9
            if a != b { return a < b }
            return ($0.sessions.map(\.updated).max() ?? .distantPast) > ($1.sessions.map(\.updated).max() ?? .distantPast)
        }
        if force || sorted != groups {
            withAnimation(.spring(response: 0.45, dampingFraction: 0.82)) { groups = sorted }
        }
    }
}

// claude 프로세스별 백그라운드 셸 수 — Claude Code 는 Bash 도구를 shell-snapshots 를 읽는 zsh/bash 자식으로 띄운다.
// 응답이 끝난 뒤에도 남아 있는 이 자식은 run_in_background 로 띄운 셸이다
func backgroundShells() -> [Int32: Int] {
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "/bin/ps")
    p.arguments = ["-axo", "ppid=,command="]
    let pipe = Pipe()
    p.standardOutput = pipe
    p.standardError = FileHandle.nullDevice
    guard (try? p.run()) != nil else { return [:] }
    let out = String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
    p.waitUntilExit()
    var counts: [Int32: Int] = [:]
    for line in out.split(separator: "\n") where line.contains("/.claude/shell-snapshots/snapshot-") {
        let t = line.trimmingCharacters(in: .whitespaces)
        if let ppid = Int32(t.prefix { $0.isNumber }) { counts[ppid, default: 0] += 1 }
    }
    return counts
}

private func formatter(_ format: String) -> DateFormatter {
    let f = DateFormatter()
    f.locale = Locale(identifier: "ko_KR")
    f.dateFormat = format
    return f
}
private let hourMinute = formatter("HH:mm"), dayHourMinute = formatter("M/d(E) HH:mm")

// 종료된 세션은 흐르는 시간 대신 끝난 시각을 고정해서 보여 준다
func clock(_ date: Date) -> String { hourMinute.string(from: date) }

func elapsed(_ from: Date, _ now: Date) -> String {
    let s = max(0, Int(now.timeIntervalSince(from)))
    if s < 60 { return "\(s)초" }
    if s < 3600 { return "\(s / 60)분 \(s % 60)초" }
    return "\(s / 3600)시간 \(s % 3600 / 60)분"
}

// MARK: - 상태 표시 (애니메이션)

// 늘 도는 애니메이션의 시계 — SwiftUI 반복 애니메이션은 화면 주사율만큼 창 전체를 다시 그려 클릭이 버벅이므로 초당 30번으로 묶는다
// 늘 도는 애니메이션은 시계 하나를 함께 본다 — 따로 돌면 시계마다 창 전체를 따로 다시 그린다
final class AmbientTicker: ObservableObject {
    static let shared = AmbientTicker()
    @Published private(set) var now = Date().timeIntervalSinceReferenceDate
    private var timer: Timer?
    // 창이 다른 창에 다 가려졌거나 숨었으면 멈춘다 — 보이지 않는 감독·회전 표시를 초당 30번 다시 그리지 않게
    var running = true { didSet { if running != oldValue { schedule() } } }
    private init() { schedule() }
    private func schedule() {
        timer?.invalidate()
        timer = nil
        guard running else { return }
        let t = Timer(timeInterval: 1.0 / 30, repeats: true) { [weak self] _ in self?.now = Date().timeIntervalSinceReferenceDate }
        RunLoop.main.add(t, forMode: .common)
        timer = t
    }
}
struct Ambient<Content: View>: View {
    @ObservedObject private var ticker = AmbientTicker.shared
    @ViewBuilder let content: (TimeInterval) -> Content
    var body: some View { content(ticker.now) }
}
func cycle(_ t: TimeInterval, _ period: Double) -> Double { (t / period).truncatingRemainder(dividingBy: 1) }
// 0 → 1 → 0 을 부드럽게 오가는 값 (period 동안 한 번 왕복)
func swing(_ t: TimeInterval, _ period: Double) -> Double { (1 - cos(2 * .pi * cycle(t, period))) / 2 }

// 작업 중: 그라데이션 호가 계속 회전한다
struct Spinner: View {
    let color: Color
    var body: some View {
        Ambient { now in
        ZStack {
            Circle().stroke(color.opacity(0.18), lineWidth: 2.5)
            Circle()
                .trim(from: 0, to: 0.72)
                .stroke(AngularGradient(colors: [color.opacity(0), color], center: .center),
                        style: StrokeStyle(lineWidth: 2.5, lineCap: .round))
                .rotationEffect(.degrees(cycle(now, 0.9) * 360))
        }
        }
    }
}

// 승인 대기: 점 둘레로 파문이 퍼진다
struct Pulse: View {
    let color: Color
    var body: some View {
        Ambient { now in
            let p = cycle(now, 1.1)
            let e = 1 - (1 - p) * (1 - p)   // easeOut
            ZStack {
                Circle().fill(color.opacity(0.35)).scaleEffect(0.8 + 1.1 * e).opacity(1 - e)
                Circle().fill(color).frame(width: 9, height: 9)
            }
        }
    }
}

// 대기: 체크 표시가 통통 튀며 나타난다
struct Check: View {
    let color: Color
    @State private var shown = false
    var body: some View {
        Image(systemName: "checkmark.circle.fill")
            .font(.system(size: 16, weight: .semibold))
            .foregroundStyle(color)
            .scaleEffect(shown ? 1 : 0.3)
            .opacity(shown ? 1 : 0)
            .onAppear { withAnimation(.spring(response: 0.35, dampingFraction: 0.5)) { shown = true } }
    }
}

struct StateIcon: View {
    let state: String
    var body: some View {
        let c = Palette.color(state)
        ZStack {
            switch state {
            case "working": Spinner(color: c)
            case "waiting": Pulse(color: c)
            case "idle": Check(color: c)
            case "paused": Image(systemName: "pause.circle.fill").font(.system(size: 16, weight: .semibold)).foregroundStyle(c)
            default:
                Image(systemName: "minus.circle.fill").font(.system(size: 15)).foregroundStyle(Color(red: 0.93, green: 0.33, blue: 0.33))
            }
        }
        .frame(width: 18, height: 18)
        .id(state)   // 상태가 바뀌면 새로 그려 등장 애니메이션을 다시 낸다
        .transition(.scale.combined(with: .opacity))
    }
}

// 작업 중인 줄 위를 은은한 빛이 지나간다
struct Shimmer: ViewModifier {
    let active: Bool
    let color: Color
    func body(content: Content) -> some View {
        content.overlay {
            if active {
                GeometryReader { geo in
                    Ambient { now in
                        let phase = -1 + 2 * cycle(now, 2.4)
                        LinearGradient(colors: [.clear, color.opacity(0.10), .clear], startPoint: .leading, endPoint: .trailing)
                            .frame(width: geo.size.width * 0.5)
                            .offset(x: phase * geo.size.width * 1.5)
                    }
                }
                .allowsHitTesting(false)
                .clipShape(RoundedRectangle(cornerRadius: 10))
            }
        }
    }
}

// MARK: - 화면

// 세션 실행 방식 라벨: 터미널 창에서 도는 세션인지, 터미널 없이 백그라운드로 도는 세션인지
struct KindLabel: View {
    var headless = false
    var subagent = false
    var compact = false     // 줄이 좁으면 글자를 빼고 아이콘만 보인다
    var body: some View {
        let c = subagent ? Color(red: 0.95, green: 0.55, blue: 0.75)
            : headless ? Color(red: 0.72, green: 0.55, blue: 1.00) : Color(red: 0.32, green: 0.78, blue: 0.86)
        let name = subagent ? "서브에이전트" : headless ? "백그라운드" : "터미널"
        Label(name, systemImage: subagent ? "person.2.fill" : headless ? "moon.stars.fill" : "terminal.fill")
            .font(.system(size: 10, weight: .semibold))
            .labelStyle(compact ? AnyLabelStyle(.iconOnly) : AnyLabelStyle(.titleAndIcon))
            .fixedSize()
            .hint(name)
            .foregroundStyle(c)
            .padding(.horizontal, 6)
            .padding(.vertical, 1.5)
            .background(Capsule().fill(c.opacity(0.15)))
            .overlay(Capsule().stroke(c.opacity(0.25), lineWidth: 0.5))
    }
}

// 세션 칩 — 어디서 도는지: 보드 탭 CB(탭 안에서 시작된 claude -p 는 CB -p), 다른 터미널은 앱 이름,
// 백그라운드는 실제 실행 방식($ claude -p·--bg). 끝난 세션은 누르면 보드에서 이어 연다는 표시
struct PlaceChip: View {
    let s: Session
    let place: SessionPlace
    let appName: String?
    let mode: String?   // -p·--bg — 대화형이면 nil

    private func chip(_ text: String, _ bg: Color, _ fg: Color, mono: Bool = false, hint: String) -> some View {
        Text(text)
            .font(.system(size: 10, weight: .semibold, design: mono ? .monospaced : .default))
            .foregroundStyle(fg)
            .padding(.horizontal, 6).padding(.vertical, 1.5)
            .background(Capsule().fill(bg))
            .fixedSize()
            .hint(hint)
    }

    var body: some View {
        switch place {
        case .tab(_, let headless):
            chip(mode.map { "CB \($0)" } ?? "CB", Color(red: 0.18, green: 0.23, blue: 0.34), Color(red: 0.66, green: 0.76, blue: 1.0),
                 hint: headless || mode != nil ? "보드 탭 안에서 시작된 claude \(mode ?? "-p") — 탭이 닫히면 함께 끝납니다" : "보드 탭에서 도는 세션 — 누르면 그 탭으로")
        case .terminal:
            if let appName {
                chip(appName, Color(red: 0.14, green: 0.25, blue: 0.29), Color(red: 0.56, green: 0.85, blue: 0.91), hint: "\(appName) 에서 도는 세션")
            }
        case .background:
            chip("$ claude \(mode ?? "-p")", Color(red: 0.23, green: 0.20, blue: 0.27), Color(red: 0.83, green: 0.72, blue: 1.0), mono: true,
                 hint: "터미널 없이 도는 세션 — 보드는 상태만 보여 줍니다")
        case .ended:
            if s.headless {
                chip("$ claude -p", Color.white.opacity(0.08), .secondary, mono: true, hint: "끝난 백그라운드 세션 — 메뉴의 세션 다시 열기로 보드 탭에서 엽니다")
            } else {
                chip("보드에서 이어 열기", Color.white.opacity(0.08), .secondary, hint: "누르면 보드 새 탭에서 이 대화를 이어 엽니다")
            }
        }
    }
}

func tokens(_ n: Int) -> String {
    n >= 1_000_000 ? String(format: "%.1fM", Double(n) / 1_000_000) : n >= 1000 ? "\(n / 1000)K" : "\(n)"
}

// 컨텍스트 사용량 라벨: 작은 막대 + 비율 + 토큰 수 (30% 미만 초록, 70% 미만 노랑, 그 이상 빨강 — 상태줄과 같은 기준)
struct ContextLabel: View {
    let ctx: Ctx
    var level = 0           // 0 막대·비율·토큰, 1 막대 없이 비율·토큰, 2 비율만 — 줄이 좁을수록 줄인다
    var body: some View {
        let c = ctx.pct < 30 ? Color(red: 0.42, green: 0.85, blue: 0.60)
            : ctx.pct < 70 ? Color(red: 1.00, green: 0.80, blue: 0.35) : Color(red: 1.00, green: 0.42, blue: 0.42)
        HStack(spacing: 4) {
            Image(systemName: "brain.head.profile").font(.system(size: 9, weight: .semibold))
            if level == 0 {
                ZStack(alignment: .leading) {
                    Capsule().fill(c.opacity(0.18))
                    Capsule().fill(c).frame(width: max(2, 26 * CGFloat(ctx.pct) / 100))
                }
                .frame(width: 26, height: 4)
            }
            Text(level == 2 ? "\(ctx.pct)%" : "\(ctx.pct)% · \(tokens(ctx.used))").monospacedDigit().contentTransition(.numericText())
        }
        .font(.system(size: 10, weight: .semibold))
        .foregroundStyle(c)
        .padding(.horizontal, 6)
        .padding(.vertical, 1.5)
        .background(Capsule().fill(c.opacity(0.12)))
        .fixedSize()   // 정보 줄이 길어져도 라벨 안 글자가 두 줄로 꺾이지 않게
        .overlay(Capsule().stroke(c.opacity(0.22), lineWidth: 0.5))
        .hint("컨텍스트 \(ctx.used.formatted()) / \(ctx.size.formatted()) 토큰")
        .animation(.easeInOut(duration: 0.4), value: ctx)
    }
}

// 모델·effort 라벨: 계열 이름(색으로 계열 구분) + effort 막대 5칸(low 1 … max 5)
struct ModelLabel: View {
    let brain: Brain
    var level = 0           // 0 계열·버전, 1 계열만, 2 계열 첫 글자 — 줄이 좁을수록 줄인다
    var body: some View {
        let c = Self.color(brain.family)
        HStack(spacing: 4) {
            Text(level == 0 ? [brain.family, brain.version].filter { !$0.isEmpty }.joined(separator: " ")
                 : level == 1 ? brain.family : String(brain.family.prefix(1)))
            if brain.effortLevel > 0 { EffortBars(level: brain.effortLevel, color: c) }
        }
        .font(.system(size: 10, weight: .semibold))
        .foregroundStyle(c)
        .padding(.horizontal, 6)
        .padding(.vertical, 1.5)
        .background(Capsule().fill(c.opacity(0.12)))
        .fixedSize()
        .overlay(Capsule().stroke(c.opacity(0.22), lineWidth: 0.5))
        .hint("모델 \(brain.model)" + (brain.effort.isEmpty ? "" : " · effort \(brain.effort)"))
    }

    static func color(_ family: String) -> Color {
        switch family {
        case "Opus": Color(red: 1.00, green: 0.60, blue: 0.45)
        case "Sonnet": Color(red: 0.48, green: 0.70, blue: 1.00)
        case "Haiku": Color(red: 0.55, green: 0.88, blue: 0.80)
        case "Fable": Color(red: 0.88, green: 0.70, blue: 1.00)
        default: Color(white: 0.75)
        }
    }
}

// effort 막대 — 키가 점점 커지는 5칸, 채운 칸 수 = effort 단계
struct EffortBars: View {
    let level: Int
    let color: Color
    var body: some View {
        HStack(alignment: .bottom, spacing: 1.2) {
            ForEach(1...5, id: \.self) { i in
                RoundedRectangle(cornerRadius: 0.8)
                    .fill(i <= level ? color : color.opacity(0.22))
                    .frame(width: 2.2, height: 3 + CGFloat(i) * 1.3)
            }
        }
        .frame(height: 9.5, alignment: .bottom)
    }
}

// 남은 시간 + 초기화 시각 — 하루 넘게 남으면 날짜를 붙인다
func resetText(_ d: Date?, _ now: Date) -> String {
    guard let d else { return "" }
    let s = Int(d.timeIntervalSince(now))
    if s <= 0 { return "곧 초기화" }
    let left = s >= 86400 ? "\(s / 86400)일 \(s % 86400 / 3600)시간" : s >= 3600 ? "\(s / 3600)시간 \(s % 3600 / 60)분" : "\(max(1, s / 60))분"
    return "\(left) 뒤 · \((s >= 86400 ? dayHourMinute : hourMinute).string(from: d))"
}

// 사용 한도 카드: 5시간·주간 전체·주간 모델별 막대. 머리에 로그인 계정(눌러서 전환), 오른쪽 위 단추로 간략(한 줄)·상세를 바꾼다
struct UsagePanel: View {
    @ObservedObject var usage: UsageStore
    @ObservedObject var accounts: AccountStore
    let now: Date
    var forceCompact = false    // 창이 낮아 중간 화면일 때 — 저장한 간략·상세 설정은 두고 한 줄로만 보인다
    let busy: () -> [Session]
    @AppStorage("usageCompact") private var compactSetting = false
    private var compact: Bool { compactSetting || forceCompact }

    static func color(_ p: Double) -> Color {
        p < 50 ? Color(red: 0.42, green: 0.85, blue: 0.60) : p < 80 ? Color(red: 1.00, green: 0.70, blue: 0.36)
            : Color(red: 1.00, green: 0.42, blue: 0.42)
    }

    private var status: some View {
        Label(usage.tokenExpired ? "로그인 토큰 갱신 전 — 세션이 다음 요청을 보내면 보입니다"
              : usage.failed ? "사용량을 불러오지 못했습니다" : "사용량 불러오는 중…",
              systemImage: "gauge.with.dots.needle.33percent")
            .font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1)
    }

    // 간략 모드 한 줄 — 넓으면 이름·막대·비율, 좁으면 막대를 빼고, 더 좁으면 이름을 줄인다
    private func line(_ level: Int) -> some View {
        HStack(spacing: 8) {
            ForEach(Array(usage.limits.enumerated()), id: \.element.id) { i, l in
                if i > 0 { Rectangle().fill(Color.white.opacity(0.14)).frame(width: 0.5, height: 12) }
                CompactLimit(limit: l, level: level, now: now)
            }
        }
        .fixedSize()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: compact ? 0 : 9) {
            if compact {
                HStack(spacing: 8) {
                    AccountChip(accounts: accounts, compact: true, busy: busy)
                    if usage.limits.isEmpty {
                        status
                    } else {
                        ViewThatFits(in: .horizontal) { line(0); line(1); line(2) }
                    }
                    Spacer(minLength: 0)
                    if !forceCompact { UsageModeButton(compact: $compactSetting) }   // 중간 화면에서는 바꿔도 보이지 않으므로 숨긴다
                }
            } else {
                HStack(spacing: 8) {
                    AccountChip(accounts: accounts, compact: false, busy: busy)
                    Spacer(minLength: 4)
                    UsageModeButton(compact: $compactSetting)
                }
                if usage.limits.isEmpty { status }
                limitRows
            }
            if let n = accounts.notice {
                Label(n.text, systemImage: n.error ? "exclamationmark.triangle.fill" : "checkmark.circle.fill")
                    .font(.system(size: 10.5, weight: .medium))
                    .foregroundStyle(n.error ? Color(red: 1.00, green: 0.58, blue: 0.45) : Color(red: 0.42, green: 0.85, blue: 0.60))
                    .lineLimit(2)
                    .padding(.top, compact ? 6 : 0)
                    .transition(.opacity.combined(with: .move(edge: .top)))
            }
        }
        .padding(.horizontal, compact ? 8 : 12)
        .padding(.vertical, compact ? 6 : 12)
        .background(RoundedRectangle(cornerRadius: 14).fill(Color.white.opacity(0.05)))
        .overlay(RoundedRectangle(cornerRadius: 14).stroke(Color.white.opacity(0.08), lineWidth: 0.5))
        .animation(.spring(response: 0.6, dampingFraction: 0.85), value: usage.limits)
        .animation(.easeOut(duration: 0.2), value: accounts.notice)
    }

    private var limitRows: some View {
        ForEach(usage.limits) { l in
            let c = Self.color(l.pct)
            VStack(alignment: .leading, spacing: 4) {
                HStack(spacing: 6) {
                    Text(l.label).font(.system(size: 12, weight: .semibold)).lineLimit(1).layoutPriority(1)
                    Spacer(minLength: 4)
                    Text(resetText(l.resets, now)).font(.system(size: 10.5)).foregroundStyle(.secondary)
                        .monospacedDigit()
                        .lineLimit(1)
                        .minimumScaleFactor(0.8)
                    Text("\(Int(l.pct.rounded()))%")
                        .font(.system(size: 13, weight: .bold)).monospacedDigit()
                        .foregroundStyle(c)
                        .frame(minWidth: 38, alignment: .trailing)
                        .contentTransition(.numericText())
                }
                GeometryReader { geo in
                    ZStack(alignment: .leading) {
                        Capsule().fill(Color.white.opacity(0.08))
                        Capsule()
                            .fill(LinearGradient(colors: [c.opacity(0.65), c], startPoint: .leading, endPoint: .trailing))
                            .frame(width: max(4, geo.size.width * CGFloat(min(l.pct, 100)) / 100))
                            .shadow(color: c.opacity(0.45), radius: 4)
                    }
                }
                .frame(height: 6)
            }
            .transition(.opacity)
        }
    }
}

// 간략 모드의 한도 하나 — 이름 · 작은 막대 · 비율. 마우스를 올리면 초기화 시각
struct CompactLimit: View {
    let limit: Limit
    let level: Int          // 0 이름·막대·비율, 1 이름·비율, 2 짧은 이름·비율
    let now: Date
    var body: some View {
        let c = UsagePanel.color(limit.pct)
        HStack(spacing: 4) {
            Text(level == 2 ? limit.tiny : limit.short)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.secondary)
            if level == 0 {
                ZStack(alignment: .leading) {
                    Capsule().fill(Color.white.opacity(0.10))
                    Capsule().fill(c).frame(width: max(2, 26 * CGFloat(min(limit.pct, 100)) / 100))
                }
                .frame(width: 26, height: 4)
            }
            Text("\(Int(limit.pct.rounded()))%")
                .font(.system(size: 12, weight: .bold)).monospacedDigit()
                .foregroundStyle(c)
                .contentTransition(.numericText())
        }
        .hint("\(limit.label) \(Int(limit.pct.rounded()))% · \(resetText(limit.resets, now))")
    }
}

// 사용 한도 카드 오른쪽 위의 간략·상세 바꾸기 단추
struct UsageModeButton: View {
    @Binding var compact: Bool
    @State private var hover = false
    var body: some View {
        Button { withAnimation(foldSpring) { compact.toggle() } } label: {
            Image(systemName: compact ? "rectangle.expand.vertical" : "rectangle.compress.vertical")
                .font(.system(size: 10, weight: .semibold))
                .foregroundStyle(hover ? .primary : .secondary)
                .frame(width: 24, height: 20)
                .background(Capsule().fill(Color.white.opacity(hover ? 0.14 : 0.06)))
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint(compact ? "자세히 보기 — 한도마다 막대와 초기화 시각" : "간략히 보기 — 한 줄로")
    }
}

// 지금 로그인 계정 — 누르면 저장해 둔 계정으로 전환하거나 계정을 추가하는 메뉴
struct AccountChip: View {
    @ObservedObject var accounts: AccountStore
    var compact = false     // 간략 모드에서는 아이콘만 — 계정은 도움말로 보인다
    let busy: () -> [Session]
    @State private var hover = false

    var body: some View {
        let c = accounts.current
        Button(action: showMenu) {
            HStack(spacing: 5) {
                Image(systemName: accounts.adding ? "person.crop.circle.badge.plus"
                      : accounts.switching ? "arrow.triangle.2.circlepath.circle.fill" : "person.crop.circle.fill")
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(Palette.accent)
                    .contentTransition(.symbolEffect(.replace))
                if !compact {
                    Text(accounts.adding ? "로그인 기다리는 중…" : c?.email ?? "로그인 정보 없음")
                        .font(.system(size: 11.5, weight: .semibold))
                        .lineLimit(1)
                        .truncationMode(.middle)
                    if !accounts.adding, let p = c?.plan, !p.isEmpty {
                        Text(p)
                            .font(.system(size: 9.5, weight: .bold))
                            .foregroundStyle(.secondary)
                            .padding(.horizontal, 5).padding(.vertical, 1)
                            .background(Capsule().fill(Color.white.opacity(0.08)))
                            .fixedSize()
                    }
                }
                if accounts.autoSwitch {
                    Image(systemName: "arrow.triangle.swap").font(.system(size: 9, weight: .bold)).foregroundStyle(.secondary)
                }
                Image(systemName: "chevron.down").font(.system(size: 8, weight: .bold)).foregroundStyle(.secondary)
            }
            .padding(.leading, compact ? 4 : 5).padding(.trailing, compact ? 6 : 8).padding(.vertical, 3)
            .background(Capsule().fill(Color.white.opacity(hover ? 0.12 : 0.05)))
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint("로그인 계정 \(c?.email ?? "없음")\(c.map { $0.org.isEmpty ? "" : " · \($0.org)" } ?? "") — 눌러서 다른 계정으로 전환하거나 계정을 추가합니다"
              + (accounts.autoSwitch ? " · 한도 자동 전환 켜짐" : ""))
    }

    private func showMenu() {
        let menu = NSMenu()
        menu.autoenablesItems = false
        let head = NSMenuItem(title: "Claude 계정 — 고르면 모든 세션이 그 계정으로 이어 갑니다", action: nil, keyEquivalent: "")
        head.isEnabled = false
        menu.addItem(head)
        var list = accounts.saved
        if let c = accounts.current, !list.contains(where: { $0.id == c.id }) { list.insert(c, at: 0) }
        for a in list {
            let isCurrent = a.id == accounts.current?.id
            let title = [a.email, a.plan].filter { !$0.isEmpty }.joined(separator: "  ·  ")
                + (!isCurrent && a.expired ? "  — 다시 로그인 필요" : "")
            let item = ActionItem(title, enabled: !accounts.switching) {
                guard !isCurrent else { return }
                a.expired ? accounts.beginAdd(email: a.email) : confirmSwitch(a)
            }
            item.state = isCurrent ? .on : .off
            if !a.org.isEmpty { item.toolTip = a.org }
            menu.addItem(item)
        }
        menu.addItem(.separator())
        if accounts.adding {
            menu.addItem(ActionItem("로그인 기다리는 중 — 계정 추가 취소") { accounts.cancelAdd() })
        } else {
            menu.addItem(ActionItem("계정 추가… (터미널·브라우저에서 로그인)") { accounts.beginAdd() })
        }
        let removable = accounts.saved.filter { $0.id != accounts.current?.id }
        if !removable.isEmpty {
            let sub = NSMenu()
            for a in removable { sub.addItem(ActionItem(a.email) { accounts.remove(a) }) }
            let item = NSMenuItem(title: "저장한 계정 지우기", action: nil, keyEquivalent: "")
            item.submenu = sub
            menu.addItem(item)
        }
        menu.addItem(.separator())
        let auto = ActionItem("한도가 차면 다른 계정으로 자동 전환") { accounts.autoSwitch.toggle() }
        auto.state = accounts.autoSwitch ? .on : .off
        auto.toolTip = "5시간·주간 한도가 차거나 세션이 한도에 걸려 멈추면 저장한 다른 계정으로 바꾸고, 멈춘 세션(Ghostty)을 이어서 진행합니다"
        menu.addItem(auto)
        menu.popUp(positioning: nil, at: NSEvent.mouseLocation, in: nil)
    }

    // 작업 중·승인 대기 세션이 있으면 이전 계정으로 일하는 중이라고 알리고 한 번 더 묻는다
    private func confirmSwitch(_ a: Account) {
        let working = busy()
        if !working.isEmpty {
            let names = working.prefix(5).map { s in
                "• \((s.cwd as NSString).lastPathComponent) — \(s.topic.isEmpty ? Palette.name(s.state) : s.topic)"
            }
            let alert = NSAlert()
            alert.messageText = "이전 계정으로 작업 중인 세션이 있습니다"
            alert.informativeText = """
            \(accounts.current?.email ?? "지금") 계정으로 작업 중이거나 승인을 기다리는 세션이 \(working.count)개 있습니다.
            \(names.joined(separator: "\n"))\(working.count > 5 ? "\n… 외 \(working.count - 5)개" : "")

            전환하면 모든 세션이 다음 요청부터 \(a.email) 계정으로 이어서 동작합니다(30초 안에 반영). 지금 받고 있는 응답은 이전 계정으로 마무리됩니다.

            계정을 전환할까요?
            """
            alert.addButton(withTitle: "전환")
            alert.addButton(withTitle: "취소")
            NSApp.activate(ignoringOtherApps: true)
            guard alert.runModal() == .alertFirstButtonReturn else { return }
        }
        accounts.switchTo(a)
    }
}

struct SessionRow: View {
    let s: Session
    let now: Date
    @ObservedObject private var hub = TerminalHub.shared
    @State private var hover = false
    // 서브에이전트 트리를 접어 둔 세션 id(쉼표로 이음) — 앱을 다시 켜도 유지한다
    @AppStorage("subCollapsed") private var subCollapsed = ""

    private var subOpen: Bool { !subCollapsed.split(separator: ",").contains(Substring(s.id)) }

    private func toggleSubs() {
        var ids = Set(subCollapsed.split(separator: ",").map(String.init))
        if ids.contains(s.id) { ids.remove(s.id) } else { ids.insert(s.id) }
        withAnimation(foldSpring) { subCollapsed = ids.sorted().joined(separator: ",") }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            row
            if subOpen {
                ForEach(Array(s.subagents.enumerated()), id: \.element.id) { i, sub in
                    SubagentRow(sub: sub, now: now, last: i == s.subagents.count - 1)
                        .transition(foldIn(i))
                }
            }
        }
        .animation(.spring(response: 0.4, dampingFraction: 0.85), value: s.subagents)
    }

    private func meta(level: Int) -> some View {
        let compact = level > 0
        return HStack(spacing: 5) {
            Text(s.state == "ended" ? "\(clock(s.updated)) 종료"
                 : s.state == "idle" ? "\(elapsed(s.since, now)) 전" : elapsed(s.since, now))
                .monospacedDigit()
                .fixedSize()
                .contentTransition(.numericText())
            PlaceChip(s: s, place: hub.place(s), appName: hub.appNames[s.id], mode: s.state == "ended" ? nil : hub.mode(s))
            // 터미널·백그라운드 구분은 작업경로 카드 안 구역 머리글이 맡는다
            if !s.subagents.isEmpty || s.shells > 0 || s.ctx != nil || s.brain != nil { Text("·") }
            if !s.subagents.isEmpty {
                SubToggle(count: s.subagents.count,
                          running: s.subagents.filter { $0.state != "done" }.count,
                          open: subOpen,
                          toggle: toggleSubs,
                          compact: compact)
            }
            if s.shells > 0 {
                Label(compact ? "\(s.shells)" : "셸 \(s.shells)개 실행 중", systemImage: "apple.terminal.on.rectangle")
                    .labelStyle(.titleAndIcon)
                    .foregroundStyle(Palette.color("working"))
                    .fixedSize()
                    .hint("셸 \(s.shells)개 실행 중")
            }
            if let b = s.brain { ModelLabel(brain: b, level: level) }
            if let ctx = s.ctx { ContextLabel(ctx: ctx, level: level) }
        }
        .font(.system(size: 11))
        .foregroundStyle(.secondary)
    }

    private var row: some View {
        let c = Palette.color(s.state)
        return HStack(alignment: .top, spacing: 10) {
            HStack(spacing: 4) {
                // 서브에이전트가 있는 세션만 접기·펼치기 화살표를 보인다 — 없는 세션도 자리는 비워 줄을 맞춘다
                if s.subagents.isEmpty {
                    Color.clear.frame(width: 14, height: 18)
                } else {
                    FoldChevron(open: subOpen, toggle: toggleSubs)
                }
                StateIcon(state: s.state)
                    .hint(Palette.name(s.state))   // 상태 배지를 뺐으므로 아이콘에 상태 이름을 단다
            }
            .padding(.top, 1)
            VStack(alignment: .leading, spacing: 3) {
                // 첫 줄: 제목 + 오른쪽 단추들. 정보 라벨은 둘째 줄을 가로로 다 쓴다 — 창이 좁아도 넘치지 않게
                HStack(alignment: .center, spacing: 8) {
                    Text(s.title)
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(s.state == "ended" ? .secondary : .primary)
                        .lineLimit(1)
                    Spacer(minLength: 4)
                    if s.state != "ended" { MuteToggle(s: s) }
                    SessionMenu(s: s)
                    if s.state == "ended" {
                        Button { try? FileManager.default.removeItem(atPath: s.file) } label: {
                            Image(systemName: "xmark.circle.fill")
                                .font(.system(size: 14))
                                .foregroundStyle(.secondary.opacity(hover ? 1 : 0.5))
                        }
                        .buttonStyle(.plain)
                        .hint("목록에서 지우기")
                    }
                }
                // 다 들어가면 글자까지, 좁으면 종류 라벨은 아이콘만·하위 작업은 아이콘 + 개수만, 더 좁으면 컨텍스트 막대·토큰을 뺀다
                ViewThatFits(in: .horizontal) {
                    meta(level: 0)
                    meta(level: 1)
                    meta(level: 2)
                }
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
                if s.state == "waiting", !s.message.isEmpty {
                    Label(s.message, systemImage: "hand.raised.fill")
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(c)
                        .lineLimit(2)
                        .transition(.move(edge: .top).combined(with: .opacity))
                }
            }
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity(hover ? 0.07 : 0)))
        .modifier(Shimmer(active: s.state == "working", color: c))
        .contentShape(Rectangle())
        .onHover { h in withAnimation(.easeOut(duration: 0.15)) { hover = h } }
        // 한 번 클릭·더블 클릭을 제스처 둘로 나누면 SwiftUI 가 더블 클릭 여부를 기다리느라 줄 안의 스위치·단추까지 0.5초 늦게 반응한다.
        // 제스처 하나로 받고 macOS 클릭 횟수로 나눈다 — 더블 클릭은 첫 클릭(앱 활성화) 뒤에 둘째 클릭에서 포커스·재개
        .onTapGesture { openSession(s, double: (NSApp.currentEvent?.clickCount ?? 1) >= 2) }
        .animation(.spring(response: 0.4, dampingFraction: 0.8), value: s.state)
    }
}

// 세션 줄 오른쪽 위의 … 메뉴 — 일시중지·재개·중단
struct SessionMenu: View {
    let s: Session
    @State private var hover = false

    // SwiftUI Menu(borderlessButton) 는 AppKit 팝업 단추라 확대하면 마우스 감지 영역까지 커졌다 줄어 떨리고,
    // 커서도 스스로 화살표로 되돌린다 — 별·X 단추와 같은 일반 버튼으로 두고 누르면 메뉴를 직접 띄운다
    var body: some View {
        Button(action: showMenu) {
            Image(systemName: "ellipsis")
                .font(.system(size: 12, weight: .bold))
                .foregroundStyle(.primary.opacity(hover ? 0.95 : 0.55))
                .frame(width: 26, height: 22)
                .background(Capsule().fill(Color.white.opacity(hover ? 0.16 : 0.06)))
                .contentShape(Capsule())
                .scaleEffect(hover ? 1.3 : 1)
                .animation(.spring(response: 0.25, dampingFraction: 0.6), value: hover)
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint("세션 일시중지·재개·중단")
    }

    private func showMenu() {
        let menu = NSMenu()
        menu.autoenablesItems = false
        let s = self.s
        switch s.state {
        case "ended":
            menu.addItem(ActionItem(s.paused ? "세션 재개 (이어서 진행)" : "세션 다시 열기") { resumeSession(s, carryOn: s.paused) })
        case "paused":
            menu.addItem(ActionItem("세션 재개") { resumePaused(s) })
            menu.addItem(.separator())
            menu.addItem(ActionItem("세션 중단") { stopSession(s) })
        default:
            menu.addItem(ActionItem("일시중지", enabled: s.state != "idle" && canDrive(s)) { pauseSession(s) })
            menu.addItem(.separator())
            menu.addItem(ActionItem("세션 중단") { stopSession(s) })
        }
        menu.popUp(positioning: nil, at: NSEvent.mouseLocation, in: nil)
    }
}

// 누르면 클로저를 실행하는 메뉴 항목
final class ActionItem: NSMenuItem {
    private let run: @MainActor () -> Void
    init(_ title: String, enabled: Bool = true, run: @escaping @MainActor () -> Void) {
        self.run = run
        super.init(title: title, action: #selector(fire), keyEquivalent: "")
        target = self
        isEnabled = enabled
    }
    required init(coder: NSCoder) { fatalError() }
    @objc private func fire() { MainActor.assumeIsolated { run() } }   // 메뉴 동작은 메인 스레드에서 온다
}

// 트리 연결선: 위에서 내려와 오른쪽으로 꺾이고, 마지막 줄이 아니면 아래로 계속 이어진다
struct TreeBranch: Shape {
    let last: Bool
    func path(in r: CGRect) -> Path {
        var p = Path()
        let x = r.midX, y = r.midY
        p.move(to: CGPoint(x: x, y: r.minY))
        p.addLine(to: CGPoint(x: x, y: last ? y - 5 : r.maxY))
        if last { p.addQuadCurve(to: CGPoint(x: x + 5, y: y), control: CGPoint(x: x, y: y)) } else {
            p.move(to: CGPoint(x: x, y: y))
        }
        p.addLine(to: CGPoint(x: r.maxX, y: y))
        return p
    }
}

// 세션 줄 왼쪽의 접기·펼치기 화살표 — 그룹 카드 머리의 화살표와 같은 모양
struct FoldChevron: View {
    let open: Bool
    let toggle: () -> Void
    @State private var hover = false

    var body: some View {
        Button(action: toggle) {
            Image(systemName: "chevron.right")
                .font(.system(size: 11, weight: .bold))
                .foregroundStyle(.primary.opacity(hover ? 1 : 0.65))
                .rotationEffect(.degrees(open ? 90 : 0))
                .frame(width: 14, height: 18)
                .background(RoundedRectangle(cornerRadius: 4).fill(Color.white.opacity(hover ? 0.12 : 0)))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint(open ? "서브에이전트 목록 접기" : "서브에이전트 목록 펼치기")
    }
}

// 세션 줄의 서브에이전트 트리 접기·펼치기 단추
// 세션별 알림 켜기·끄기 — 끄면 작업 완료·승인 대기 패널과 소리를 모두 띄우지 않는다
struct MuteToggle: View {
    let s: Session
    @Environment(\.notifyLocked) private var locked
    @State private var hover = false
    // 누른 순간의 값 — 표지 파일을 다시 읽어 s.muted 가 따라올 때까지 이 값으로 바로 보인다
    @State private var pending: Bool?

    var body: some View {
        let on = pending ?? !s.muted
        Toggle("", isOn: Binding(get: { on }, set: { v in
            withAnimation(.spring(response: 0.22, dampingFraction: 0.8)) { pending = v }
            setMuted(s, !v)
        }))
            .toggleStyle(BoardSwitch(tint: Palette.color("working"), hover: hover, locked: locked))
            .labelsHidden()
            .disabled(locked)
            .modifier(ClickableIf(active: !locked, hover: $hover))
            .onChange(of: s.muted) { pending = nil }
            .hint(locked ? "전체 알림이 꺼져 있어 바꿀 수 없습니다 — 윗줄 종 단추로 전체 알림을 켜면 이 세션의 설정(\(on ? "켜짐" : "꺼짐"))대로 돌아갑니다"
                  : !on ? "알림 꺼짐 — 켜면 작업 완료·승인 대기 알림을 다시 띄웁니다" : "알림 켜짐 — 끄면 이 세션의 작업 완료·승인 대기 알림을 띄우지 않습니다")
    }
}

// 직접 그린 스위치 — macOS 기본 스위치는 창이 비활성이면 회색으로 바뀌어 켜짐·꺼짐이 안 보인다
struct BoardSwitch: ToggleStyle {
    let tint: Color
    var hover = false
    var locked = false      // 전체 알림 꺼짐 — 회색으로 흐리게, 손잡이는 세션 설정 위치 그대로(다시 켜면 돌아갈 상태)

    func makeBody(configuration: Configuration) -> some View {
        let on = configuration.isOn
        let fill = locked ? Color.white.opacity(0.12) : on ? tint : Color.white.opacity(hover ? 0.22 : 0.14)
        // 탭 제스처 대신 버튼 — 세션 줄의 더블 클릭 판별을 기다리지 않고 누르는 즉시 반응한다
        Button { configuration.isOn.toggle() } label: {
        ZStack(alignment: on ? .trailing : .leading) {
            Capsule()
                .fill(fill)
                .overlay(Capsule().stroke(Color.white.opacity(locked ? 0.35 : on ? 0 : 0.18),
                                          style: StrokeStyle(lineWidth: 0.5, dash: locked ? [2, 2] : [])))
            // 손잡이 안의 종 아이콘으로 알림 스위치임을 알린다 — 켜짐 종, 꺼짐 빗금 종
            Circle()
                .fill(Color.white.opacity(locked ? 0.45 : on ? 1 : 0.75))
                .shadow(color: .black.opacity(locked ? 0 : 0.3), radius: 1, y: 0.5)
                .overlay(
                    Image(systemName: locked || !on ? "bell.slash.fill" : "bell.fill")
                        .font(.system(size: 8, weight: .bold))
                        .foregroundStyle(locked ? Color.black.opacity(0.6) : on ? tint : Color.black.opacity(0.55))
                        .contentTransition(.symbolEffect(.replace))
                )
                .padding(2)
        }
        .frame(width: 36, height: 20)
        .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .animation(.spring(response: 0.22, dampingFraction: 0.8), value: on)
    }
}

// 윗줄 전체 알림 단추 — 꺼지면 붉은 캡슐 "알림 꺼짐"(좁으면 아이콘만)으로 눈에 띄게
struct AllMuteButton: View {
    let muted: Bool
    let toggle: () -> Void
    @State private var hover = false

    var body: some View {
        let c = Color(red: 1.00, green: 0.45, blue: 0.45)
        Button(action: toggle) {
            ViewThatFits(in: .horizontal) {
                HStack(spacing: 4) {
                    Image(systemName: muted ? "bell.slash.fill" : "bell.fill")
                    if muted { Text("알림 꺼짐").fixedSize() }
                }
                Image(systemName: muted ? "bell.slash.fill" : "bell.fill")
            }
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(muted ? c : hover ? .primary : .secondary)
            .contentTransition(.symbolEffect(.replace))
            .padding(.horizontal, 8)
            .frame(minWidth: 28, minHeight: 24)
            .background(Capsule().fill(muted ? c.opacity(0.16) : Color.white.opacity(hover ? 0.12 : 0.07)))
            .overlay(Capsule().stroke(muted ? c.opacity(0.4) : .clear, lineWidth: 0.5))
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint(muted ? "전체 알림 꺼짐 — 누르면 다시 켜고, 세션마다 정해 둔 알림 설정으로 돌아갑니다"
              : "전체 알림 끄기 — 모든 세션의 작업 완료·승인 대기 알림을 띄우지 않습니다")
    }
}

// 작업경로 카드 안 구역 머리글 — 접기 화살표·종류 아이콘·이름, 상태별 색 점·개수(있는 상태만)와 가는 선. 누르면 구역을 접고 편다
struct KindSection: View {
    let headless: Bool
    let sessions: [Session]
    let collapsed: Bool
    let toggle: () -> Void
    @State private var hover = false
    var body: some View {
        let c = headless ? Color(red: 0.72, green: 0.55, blue: 1.00) : Color(red: 0.32, green: 0.78, blue: 0.86)
        HStack(spacing: 6) {
            Image(systemName: "chevron.right")
                .font(.system(size: 8, weight: .bold))
                .rotationEffect(.degrees(collapsed ? 0 : 90))
                .frame(width: 8)
            Image(systemName: headless ? "moon.stars.fill" : "terminal.fill").font(.system(size: 9, weight: .semibold))
            Text(headless ? "백그라운드" : "터미널").font(.system(size: 10.5, weight: .semibold))
            ForEach(["waiting", "working", "paused", "idle"], id: \.self) { st in
                let n = sessions.filter { $0.state == st }.count
                if n > 0 {
                    HStack(spacing: 3) {
                        Circle().fill(Palette.color(st)).frame(width: 6, height: 6)
                        Text("\(n)").font(.system(size: 10, weight: .semibold)).monospacedDigit()
                            .foregroundStyle(.secondary)
                            .contentTransition(.numericText())
                    }
                    .hint("\(Palette.name(st)) \(n)개")
                }
            }
            Rectangle().fill(c.opacity(0.22)).frame(height: 0.5)
        }
        .foregroundStyle(c.opacity(hover ? 1 : 0.85))
        .padding(.horizontal, 10)
        .padding(.vertical, 3)
        .background(RoundedRectangle(cornerRadius: 7).fill(Color.white.opacity(hover ? 0.05 : 0)))
        .padding(.top, 1)
        .contentShape(Rectangle())
        .clickable($hover)
        .onTapGesture { withAnimation(foldSpring) { toggle() } }
        .hint((headless ? "터미널 없이 도는 세션 — 더블 클릭으로 이동할 터미널이 없습니다" : "터미널 창에서 도는 세션 — 더블 클릭하면 그 터미널로 이동합니다")
              + (collapsed ? " (누르면 펼치기)" : " (누르면 접기)"))
    }
}

struct SubToggle: View {
    let count: Int
    let running: Int
    let open: Bool
    let toggle: () -> Void
    var compact = false     // 줄이 좁으면 아이콘 + 개수만 보인다
    @State private var hover = false

    var body: some View {
        let c = Color(red: 0.95, green: 0.55, blue: 0.75)
        Button(action: toggle) {
            HStack(spacing: 4) {
                Image(systemName: "person.2.fill")
                    .font(.system(size: 9))
                let n = running > 0 && running < count ? "\(running)/\(count)" : "\(count)"
                Text(compact ? n : "하위 작업 \(n)")
                    .monospacedDigit()
                if !compact {
                    Text(open ? "· 접기" : "· 펼치기").foregroundStyle(.primary.opacity(hover ? 0.9 : 0.6))
                }
            }
            .fixedSize()
            .font(.system(size: 10, weight: .semibold))
            .foregroundStyle(c)
            .padding(.horizontal, 6)
            .padding(.vertical, 1.5)
            .background(Capsule().fill(c.opacity(hover ? 0.28 : 0.15)))
            .overlay(Capsule().stroke(c.opacity(0.25), lineWidth: 0.5))
            .contentShape(Capsule())
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint("하위 작업 \(count)개 — " + (open ? "서브에이전트 목록 접기" : "서브에이전트 목록 펼치기"))
    }
}

// labelStyle 을 조건에 따라 고르기 위한 형식 지우개
struct AnyLabelStyle: LabelStyle {
    private let make: (Configuration) -> AnyView
    init<S: LabelStyle>(_ style: S) { make = { AnyView(style.makeBody(configuration: $0)) } }
    func makeBody(configuration: Configuration) -> some View { make(configuration) }
}

struct SubagentRow: View {
    let sub: Sub
    let now: Date
    let last: Bool

    var body: some View {
        let done = sub.state == "done"
        HStack(spacing: 8) {
            TreeBranch(last: last)
                .stroke(Color.white.opacity(0.22), style: StrokeStyle(lineWidth: 1.2, lineCap: .round))
                .frame(width: 16)
            StateIcon(state: sub.boardState).scaleEffect(0.8)
            VStack(alignment: .leading, spacing: 2) {
                Text(sub.desc.isEmpty ? sub.type : sub.desc)
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(done ? .secondary : .primary)
                    .lineLimit(1)
                // 좁으면 라벨을 아이콘만, 더 좁으면 에이전트 종류·도구 이름을 뺀다 — 글자가 잘게 끊기지 않게
                ViewThatFits(in: .horizontal) {
                    meta(level: 0)
                    meta(level: 1)
                    meta(level: 2)
                }
                .font(.system(size: 10.5))
                .foregroundStyle(.secondary)
            }
            Spacer(minLength: 0)
        }
        .padding(.leading, 36)          // 부모 줄의 상태 아이콘 아래에 연결선이 오도록
        .padding(.trailing, 10)
        .frame(minHeight: 38)
        .opacity(done ? 0.6 : 1)
        .hint(sub.background ? "백그라운드로 실행된 서브에이전트" : "세션이 결과를 기다리는 서브에이전트")
 
    }

    private func meta(level: Int) -> some View {
        let done = sub.state == "done"
        return HStack(spacing: 5) {
            KindLabel(subagent: true, compact: level > 0)
            if level < 2, !sub.type.isEmpty { Text(sub.type).fixedSize() }
            if level < 2 { Text("·") }
            Text(done ? "\(elapsed(sub.started, sub.ended ?? now)) 걸려 완료"
                 : sub.state == "waiting" ? "승인 대기" : elapsed(sub.started, now))
                .monospacedDigit()
                .fixedSize()
                .contentTransition(.numericText())
            if let b = sub.brain { ModelLabel(brain: b, level: level) }
            if !done, !sub.tool.isEmpty {
                Text("·")
                Label(level < 2 ? sub.tool : "", systemImage: "wrench.and.screwdriver.fill")
                    .labelStyle(level < 2 ? AnyLabelStyle(.titleAndIcon) : AnyLabelStyle(.iconOnly))
                    .fixedSize()
                    .hint(sub.tool)
            }
        }
    }
}

// 카드 테두리 — 승인 대기가 있으면 테두리와 빛이 천천히 숨 쉰다
struct CardBorder: View {
    let color: Color
    let waiting: Bool
    var body: some View {
        if waiting {
            Ambient { now in
                let k = swing(now, 2.4)
                RoundedRectangle(cornerRadius: 16)
                    .stroke(color.opacity(0.25 + 0.5 * k), lineWidth: 1)
                    .shadow(color: color.opacity(0.35 * k), radius: 12, y: 3)
            }
        } else {
            RoundedRectangle(cornerRadius: 16).stroke(color.opacity(0.22), lineWidth: 1)
        }
    }
}

struct GroupCard: View {
    let g: Group
    let now: Date
    let collapsed: Bool
    let toggle: () -> Void
    let endedOpen: Bool
    let toggleEnded: () -> Void
    let pinned: Bool
    let togglePin: () -> Void
    let remove: () -> Void
    let clearEnded: () -> Void
    @ObservedObject private var hub = TerminalHub.shared
    @State private var hover = false
    @State private var endedHover = false
    // 접어 둔 구역 "<작업경로>|term" / "<작업경로>|bg" (줄바꿈으로 이음) — 앱을 다시 켜도 유지한다
    @AppStorage("kindCollapsed") private var kindCollapsed = ""

    private func kindFolded(_ kind: String) -> Bool {
        kindCollapsed.split(separator: "\n").contains(Substring("\(g.id)|\(kind)"))
    }

    private func toggleKind(_ kind: String) {
        let key = "\(g.id)|\(kind)"
        var keys = Set(kindCollapsed.split(separator: "\n").map(String.init))
        if keys.contains(key) { keys.remove(key) } else { keys.insert(key) }
        kindCollapsed = keys.sorted().joined(separator: "\n")
    }

    var body: some View {
        let c = Palette.color(g.urgent)
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 10) {
                Image(systemName: "chevron.right")
                    .font(.system(size: 11, weight: .bold))
                    .foregroundStyle(.secondary.opacity(hover ? 1 : 0.6))
                    .rotationEffect(.degrees(collapsed ? 0 : 90))
                    .frame(width: 10)
                    .hint(collapsed ? "경로 카드 펼치기" : "경로 카드 접기")
                // 접혀 있어도 작업 중(도는 링)·승인 대기(퍼지는 파문)가 보이게 폴더 자리에 상태 표시를 그린다
                ZStack {
                    if collapsed, let top = g.active.first {
                        StateIcon(state: top.state)
                    } else {
                        Image(systemName: "folder.fill")
                            .font(.system(size: 13, weight: .semibold))
                            .foregroundStyle(c)
                    }
                }
                .frame(width: 30, height: 30)
                .background(RoundedRectangle(cornerRadius: 8).fill(c.opacity(0.16)))
                .hint(collapsed ? g.active.first.map { "\(Palette.name($0.state)) · 누르면 펼치기" } ?? "작업 경로 · 누르면 펼치기"
                      : "작업 경로 · 누르면 접기")
                VStack(alignment: .leading, spacing: 1) {
                    Text(g.name).font(.system(size: 15, weight: .semibold)).lineLimit(1)
                    // 접혀 있으면 경로 대신 지금 하는 일을 상태 색으로 — 여럿이면 나머지 개수를 붙인다
                    if collapsed, let top = g.active.first {
                        HStack(spacing: 4) {
                            Text("\(Palette.name(top.state)) · \(doing(top))").lineLimit(1)
                            if g.active.count > 1 { Text("+\(g.active.count - 1)").fixedSize() }
                        }
                        .font(.system(size: 11, weight: .medium))
                        .foregroundStyle(Palette.color(top.state))
                    } else {
                        Text(g.path).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(1).truncationMode(.head)
                    }
                }
                Spacer()
                // 이 경로에서 바로 새 세션 — 기본 명령으로 보드 탭에서 띄운다
                HeaderButton(icon: "plus", tint: nil, help: "이 경로에서 새 세션 (\(defaultSessionCommand))") {
                    TerminalHub.shared.startSession(defaultSessionCommand, in: g.id)
                }
                HeaderButton(icon: pinned ? "star.fill" : "star", tint: pinned ? Color(red: 1.0, green: 0.78, blue: 0.28) : nil,
                             help: pinned ? "상단 고정 풀기" : "상단에 고정", action: togglePin)
                HeaderButton(icon: "xmark", tint: nil, help: "목록에서 지우기 — 새 활동이 생기면 다시 나타납니다", action: remove)
                HStack(spacing: 6) {
                    ForEach(["waiting", "working", "idle"], id: \.self) { st in
                        let n = g.sessions.filter { $0.state == st }.count
                        if n > 0 {
                            HStack(spacing: 3) {
                                Circle().fill(Palette.color(st)).frame(width: 6, height: 6)
                                Text("\(n)").font(.system(size: 11, weight: .semibold)).monospacedDigit()
                                    .contentTransition(.numericText())
                            }
                            .hint("\(Palette.name(st)) \(n)개")
                        }
                    }
                }
                .foregroundStyle(.secondary)
            }
            .padding(6)
            .background(RoundedRectangle(cornerRadius: 10).fill(Color.white.opacity(hover ? 0.06 : 0)))
            .modifier(Shimmer(active: collapsed && g.urgent == "working", color: c))   // 접힌 채 작업 중이면 세션 줄처럼 빛이 지나간다
            .padding(-6)
            .padding(.bottom, collapsed ? 0 : 2)
            .contentShape(Rectangle())
            .clickable($hover)
            .onTapGesture { withAnimation(foldSpring) { toggle() } }
            .contextMenu {
                Button(pinned ? "상단 고정 풀기" : "상단에 고정", action: togglePin)
                Button("목록에서 지우기", role: .destructive, action: remove)
            }

            if !collapsed {
                let ended = g.sessions.filter { $0.state == "ended" }
                VStack(alignment: .leading, spacing: 6) {
                    // 터미널 세션과 백그라운드 세션을 구역으로 나눈다 — 백그라운드는 더블 클릭으로 이동할 터미널이 없다.
                    // 세션이 있는 종류마다 머리글을 붙인다 — 한 종류만 있어도 터미널인지 백그라운드인지 보인다
                    let live = g.sessions.filter { $0.state != "ended" }
                    // 보드 탭 안에서 시작된 claude -p 는 탭과 함께 끝나므로 터미널 구역(탭 쪽)에 둔다
                    let terminal = live.filter { hub.place($0) != .background }
                    let background = live.filter { hub.place($0) == .background }
                    if !terminal.isEmpty {
                        KindSection(headless: false, sessions: terminal,
                                    collapsed: kindFolded("term"), toggle: { toggleKind("term") })
                    }
                    if !kindFolded("term") {
                        ForEach(Array(terminal.enumerated()), id: \.element.id) { i, s in
                            SessionRow(s: s, now: now)
                                .transition(foldIn(i))
                        }
                    }
                    if !background.isEmpty {
                        KindSection(headless: true, sessions: background,
                                    collapsed: kindFolded("bg"), toggle: { toggleKind("bg") })
                    }
                    if !kindFolded("bg") {
                        ForEach(Array(background.enumerated()), id: \.element.id) { i, s in
                            SessionRow(s: s, now: now)
                                .transition(foldIn(terminal.count + i))
                        }
                    }
                    if !ended.isEmpty {
                        // 화살표 크기·칸 폭·간격을 구역 머리글(KindSection)과 같게 — 화살표가 같은 세로줄에 선다
                        HStack(spacing: 6) {
                            Image(systemName: "chevron.right")
                                .font(.system(size: 8, weight: .bold))
                                .rotationEffect(.degrees(endedOpen ? 90 : 0))
                                .frame(width: 8)
                            Text("종료된 세션 \(ended.count)개")
                                .font(.system(size: 11, weight: .medium))
                                .contentTransition(.numericText())
                            Spacer()
                            if endedHover {
                                ClearEndedButton(count: ended.count, action: clearEnded)
                            }
                        }
                        .foregroundStyle(.secondary.opacity(endedHover ? 1 : 0.7))
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(RoundedRectangle(cornerRadius: 8).fill(Color.white.opacity(endedHover ? 0.06 : 0)))
                        .contentShape(Rectangle())
                        .clickable($endedHover)
                        .hint(endedOpen ? "종료된 세션 접기" : "종료된 세션 펼치기")
                        .onTapGesture { withAnimation(foldSpring) { toggleEnded() } }
                        .transition(.opacity)
                        if endedOpen {
                            VStack(alignment: .leading, spacing: 6) {
                                ForEach(Array(ended.enumerated()), id: \.element.id) { i, s in
                                    SessionRow(s: s, now: now)
                                        .transition(foldIn(i))
                                }
                            }
                            .transition(.opacity)
                        }
                    }
                }
                .transition(.opacity.combined(with: .scale(scale: 0.97, anchor: .top)))
            }
        }
        .padding(12)
        .clipShape(RoundedRectangle(cornerRadius: 16))   // 접고 펼치는 동안 줄이 카드 밖으로 비치지 않게
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(LinearGradient(colors: [c.opacity(0.10), Color.white.opacity(0.03)],
                                     startPoint: .topLeading, endPoint: .bottomTrailing))
        )
        .overlay(CardBorder(color: c, waiting: g.urgent == "waiting"))
        .shadow(color: g.urgent == "waiting" ? .clear : .black.opacity(0.25), radius: 8, y: 3)
        .animation(.easeInOut(duration: 0.4), value: g.urgent)
    }
}

// MARK: - 접기·펼치기

let foldSpring = Animation.spring(response: 0.38, dampingFraction: 0.86)

// 펼칠 때 줄마다 조금씩 늦게 위에서 내려오며 나타난다
func foldIn(_ i: Int) -> AnyTransition {
    .asymmetric(insertion: .opacity.combined(with: .offset(y: -10))
                    .animation(foldSpring.delay(Double(min(i, 8)) * 0.035)),
                removal: .opacity.animation(.easeOut(duration: 0.15)))
}

// 마우스를 올리면 오른쪽에 나타나는 접기·펼치기 안내
// 그룹 카드 머리의 작은 아이콘 단추(상단 고정·목록에서 지우기)
struct HeaderButton: View {
    let icon: String
    let tint: Color?
    let help: String
    let action: @MainActor () -> Void
    @State private var hover = false

    var body: some View {
        Button(action: action) {
            Image(systemName: icon)
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(tint ?? .primary.opacity(hover ? 0.95 : 0.55))
                .frame(width: 26, height: 26)
                .background(Circle().fill(Color.white.opacity(hover ? 0.16 : 0.06)))
                .contentShape(Circle())
                .scaleEffect(hover ? 1.3 : 1)   // 마우스를 올리면 커져서 누르기 쉽다
                .animation(.spring(response: 0.25, dampingFraction: 0.6), value: hover)
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint(help)
        .transition(.opacity.combined(with: .scale(scale: 0.8)))
    }
}

// "종료된 세션 N개" 줄에 마우스를 올리면 나오는 휴지통 — 그 작업경로의 종료된 세션을 한 번에 지운다
struct ClearEndedButton: View {
    let count: Int
    let action: () -> Void
    @State private var hover = false
    var body: some View {
        Button(action: action) {
            HStack(spacing: 3) {
                Image(systemName: "trash").font(.system(size: 9, weight: .bold))
                if hover { Text("모두 지우기").font(.system(size: 10, weight: .semibold)) }
            }
            .foregroundStyle(hover ? Color.red : .primary.opacity(0.8))
            .padding(.horizontal, 7)
            .padding(.vertical, 3)
            .background(Capsule().fill(hover ? Color.red.opacity(0.18) : Color.white.opacity(0.10)))
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint("종료된 세션 \(count)개를 목록에서 지우기")
        .transition(.opacity.combined(with: .move(edge: .trailing)))
    }
}

// 조건부 clickable — 잠긴 단추에는 손가락 커서를 보이지 않는다
struct ClickableIf: ViewModifier {
    let active: Bool
    @Binding var hover: Bool
    func body(content: Content) -> some View {
        if active { content.clickable($hover) } else { content }
    }
}

extension View {
    // 클릭할 수 있는 영역: 손가락 커서를 보이고 hover 상태를 갱신한다
    func clickable(_ hover: Binding<Bool>) -> some View {
        onContinuousHover { phase in
            var inside = false
            if case .active = phase { inside = true }
            (inside ? NSCursor.pointingHand : NSCursor.arrow).set()
            if hover.wrappedValue != inside {
                withAnimation(.easeOut(duration: 0.15)) { hover.wrappedValue = inside }
            }
        }
    }

    // 마우스를 올리면 곧 뜨는 짧은 설명 — 시스템 툴팁(.help) 대신 쓴다
    func hint(_ text: String) -> some View { modifier(HintArea(text: text)) }
}

// MARK: - 힌트

// 시스템 툴팁(.help)은 1초 넘게 기다려야 뜨고, 보드가 초당 30번 다시 그려지면 그 기다림이 자꾸 처음부터 다시 시작돼
// 몇 초씩 걸리거나 아예 뜨지 않는다. 카드·스크롤 영역에 잘리지 않게 보드 위의 작은 패널 창에 직접 띄운다
final class HintPanel {
    static let shared = HintPanel()
    static let delay: TimeInterval = 0.35
    private let panel = NSPanel(contentRect: .zero, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
    private let host = NSHostingController(rootView: HintLabel(text: ""))
    // 마우스가 들어가 있는 힌트 영역 — 안쪽 영역은 바깥 영역보다 나중에 들어가므로 마지막 것을 보인다
    private var stack: [(id: UUID, text: String)] = []
    private var pending: DispatchWorkItem?
    private var lastShown = Date.distantPast   // 방금까지 힌트가 떠 있었으면 옆 단추로 옮겨도 기다리지 않는다

    private init() {
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = true
        panel.ignoresMouseEvents = true
        panel.level = .popUpMenu   // 위에 고정한 보드(.floating)보다 위
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        host.sizingOptions = []
        panel.contentView = host.view
        // 누르거나 스크롤하면 시스템 툴팁처럼 사라진다
        NSEvent.addLocalMonitorForEvents(matching: [.leftMouseDown, .rightMouseDown, .scrollWheel]) { [weak self] e in
            self?.hide()
            return e
        }
    }

    func hover(_ id: UUID, _ text: String, _ inside: Bool) {
        // 마우스가 없던 영역이 사라질 때(줄 폭에 맞춰 라벨을 바꾸는 등)는 떠 있는 힌트를 건드리지 않는다
        if !inside, !stack.contains(where: { $0.id == id }) { return }
        stack.removeAll { $0.id == id }
        if inside { stack.append((id, text)) }
        hide()
        guard let top = stack.last else { return }
        let w = DispatchWorkItem { [weak self] in self?.show(top.text) }
        pending = w
        let warm = Date().timeIntervalSince(lastShown) < 0.4
        DispatchQueue.main.asyncAfter(deadline: .now() + (warm ? 0 : Self.delay), execute: w)
    }

    // 마우스가 보드 창 밖으로 나갔다 — 빠르게 나가면 영역마다의 나감 신호를 놓칠 수 있다
    func leaveAll() {
        stack.removeAll()
        hide()
    }

    func hide() {
        pending?.cancel()
        pending = nil
        if panel.isVisible {
            panel.orderOut(nil)
            lastShown = Date()
        }
    }

    private func show(_ text: String) {
        host.rootView = HintLabel(text: text)
        let size = host.sizeThatFits(in: CGSize(width: HintLabel.maxWidth, height: 1000))
        let m = NSEvent.mouseLocation
        guard let vf = (NSScreen.screens.first { $0.frame.contains(m) } ?? NSScreen.main)?.visibleFrame else { return }
        var o = NSPoint(x: m.x + 2, y: m.y - 22 - size.height)   // 시스템 툴팁처럼 커서 오른쪽 아래
        if o.y < vf.minY { o.y = m.y + 10 }                         // 아래 자리가 없으면 커서 위로
        o.x = max(vf.minX + 4, min(o.x, vf.maxX - size.width - 4))
        panel.setFrame(NSRect(origin: o, size: size), display: true)
        panel.invalidateShadow()       // 그림자를 새 크기에 맞춘다
        panel.orderFrontRegardless()   // 보드를 활성화하지 않는다 — 포커스는 쓰던 앱에 남긴다
    }
}

struct HintLabel: View {
    static let maxWidth: CGFloat = 280   // 긴 설명은 이 폭에서 줄을 바꾼다
    let text: String
    var body: some View {
        // 터미널 독의 힌트와 같은 색
        Text(text)
            .font(.system(size: 11, weight: .medium))
            .foregroundStyle(Color.white.opacity(0.92))
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .background(RoundedRectangle(cornerRadius: 6).fill(Color(red: 0.10, green: 0.10, blue: 0.15).opacity(0.96)))
            .overlay(RoundedRectangle(cornerRadius: 6).stroke(Color.white.opacity(0.14), lineWidth: 0.5))
    }
}

struct HintArea: ViewModifier {
    let text: String
    @State private var id = UUID()
    func body(content: Content) -> some View {
        content
            .onHover { HintPanel.shared.hover(id, text, $0) }
            // 상태가 바뀌어 뷰가 사라지면 마우스가 그 자리에 있어도 나감 신호가 오지 않는다
            .onDisappear { HintPanel.shared.hover(id, text, false) }
            .accessibilityHint(text)
    }
}

struct SummaryChip: View {
    let state: String
    let n: Int
    var filtering = false
    var tap: () -> Void = {}
    @State private var hover = false
    var body: some View {
        HStack(spacing: 5) {
            Circle().fill(Palette.color(state)).frame(width: 7, height: 7)
            Text("\(n)").font(.system(size: 12, weight: .bold)).monospacedDigit().contentTransition(.numericText())
        }
        .padding(.horizontal, 9)
        .padding(.vertical, 4)
        .background(Capsule().fill(Palette.color(state).opacity(filtering ? 0.35 : hover ? 0.25 : 0.15)))
        .overlay(Capsule().stroke(Palette.color(state).opacity(filtering ? 0.9 : 0), lineWidth: 1))
        .fixedSize()   // 윗줄이 좁아도 숫자가 잘리지 않게 — 모자라면 제목이 먼저 줄어든다
        .contentShape(Capsule())
        .onTapGesture(perform: tap)
        .clickable($hover)
        .hint(filtering ? "\(Palette.name(state))만 보는 중 — 누르면 모두 보기"
                        : "\(Palette.name(state)) \(n)개 — 누르면 \(Palette.name(state))만 보기")
    }
}

// 창 바탕: 짙은 남색 그라데이션 위에 위쪽에서 강조색 빛이 번진다 (블러가 비쳐 보이도록 살짝 투명)
struct Backdrop: View {
    var body: some View {
        ZStack {
            LinearGradient(colors: [Color(red: 0.12, green: 0.12, blue: 0.19), Color(red: 0.07, green: 0.07, blue: 0.11)],
                           startPoint: .top, endPoint: .bottom)
                .opacity(0.94)
            RadialGradient(colors: [Palette.accent.opacity(0.22), .clear], center: .topLeading,
                           startRadius: 0, endRadius: 360)
            RadialGradient(colors: [Color(red: 0.45, green: 0.50, blue: 1.0).opacity(0.14), .clear], center: .bottomTrailing,
                           startRadius: 0, endRadius: 420)
        }
        .ignoresSafeArea()
    }
}

// 설치된 Claude Code 버전 — 제목을 그릴 때마다 부르므로 1분 동안은 읽어 둔 값을 쓴다
private var versionCache: (at: Date, value: String?) = (.distantPast, nil)

func claudeVersion() -> String? {
    if Date().timeIntervalSince(versionCache.at) > 60 { versionCache = (Date(), readClaudeVersion()) }
    return versionCache.value
}

// ~/.local/bin/claude 가 가리키는 versions/<버전> 의 이름, 못 읽으면 versions 안 가장 높은 것
private func readClaudeVersion() -> String? {
    let home = NSHomeDirectory()
    if let t = try? FileManager.default.destinationOfSymbolicLink(atPath: home + "/.local/bin/claude") {
        let v = (t as NSString).lastPathComponent
        if v.first?.isNumber == true { return v }
    }
    let vs = (try? FileManager.default.contentsOfDirectory(atPath: home + "/.local/share/claude/versions")) ?? []
    return vs.filter { $0.first?.isNumber == true }.max { $0.compare($1, options: .numeric) == .orderedAscending }
}

// 감독의 말풍선 교체·눈 깜빡임 시계 — 보드가 1초마다 다시 그려져도 타이머가 새로 만들어지지 않게 객체에 둔다
final class ForemanClock: ObservableObject {
    @Published var line = 0
    @Published var blink = false
    // 말풍선이 바뀐 시각 — 감독이 그 순간 폴짝 뛴다
    private(set) var changedAt = Date().timeIntervalSinceReferenceDate
    private var timers: [Timer] = []
    init() {
        timers.append(Timer.scheduledTimer(withTimeInterval: 3.2, repeats: true) { [weak self] _ in self?.next() })
        timers.append(Timer.scheduledTimer(withTimeInterval: 3.7, repeats: true) { [weak self] _ in
            withAnimation(.easeIn(duration: 0.08)) { self?.blink = true }
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.14) { withAnimation(.easeOut(duration: 0.08)) { self?.blink = false } }
        })
    }
    func next() {
        changedAt = Date().timeIntervalSinceReferenceDate
        withAnimation(.spring(response: 0.38, dampingFraction: 0.55)) { line += 1 }
    }
    deinit { timers.forEach { $0.invalidate() } }
}

// 감독의 기분 — 세션 목록에서 정한다
enum Mood { case urgent, busy, idle, sleepy }

// 말풍선 한 줄 — 기분과 함께
struct Quip: Equatable {
    let text: String
    let mood: Mood
}

// 지금 세션 목록·사용 한도로 감독이 할 말을 만든다
func foremanQuips(groups: [Group], limits: [Limit], now: Date) -> [Quip] {
    let live = groups.flatMap(\.sessions).filter { $0.state != "ended" }
    let name = { (s: Session) in (s.cwd as NSString).lastPathComponent }
    let short = { (t: String, n: Int) in t.count > n ? String(t.prefix(n)) + "…" : t }
    let waiting = live.filter { $0.state == "waiting" }
    let working = live.filter { $0.state == "working" }
    let idle = live.filter { $0.state == "idle" }
    let paused = live.filter { $0.state == "paused" }
    var out: [Quip] = []

    if !waiting.isEmpty {
        out.append(Quip(text: waiting.count == 1 ? "승인 기다리는 세션이 있어요!" : "승인 대기 \(waiting.count)개! 확인해 줘요", mood: .urgent))
        for s in waiting.prefix(3) {
            out.append(Quip(text: s.topic.isEmpty ? "\(name(s)) 에서 기다려요!" : "\(name(s)): \(short(s.topic, 12)) — 승인 부탁!", mood: .urgent))
        }
        return out   // 승인 대기가 있으면 그것만 재촉한다
    }
    if !working.isEmpty {
        out.append(Quip(text: working.count == 1 ? "\(name(working[0])) 열일 중! 힘내!" : "세션 \(working.count)개 동시에 달리는 중!", mood: .busy))
        for s in working.prefix(2) {
            let mins = Int(now.timeIntervalSince(s.since) / 60)
            if mins >= 10 { out.append(Quip(text: "\(name(s)) \(mins)분째 작업 중… 조금만 더!", mood: .busy)) }
            else if !s.topic.isEmpty { out.append(Quip(text: "지금은 \(short(s.topic, 14)) 처리 중", mood: .busy)) }
        }
        let subs = working.reduce(0) { $0 + $1.subagents.filter { $0.state != "done" }.count }
        if subs > 0 { out.append(Quip(text: "서브에이전트 \(subs)명 출동!", mood: .busy)) }
        let shells = working.reduce(0) { $0 + $1.shells }
        if shells > 0 { out.append(Quip(text: "백그라운드 셸 \(shells)개 지켜보는 중", mood: .busy)) }
    }
    for s in live.filter({ ($0.ctx?.pct ?? 0) >= 80 }).prefix(2) {
        out.append(Quip(text: "\(name(s)) 컨텍스트 \(s.ctx!.pct)%! 정리할 때예요", mood: working.isEmpty ? .idle : .busy))
    }
    for l in limits where l.pct >= 80 {
        out.append(Quip(text: "\(l.label) 한도 \(Int(l.pct.rounded()))% — 아껴 써요!", mood: working.isEmpty ? .idle : .busy))
    }
    if !paused.isEmpty { out.append(Quip(text: "일시중지 \(paused.count)개, 잊지 마세요", mood: .idle)) }
    if working.isEmpty {
        if let s = idle.max(by: { $0.updated < $1.updated }) {
            let quiet = now.timeIntervalSince(s.updated)
            if quiet > 30 * 60 {
                out.append(Quip(text: "다들 쉬는 중… 할 일 없나요?", mood: .sleepy))
                out.append(Quip(text: "Zzz… 다음 지시 기다려요", mood: .sleepy))
            } else {
                out.append(Quip(text: "\(name(s)) 끝났어요! 확인해 볼까요?", mood: .idle))
                out.append(Quip(text: idle.count == 1 ? "다음 작업 뭐 할까요?" : "세션 \(idle.count)개 대기 중 — 다음 지시!", mood: .idle))
            }
        } else if out.isEmpty {
            out.append(Quip(text: "세션을 열어 일을 맡겨 주세요!", mood: .sleepy))
            out.append(Quip(text: "오늘도 화이팅!", mood: .sleepy))
        }
    }
    return out
}

// 꼬리가 오른쪽(감독 쪽)으로 난 말풍선
struct SpeechBubble: Shape {
    func path(in r: CGRect) -> Path {
        let tail: CGFloat = 7
        let body = CGRect(x: r.minX, y: r.minY, width: r.width - tail, height: r.height)
        var p = Path(roundedRect: body, cornerRadius: min(10, body.height / 2))
        let y = body.midY + 2
        p.move(to: CGPoint(x: body.maxX - 1, y: y - 5))
        p.addQuadCurve(to: CGPoint(x: r.maxX, y: y + 3), control: CGPoint(x: body.maxX + 2, y: y))
        p.addQuadCurve(to: CGPoint(x: body.maxX - 1, y: y + 4), control: CGPoint(x: body.maxX + 1, y: y + 4))
        p.closeSubpath()
        return p
    }
}

// 제목 오른쪽의 작은 캐릭터 — 세션 상태에 따라 움직이고, 재촉하고, 졸고, 말풍선으로 상황을 알려 준다. 누르면 다음 말.
// 그림은 고른 캐릭터(브레인롯 7종 또는 현장 감독)로 그린다 — 오른쪽 클릭이나 앱 메뉴 "캐릭터" 에서 고른다
struct Foreman: View {
    let groups: [Group]
    let limits: [Limit]
    let now: Date
    var compact = false     // 미니 화면 — 말풍선은 한 줄, 캐릭터는 조금 작게
    @StateObject private var clock = ForemanClock()
    @AppStorage("mascot") private var mascot: Mascot = .tralalero

    // 승인 대기가 없으면 돌아가는 말 끝에 캐릭터 유행어를 한 번 끼운다
    private func withCatchphrase(_ quips: [Quip]) -> [Quip] {
        guard let c = mascot.catchphrase, let first = quips.first, first.mood != .urgent else { return quips }
        return quips + [Quip(text: c, mood: first.mood)]
    }

    var body: some View {
        let quips = withCatchphrase(foremanQuips(groups: groups, limits: limits, now: now))
        let q = quips[clock.line % max(quips.count, 1)]
        let tint: Color = switch q.mood {
        case .urgent: Palette.color("waiting")
        case .busy: Palette.color("working")
        case .idle: Palette.accent
        case .sleepy: Color(white: 0.45)
        }
        HStack(alignment: .center, spacing: 2) {
            Text(q.text)
                .font(.system(size: 11, weight: .bold))
                .foregroundStyle(.white)
                .multilineTextAlignment(.leading)
                .lineLimit(compact ? 1 : 2)
                .fixedSize(horizontal: false, vertical: true)
                .padding(.leading, 9).padding(.trailing, 16).padding(.vertical, 5)
                .background(SpeechBubble().fill(tint.opacity(0.92)))
                .overlay(SpeechBubble().stroke(Color.white.opacity(0.18), lineWidth: 0.6))
                .frame(maxWidth: compact ? 230 : 170, alignment: .trailing)
                .id(q.text)
                .transition(.asymmetric(
                    insertion: .scale(scale: 0.4, anchor: .trailing).combined(with: .opacity),
                    removal: .scale(scale: 0.8, anchor: .trailing).combined(with: .opacity)))
            Ambient { t in
                ZStack {
                    if mascot == .foreman {
                        figure(mood: q.mood, t: t, sinceChange: t - clock.changedAt)
                    } else {
                        BrainrotFigure(kind: mascot, mood: q.mood, t: t, sinceChange: t - clock.changedAt, blink: clock.blink)
                    }
                }
                .scaleEffect(compact ? 0.75 : 1)
                .frame(width: compact ? 35 : 46, height: compact ? 33 : 44)
            }
        }
        .contentShape(Rectangle())
        .onTapGesture { clock.next() }
        .contextMenu {
            Picker("캐릭터", selection: $mascot) {
                ForEach(Mascot.allCases) { Text($0.name).tag($0) }
            }
            .pickerStyle(.inline)
        }
        .hint("\(mascot.name) — 누르면 다음 말 · 오른쪽 클릭으로 캐릭터 바꾸기")
    }

    // 동작 — 모두 시계 t 의 함수라 반복 애니메이션 없이 공유 시계 하나로 움직인다
    private func figure(mood: Mood, t: TimeInterval, sinceChange: Double) -> some View {
        let skin = mood == .urgent ? Color(red: 0.93, green: 0.42, blue: 0.32) : Color(red: 0.85, green: 0.47, blue: 0.34)
        // 말풍선이 바뀌면 폴짝 — 0.45초 동안 뛰었다 내려와 착지에 찌그러진다
        let hopP = min(max(sinceChange / 0.45, 0), 1)
        var hop = sinceChange < 0.45 ? -9 * sin(.pi * hopP) : 0
        var land = sinceChange >= 0.45 && sinceChange < 0.62 ? sin(.pi * (sinceChange - 0.45) / 0.17) : 0
        var tilt = 0.0, armDeg = 0.0, dx = 0.0, breathe = 0.0
        switch mood {
        case .urgent:
            // 제자리에서 연달아 뛰며 지휘봉을 머리 위로 흔든다
            let p = cycle(t, 0.55)
            hop = min(hop, -7 * sin(.pi * p))
            land = max(land, p > 0.85 ? sin(.pi * (p - 0.85) / 0.15) : 0)
            armDeg = 70 + 35 * sin(2 * .pi * t / 0.3)
            dx = 1.5 * sin(2 * .pi * t / 0.12)
        case .busy:
            // 지휘 — 지휘봉이 8자를 그리고 몸이 박자에 맞춰 기운다
            armDeg = 18 + 30 * sin(2 * .pi * t / 0.9) + 10 * sin(4 * .pi * t / 0.9)
            tilt = 7 * sin(2 * .pi * t / 0.9)
            hop = min(hop, -2.5 * abs(sin(2 * .pi * t / 0.9)))
        case .idle:
            // 말풍선 쪽을 두 번 콕콕 찌르고 쉰다
            let p = cycle(t, 2.4)
            let jab = p < 0.12 ? sin(.pi * p / 0.12) : (p > 0.18 && p < 0.30 ? sin(.pi * (p - 0.18) / 0.12) : 0)
            armDeg = 6 + 8 * jab
            dx = -2.5 * jab
            tilt = 5 * sin(2 * .pi * t / 2.4)
            breathe = swing(t, 1.6)
        case .sleepy:
            armDeg = -25
            tilt = 9 * sin(2 * .pi * t / 3.2)
            breathe = swing(t, 3.2)
        }
        let squash = land * 0.16
        return ZStack {
            // 지휘봉 든 팔 — 어깨를 축으로 돈다(양수 = 위로 든다)
            Capsule().fill(skin)
                .frame(width: 16, height: 4)
                .overlay(alignment: .leading) {
                    ZStack(alignment: .leading) {
                        Capsule().fill(Color(white: 0.95)).frame(width: 13, height: 2.5)
                        Circle().fill(mood == .urgent ? Color.yellow : Color.white).frame(width: 4, height: 4).offset(x: -2)
                    }
                    .offset(x: -11)
                }
                .offset(x: -9)
                .rotationEffect(.degrees(armDeg), anchor: .trailing)
                .offset(x: -6, y: 4)
            // 얼굴
            Circle().fill(skin)
                .frame(width: 28, height: 28)
                .overlay { eyes(mood: mood, t: t) }
                .overlay(alignment: .bottom) { mouth(mood: mood, t: t).offset(y: -5) }
                .overlay {
                    HStack(spacing: 13) {
                        ForEach(0..<2, id: \.self) { _ in
                            Ellipse().fill(Color(red: 1.0, green: 0.55, blue: 0.55).opacity(mood == .urgent ? 0.95 : 0.7)).frame(width: 5, height: 3)
                        }
                    }
                    .offset(y: 6)
                }
                .overlay(alignment: .top) {
                    // 안전모 — 착지할 때 살짝 들썩인다
                    UnevenRoundedRectangle(topLeadingRadius: 10, bottomLeadingRadius: 2, bottomTrailingRadius: 2, topTrailingRadius: 10)
                        .fill(Color(red: 1.0, green: 0.80, blue: 0.25))
                        .frame(width: 30, height: 10)
                        .offset(y: -4 - 2 * land)
                }
            // 머리 위 표시: 재촉 "!" · 졸음 "z"
            if mood == .urgent {
                Text("!").font(.system(size: 13, weight: .black)).foregroundStyle(Color.yellow)
                    .scaleEffect(1 + 0.25 * swing(t, 0.4))
                    .offset(x: 14, y: -19)
            } else if mood == .sleepy {
                ForEach(0..<2, id: \.self) { i in
                    let p = cycle(t + Double(i) * 1.2, 2.4)
                    Text("z").font(.system(size: 8 + 3 * p, weight: .bold)).foregroundStyle(.white.opacity(1 - p))
                        .offset(x: 12 + 6 * p, y: -10 - 14 * p)
                }
            }
        }
        .frame(width: 46, height: 44)
        .scaleEffect(x: 1 + squash + 0.03 * breathe, y: 1 - squash + 0.03 * breathe, anchor: .bottom)
        .rotationEffect(.degrees(tilt), anchor: .bottom)
        .offset(x: dx, y: hop + 2)
    }

    @ViewBuilder private func eyes(mood: Mood, t: TimeInterval) -> some View {
        HStack(spacing: 7) {
            ForEach(0..<2, id: \.self) { _ in
                switch mood {
                case .sleepy:
                    Capsule().fill(Color(white: 0.12)).frame(width: 5, height: 1.2)
                case .urgent:
                    Circle().fill(Color(white: 0.12)).frame(width: 5, height: 5)
                default:
                    Capsule().fill(Color(white: 0.12)).frame(width: 3.5, height: clock.blink ? 0.8 : 5)
                }
            }
        }
        // 말풍선 쪽(왼쪽)을 흘끗 본다
        .offset(x: mood == .sleepy ? 0 : -1.5, y: 1)
    }

    @ViewBuilder private func mouth(mood: Mood, t: TimeInterval) -> some View {
        switch mood {
        case .urgent:
            // 소리치는 입 — 열렸다 닫힌다
            Ellipse().fill(Color(white: 0.12)).frame(width: 7, height: 3 + 3 * swing(t, 0.3))
        case .busy:
            Capsule().fill(Color(white: 0.12)).frame(width: 8, height: 2)
        case .idle:
            // 웃는 입
            Path { p in p.move(to: CGPoint(x: 0, y: 0)); p.addQuadCurve(to: CGPoint(x: 8, y: 0), control: CGPoint(x: 4, y: 4)) }
                .stroke(Color(white: 0.12), style: StrokeStyle(lineWidth: 1.6, lineCap: .round))
                .frame(width: 8, height: 4)
        case .sleepy:
            Circle().fill(Color(white: 0.12)).frame(width: 3, height: 3)
        }
    }
}

// MARK: - 브레인롯 캐릭터

// 제목 오른쪽 캐릭터 — 브레인롯 7종과 기존 현장 감독. 고른 값은 "mascot" 에 남아 다시 켜도 유지된다
enum Mascot: String, CaseIterable, Identifiable {
    case tralalero, bombardiro, tungtung, lirili, patapim, boneca, cappuccino, foreman
    var id: String { rawValue }

    var name: String {
        switch self {
        case .tralalero: "트랄랄레로 트랄랄라"
        case .bombardiro: "봄바르디로 크로코딜로"
        case .tungtung: "퉁 퉁 퉁 사후르"
        case .lirili: "리릴리 라릴라"
        case .patapim: "브르르 브르르 파타핌"
        case .boneca: "보네카 암발라부"
        case .cappuccino: "카푸치노 아사시노"
        case .foreman: "현장 감독"
        }
    }

    // 말풍선에 가끔 끼우는 유행어 — 감독은 없다
    var catchphrase: String? {
        switch self {
        case .tralalero: "트랄랄레로 트랄랄라~"
        case .bombardiro: "봄바르디로 크로코딜로!"
        case .tungtung: "퉁 퉁 퉁 퉁 사후르!"
        case .lirili: "리릴리 라릴라~"
        case .patapim: "브르르 브르르 파타핌!"
        case .boneca: "보네카 암발라부!"
        case .cappuccino: "카푸치노 아사시노!"
        case .foreman: nil
        }
    }
}

private let ink = Color(white: 0.12)

// 0~1 비율 좌표의 꼭짓점을 이은 다각형 — 지느러미·이빨·꼬리 같은 작은 조각
struct Poly: Shape {
    let pts: [(CGFloat, CGFloat)]
    init(_ pts: [(CGFloat, CGFloat)]) { self.pts = pts }
    func path(in r: CGRect) -> Path {
        Path { p in
            p.addLines(pts.map { CGPoint(x: r.minX + $0.0 * r.width, y: r.minY + $0.1 * r.height) })
            p.closeSubpath()
        }
    }
}

// 만화 눈 — 흰자에 눈동자. 졸리거나 깜빡이면 감고, 재촉할 땐 눈동자가 작아진다(놀란 눈). 눈동자는 말풍선 쪽(왼쪽)을 본다
struct CartoonEye: View {
    let size: CGFloat
    let mood: Mood
    let blink: Bool
    var body: some View {
        if mood == .sleepy || blink {
            Capsule().fill(ink).frame(width: size, height: 1.2)
        } else {
            Circle().fill(Color.white).frame(width: size, height: size)
                .overlay(Circle().fill(ink).frame(width: size * (mood == .urgent ? 0.42 : 0.56)).offset(x: -size * 0.12))
        }
    }
}

// 아래로 뾰족한 흰 이빨 줄
struct Teeth: View {
    let n: Int
    let width: CGFloat
    let height: CGFloat
    var body: some View {
        HStack(spacing: 0) {
            ForEach(0..<n, id: \.self) { _ in
                Poly([(0, 0), (1, 0), (0.5, 1)]).fill(Color.white).frame(width: width / CGFloat(n), height: height)
            }
        }
    }
}

// 운동화 — 흰 신발에 빨간 곡선, 앞코가 왼쪽
struct Sneaker: View {
    var body: some View {
        RoundedRectangle(cornerRadius: 1.5).fill(Color.white).frame(width: 7, height: 3.5)
            .overlay(Path { p in
                p.move(to: CGPoint(x: 1.5, y: 1.4))
                p.addQuadCurve(to: CGPoint(x: 6, y: 0.8), control: CGPoint(x: 3, y: 3.3))
            }.stroke(Color(red: 0.95, green: 0.30, blue: 0.25), lineWidth: 0.8))
            .offset(x: -1.5)
    }
}

// 브레인롯 캐릭터 그림과 동작 — 감독처럼 모두 공유 시계 t 의 함수다.
// 말풍선이 바뀌면 폴짝 뛰고, 기분에 따라 박자(beat)가 빨라지거나 느려진다. 머리 위 "!"·"z" 와 크기(46×44)는 감독과 같다
struct BrainrotFigure: View {
    let kind: Mascot
    let mood: Mood
    let t: TimeInterval
    let sinceChange: Double
    let blink: Bool

    // 기분별 박자 — 재촉 0.3초 · 작업 0.6초 · 대기 1.6초 · 졸음 3초에 한 번
    private var period: Double {
        switch mood { case .urgent: 0.3; case .busy: 0.6; case .idle: 1.6; case .sleepy: 3.0 }
    }
    private var beat: Double { cycle(t, period) }          // 0 → 1
    private var wave: Double { sin(2 * .pi * beat) }       // -1 ~ 1

    var body: some View {
        // 말풍선이 바뀌면 폴짝 — 0.45초 동안 뛰었다 내려와 착지에 찌그러진다(감독과 같은 곡선)
        let hopP = min(max(sinceChange / 0.45, 0), 1)
        var hop = sinceChange < 0.45 ? -9 * sin(.pi * hopP) : 0
        var land = sinceChange >= 0.45 && sinceChange < 0.62 ? sin(.pi * (sinceChange - 0.45) / 0.17) : 0
        var tilt = 0.0, dx = 0.0, breathe = 0.0
        switch mood {
        case .urgent:
            let p = cycle(t, 0.55)
            hop = min(hop, -6 * sin(.pi * p))
            land = max(land, p > 0.85 ? sin(.pi * (p - 0.85) / 0.15) : 0)
            dx = 1.5 * sin(2 * .pi * t / 0.12)
        case .busy:
            tilt = 5 * sin(2 * .pi * t / 0.9)
            hop = min(hop, -2 * abs(sin(2 * .pi * t / 0.9)))
        case .idle:
            tilt = 4 * sin(2 * .pi * t / 2.4)
            breathe = swing(t, 1.6)
        case .sleepy:
            tilt = 8 * sin(2 * .pi * t / 3.2)
            breathe = swing(t, 3.2)
        }
        let squash = land * 0.16
        return ZStack {
            character
            if mood == .urgent {
                Text("!").font(.system(size: 13, weight: .black)).foregroundStyle(Color.yellow)
                    .scaleEffect(1 + 0.25 * swing(t, 0.4))
                    .offset(x: 14, y: -19)
            } else if mood == .sleepy {
                ForEach(0..<2, id: \.self) { i in
                    let p = cycle(t + Double(i) * 1.2, 2.4)
                    Text("z").font(.system(size: 8 + 3 * p, weight: .bold)).foregroundStyle(.white.opacity(1 - p))
                        .offset(x: 12 + 6 * p, y: -10 - 14 * p)
                }
            }
        }
        .frame(width: 46, height: 44)
        .scaleEffect(x: 1 + squash + 0.03 * breathe, y: 1 - squash + 0.03 * breathe, anchor: .bottom)
        .rotationEffect(.degrees(tilt), anchor: .bottom)
        .offset(x: dx, y: hop + 2)
    }

    @ViewBuilder private var character: some View {
        switch kind {
        case .tralalero: tralalero
        case .bombardiro: bombardiro
        case .tungtung: tungtung
        case .lirili: lirili
        case .patapim: patapim
        case .boneca: boneca
        case .cappuccino: cappuccino
        case .foreman: EmptyView()   // 감독은 Foreman.figure 가 그린다
        }
    }

    // 트랄랄레로 트랄랄라 — 파란 상어가 세 다리에 운동화를 신고 걷는다. 재촉하면 제자리 뛰기, 졸리면 멈춘다
    private var tralalero: some View {
        let blue = Color(red: 0.36, green: 0.58, blue: 0.86), belly = Color(red: 0.90, green: 0.95, blue: 1.0)
        let lift: Double = mood == .sleepy ? 0 : mood == .idle ? 1.5 : 3
        return ZStack {
            ForEach(0..<3, id: \.self) { i in
                let up = max(0, sin(2 * .pi * beat + Double(i) * 2.1))
                VStack(spacing: -1) {
                    Capsule().fill(blue).frame(width: 2.6, height: 8)
                    Sneaker()
                }
                .offset(x: Double(i - 1) * 7 - 2, y: 13 - lift * up)
            }
            // 꼬리 — 박자에 맞춰 흔든다
            Poly([(0, 0.5), (1, 0), (0.7, 0.5), (1, 1)]).fill(blue)
                .frame(width: 10, height: 13)
                .rotationEffect(.degrees(12 * wave), anchor: .leading)
                .offset(x: 18, y: -5)
            Poly([(0, 1), (0.55, 0), (1, 1)]).fill(blue).frame(width: 9, height: 8).offset(x: 2, y: -14)   // 등지느러미
            Capsule().fill(blue).frame(width: 32, height: 15).offset(y: -4)
            Capsule().fill(belly).frame(width: 22, height: 6).offset(x: -3, y: 0.5)
            HStack(spacing: 1.5) {   // 아가미
                ForEach(0..<3, id: \.self) { _ in Capsule().fill(ink.opacity(0.35)).frame(width: 0.8, height: 5) }
            }
            .offset(x: 1, y: -5)
            CartoonEye(size: 5.5, mood: mood, blink: blink).offset(x: -9, y: -7)
            Capsule().fill(ink).frame(width: 9, height: 1.2).offset(x: -11, y: -1.5)   // 입
            Teeth(n: 4, width: 8, height: 2.5).offset(x: -11, y: 0.3)
        }
    }

    // 봄바르디로 크로코딜로 — 악어 머리를 단 폭격기. 프로펠러가 돌고, 재촉하면 폭탄을 떨어뜨린다
    private var bombardiro: some View {
        let green = Color(red: 0.38, green: 0.64, blue: 0.34), dark = Color(red: 0.26, green: 0.46, blue: 0.24)
        let metal = Color(red: 0.62, green: 0.66, blue: 0.70)
        let spin = abs(cos(2 * .pi * t / (mood == .sleepy ? 1.2 : mood == .idle ? 0.25 : 0.1)))
        let drop = cycle(t, 0.6)
        return ZStack {
            if mood == .urgent {
                Capsule().fill(Color(white: 0.22)).frame(width: 4, height: 6)
                    .offset(x: 4, y: 6 + 16 * drop)
                    .opacity(1 - drop)
            }
            Poly([(0.3, 1), (0.8, 1), (1, 0), (0.6, 0)]).fill(metal.opacity(0.7))   // 먼 쪽 날개 — 몸 뒤로 비스듬히
                .frame(width: 18, height: 8).offset(x: 7, y: -9)
            Poly([(0, 1), (0.35, 0), (1, 0), (0.9, 1)]).fill(metal).frame(width: 8, height: 9).offset(x: 17, y: -8)   // 꼬리날개
            Capsule().fill(green).frame(width: 30, height: 12).offset(x: 3)
            HStack(spacing: 2) {   // 등 돌기
                ForEach(0..<3, id: \.self) { _ in Poly([(0, 1), (0.5, 0), (1, 1)]).fill(dark).frame(width: 2.5, height: 2) }
            }
            .offset(x: 9, y: -6.5)
            RoundedRectangle(cornerRadius: 3).fill(green).frame(width: 15, height: 6).offset(x: -15, y: 2)   // 주둥이
            Capsule().fill(dark).frame(width: 12, height: 1).offset(x: -15, y: 3)
            Teeth(n: 4, width: 10, height: 2).offset(x: -15, y: 4.5)
            Circle().fill(dark).frame(width: 1.5, height: 1.5).offset(x: -21, y: 0)   // 콧구멍
            Circle().fill(green).frame(width: 8, height: 8)   // 툭 튀어나온 눈두덩
                .overlay(CartoonEye(size: 5, mood: mood, blink: blink))
                .offset(x: -8, y: -5)
            Poly([(0.15, 0), (0.75, 0), (0.6, 1), (0, 1)]).fill(metal)   // 가까운 쪽 날개 — 몸 아래 앞으로
                .frame(width: 22, height: 8).offset(x: 3, y: 8)
            Capsule().fill(Color(white: 0.35)).frame(width: 7, height: 4.5).offset(x: -3, y: 10)   // 날개 위 엔진
            Capsule().fill(Color(white: 0.9).opacity(0.85)).frame(width: 2, height: 12)   // 프로펠러 — 길이가 줄었다 늘며 도는 것처럼 보인다
                .scaleEffect(y: 0.15 + 0.85 * spin)
                .offset(x: -7.5, y: 10)
        }
    }

    // 퉁 퉁 퉁 사후르 — 통나무 몸에 야구방망이. 박자에 맞춰 방망이로 바닥을 퉁퉁 내려친다
    private var tungtung: some View {
        let wood = Color(red: 0.78, green: 0.57, blue: 0.35), top = Color(red: 0.92, green: 0.76, blue: 0.52)
        let bark = Color(red: 0.52, green: 0.34, blue: 0.19), batC = Color(red: 0.95, green: 0.84, blue: 0.62)
        // 바닥을 치는 정도(0~1) — 박자 가운데서 짧게 내려친다. 졸리면 방망이를 내려놓는다
        let hit = mood == .sleepy ? 0 : pow(abs(sin(.pi * beat)), mood == .urgent ? 2 : 4)
        let angle = mood == .sleepy ? 160 : 20 + 130 * hit
        return ZStack {
            HStack(spacing: 5) { ForEach(0..<2, id: \.self) { _ in Capsule().fill(bark).frame(width: 3, height: 7) } }
                .offset(x: -4, y: 18)
            RoundedRectangle(cornerRadius: 5).fill(wood).frame(width: 20, height: 30).offset(x: -4, y: 1)
            HStack(spacing: 4) {   // 나뭇결
                ForEach(0..<3, id: \.self) { _ in Capsule().fill(bark.opacity(0.25)).frame(width: 0.8, height: 22) }
            }
            .offset(x: -4, y: 4)
            Ellipse().fill(top).frame(width: 20, height: 6)   // 윗면 나이테
                .overlay(Ellipse().stroke(bark.opacity(0.45), lineWidth: 0.6).frame(width: 11, height: 3))
                .offset(x: -4, y: -14)
            HStack(spacing: 3) {
                CartoonEye(size: 6.5, mood: mood, blink: blink)
                CartoonEye(size: 6.5, mood: mood, blink: blink)
            }
            .offset(x: -5, y: -6)
            if mood == .urgent {
                Ellipse().fill(ink).frame(width: 7, height: 3 + 3 * swing(t, 0.3)).offset(x: -5, y: 4)
            } else {
                Path { p in p.move(to: CGPoint(x: 0, y: 0)); p.addQuadCurve(to: CGPoint(x: 10, y: 0), control: CGPoint(x: 5, y: 5)) }
                    .stroke(ink, style: StrokeStyle(lineWidth: 1.4, lineCap: .round))
                    .frame(width: 10, height: 5)
                    .offset(x: -5, y: 5)
            }
            // 방망이 — 아래 절반을 비워 손이 회전 중심이 되게 한다
            Poly([(0.3, 1), (0, 0.06), (0.5, 0), (1, 0.06), (0.7, 1)]).fill(batC)
                .frame(width: 5, height: 17)
                .frame(width: 5, height: 34, alignment: .top)
                .rotationEffect(.degrees(angle))
                .offset(x: 8, y: 2)
            Circle().fill(bark).frame(width: 4, height: 4).offset(x: 8, y: 2)   // 손
            if hit > 0.7 {
                Text("퉁").font(.system(size: 7, weight: .black)).foregroundStyle(Color.yellow)
                    .opacity((hit - 0.7) / 0.3)
                    .offset(x: 19, y: 19)
            }
        }
    }

    // 리릴리 라릴라 — 코끼리 머리에 선인장 몸, 샌들, 자명종. 귀가 펄럭이고 코가 흔들리며, 재촉하면 자명종이 울린다
    private var lirili: some View {
        let cactus = Color(red: 0.36, green: 0.68, blue: 0.40), spine = Color(red: 0.86, green: 0.95, blue: 0.80)
        let grey = Color(red: 0.64, green: 0.66, blue: 0.72), pink = Color(red: 0.95, green: 0.72, blue: 0.76)
        let sandal = Color(red: 0.62, green: 0.40, blue: 0.22), bell = Color(red: 0.92, green: 0.30, blue: 0.28)
        let sway = (mood == .sleepy ? 4 : mood == .urgent ? 22 : 14) * wave
        let ring = mood == .urgent ? 14 * sin(2 * .pi * t / 0.08) : 0
        let spines: [(Double, Double)] = [(-3, 4), (2, 7), (-2, 12), (3, 14), (-10, 2)]
        return ZStack {
            HStack(spacing: 4) {   // 샌들 신은 발
                ForEach(0..<2, id: \.self) { _ in
                    VStack(spacing: 0) {
                        Capsule().fill(cactus).frame(width: 3, height: 5)
                        Capsule().fill(sandal).frame(width: 7, height: 2)
                    }
                }
            }
            .offset(y: 19)
            Capsule().fill(cactus).frame(width: 13, height: 20).offset(y: 8)   // 선인장 몸통
            Capsule().fill(cactus).frame(width: 4, height: 9).offset(x: -10, y: 4)   // 왼쪽 가지
            Capsule().fill(cactus).frame(width: 6, height: 3.5).offset(x: -8, y: 8)
            Capsule().fill(cactus).frame(width: 7, height: 3.5).offset(x: 8, y: 7)   // 자명종 든 오른쪽 가지
            ForEach(0..<spines.count, id: \.self) { i in
                Circle().fill(spine).frame(width: 1.2, height: 1.2).offset(x: spines[i].0, y: spines[i].1)
            }
            Ellipse().fill(grey).frame(width: 10, height: 14)   // 귀 — 펄럭인다
                .overlay(Ellipse().fill(pink).frame(width: 6, height: 9))
                .scaleEffect(x: 1 + 0.12 * wave, anchor: .trailing)
                .offset(x: -10, y: -10)
            Ellipse().fill(grey).frame(width: 10, height: 14)
                .overlay(Ellipse().fill(pink).frame(width: 6, height: 9))
                .scaleEffect(x: 1 + 0.12 * wave, anchor: .leading)
                .offset(x: 10, y: -10)
            Circle().fill(grey).frame(width: 18, height: 18).offset(y: -11)
            HStack(spacing: 5) {
                CartoonEye(size: 4.5, mood: mood, blink: blink)
                CartoonEye(size: 4.5, mood: mood, blink: blink)
            }
            .offset(y: -13)
            // 코 — 위 끝을 축으로 흔들린다(아래 절반을 비워 회전 중심을 위로)
            Capsule().fill(grey).frame(width: 5, height: 13)
                .frame(width: 5, height: 26, alignment: .bottom)
                .rotationEffect(.degrees(sway))
                .offset(y: -6)
            ZStack {   // 자명종
                HStack(spacing: 4) { Circle().fill(bell).frame(width: 4, height: 4); Circle().fill(bell).frame(width: 4, height: 4) }
                    .offset(y: -4.5)
                Circle().fill(Color.white).frame(width: 9, height: 9).overlay(Circle().stroke(bell, lineWidth: 1.2))
                Capsule().fill(ink).frame(width: 0.9, height: 3).offset(y: -1.2)
                Capsule().fill(ink).frame(width: 2.5, height: 0.9).offset(x: 1)
            }
            .rotationEffect(.degrees(ring))
            .offset(x: 13, y: 4)
        }
    }

    // 브르르 브르르 파타핌 — 큰 코의 원숭이 머리에 나무 몸. 잎이 살랑이고 코가 출렁이며, 재촉하면 부르르 떤다
    private var patapim: some View {
        let fur = Color(red: 0.72, green: 0.46, blue: 0.26), face = Color(red: 0.96, green: 0.82, blue: 0.64)
        let nose = Color(red: 0.94, green: 0.60, blue: 0.46), bark = Color(red: 0.50, green: 0.33, blue: 0.20)
        let leaf = Color(red: 0.42, green: 0.72, blue: 0.36)
        let shake = mood == .urgent ? 1.2 * sin(2 * .pi * t / 0.05) : 0
        let leaves: [(Double, Double)] = [(-10, 3), (-7, -1), (10, 3), (7, -1)]
        return ZStack {
            HStack(spacing: 6) { ForEach(0..<2, id: \.self) { _ in Capsule().fill(bark).frame(width: 8, height: 3.5) } }   // 큰 발
                .offset(y: 20)
            HStack(spacing: 5) { ForEach(0..<2, id: \.self) { _ in Capsule().fill(bark).frame(width: 3, height: 8) } }
                .offset(y: 15)
            RoundedRectangle(cornerRadius: 4).fill(bark).frame(width: 13, height: 14).offset(y: 6)   // 나무 몸통
            ForEach(0..<leaves.count, id: \.self) { i in
                Circle().fill(leaf).frame(width: 7, height: 7)
                    .offset(x: leaves[i].0, y: leaves[i].1 + 0.8 * wave * (i % 2 == 0 ? 1 : -1))
            }
            Circle().fill(fur).frame(width: 22, height: 22).offset(y: -9)
            Ellipse().fill(face).frame(width: 16, height: 14).offset(y: -7)
            HStack(spacing: 6) {
                CartoonEye(size: 4, mood: mood, blink: blink)
                CartoonEye(size: 4, mood: mood, blink: blink)
            }
            .offset(y: -11)
            Ellipse().fill(nose).frame(width: 9, height: 10)   // 큰 코 — 아래로 늘어져 출렁인다
                .scaleEffect(y: 1 + 0.08 * wave, anchor: .top)
                .offset(y: -2)
        }
        .offset(x: shake)
    }

    // 보네카 암발라부 — 개구리 머리, 타이어 몸통, 사람 다리. 타이어가 구르며 걷고, 재촉하면 입을 벌린다
    private var boneca: some View {
        let frog = Color(red: 0.44, green: 0.74, blue: 0.34), frogDark = Color(red: 0.30, green: 0.55, blue: 0.24)
        let skin = Color(red: 0.93, green: 0.74, blue: 0.58)
        let roll = mood == .sleepy ? 0 : t / period * 90
        return ZStack {
            ForEach(0..<2, id: \.self) { i in   // 사람 다리 — 번갈아 든다
                VStack(spacing: -1) {
                    Capsule().fill(skin).frame(width: 3, height: 9)
                    Capsule().fill(ink).frame(width: 6, height: 2.5).offset(x: -1.5)
                }
                .offset(x: Double(i) * 7 - 3.5, y: 16 - (mood == .sleepy ? 0 : 2 * max(0, sin(2 * .pi * beat + Double(i) * .pi))))
            }
            ZStack {   // 타이어 몸통 — 굴러간다
                Circle().fill(Color(white: 0.17))
                ForEach(0..<10, id: \.self) { k in
                    Rectangle().fill(Color(white: 0.32)).frame(width: 2, height: 3)
                        .offset(y: -8.5).rotationEffect(.degrees(Double(k) * 36))
                }
                Circle().fill(Color(white: 0.55)).frame(width: 8, height: 8)
                Circle().fill(Color(white: 0.30)).frame(width: 3, height: 3)
            }
            .frame(width: 20, height: 20)
            .rotationEffect(.degrees(roll))
            .offset(y: 4)
            Ellipse().fill(frog).frame(width: 26, height: 13).offset(y: -9)   // 개구리 머리
            ForEach([-7.0, 7.0], id: \.self) { x in   // 툭 튀어나온 눈
                Circle().fill(frog).frame(width: 9, height: 9)
                    .overlay(CartoonEye(size: 6, mood: mood, blink: blink))
                    .offset(x: x, y: -15)
            }
            Circle().fill(frogDark).frame(width: 2, height: 2).offset(x: -9, y: -8)   // 점
            Circle().fill(frogDark).frame(width: 1.5, height: 1.5).offset(x: 9, y: -7)
            if mood == .urgent {
                Ellipse().fill(Color(red: 0.55, green: 0.15, blue: 0.20)).frame(width: 12, height: 3 + 2 * swing(t, 0.3)).offset(y: -6)
            } else {
                Path { p in p.move(to: CGPoint(x: 0, y: 0)); p.addQuadCurve(to: CGPoint(x: 16, y: 0), control: CGPoint(x: 8, y: 3)) }
                    .stroke(frogDark, style: StrokeStyle(lineWidth: 1.2, lineCap: .round))
                    .frame(width: 16, height: 3)
                    .offset(y: -6)
            }
        }
    }

    // 카푸치노 아사시노 — 닌자 복면을 쓴 카푸치노 잔. 카타나를 휘두르고, 바쁠 땐 속도선을 남기며 오간다
    private var cappuccino: some View {
        let cup = Color(white: 0.96), coffee = Color(red: 0.55, green: 0.36, blue: 0.22)
        let foam = Color(red: 0.97, green: 0.89, blue: 0.76), band = Color(red: 0.88, green: 0.22, blue: 0.24)
        let slash: Double = switch mood {
        case .urgent: -70 + 110 * swing(t, 0.25)
        case .busy: -40 + 60 * swing(t, 0.6)
        case .idle: -35 + 6 * wave
        case .sleepy: 40
        }
        let dash = mood == .busy ? 3 * sin(2 * .pi * t / 0.6) : 0
        let flutter = 18 * sin(2 * .pi * t / (mood == .sleepy ? 2.0 : 0.35))
        return ZStack {
            if mood == .busy || mood == .urgent {   // 속도선
                ForEach(0..<3, id: \.self) { i in
                    Capsule().fill(Color.white.opacity(0.35)).frame(width: 6 + 3 * Double(i % 2), height: 1)
                        .offset(x: 20, y: Double(i) * 5 - 4)
                }
            }
            HStack(spacing: 5) { ForEach(0..<2, id: \.self) { _ in Capsule().fill(ink).frame(width: 2.5, height: 7) } }   // 다리
                .offset(y: 18)
            ForEach(0..<2, id: \.self) { i in   // 머리띠 꼬리 — 바람에 날린다
                Capsule().fill(band).frame(width: 8, height: 2)
                    .rotationEffect(.degrees(flutter + Double(i) * 25 - 10), anchor: .leading)
                    .offset(x: 14, y: -9 + Double(i) * 2)
            }
            Circle().stroke(cup, lineWidth: 2.5).frame(width: 8, height: 9).offset(x: 11, y: 4)   // 손잡이
            UnevenRoundedRectangle(topLeadingRadius: 2, bottomLeadingRadius: 7, bottomTrailingRadius: 7, topTrailingRadius: 2)
                .fill(cup).frame(width: 22, height: 21).offset(y: 3)
            Ellipse().fill(coffee).frame(width: 22, height: 6).offset(y: -7.5)
            Ellipse().fill(foam).frame(width: 15, height: 3.5).offset(y: -7.5)
            Rectangle().fill(Color(white: 0.14)).frame(width: 22, height: 7).offset(y: -1)   // 닌자 복면 — 눈만 드러난다
            Rectangle().fill(band).frame(width: 22, height: 1.5).offset(y: -4.5)
            HStack(spacing: 4) {
                CartoonEye(size: 4.5, mood: mood, blink: blink)
                CartoonEye(size: 4.5, mood: mood, blink: blink)
            }
            .offset(y: -1)
            Capsule().fill(ink).frame(width: 5, height: 2).offset(x: -12, y: 6)   // 팔
            // 카타나 — 손잡이 가운데(손)가 회전 중심이 되게 위로 붙여 둔다
            VStack(spacing: 0) {
                Capsule().fill(Color(white: 0.88)).frame(width: 2, height: 16)
                Capsule().fill(Color(red: 0.85, green: 0.70, blue: 0.30)).frame(width: 6, height: 1.6)
                Capsule().fill(Color(white: 0.15)).frame(width: 2.4, height: 6)
            }
            .frame(width: 6, height: 41, alignment: .top)
            .rotationEffect(.degrees(slash))
            .offset(x: -14, y: 6)
        }
        .offset(x: dash)
    }
}

// viewport = 목록 칸(스크롤 또는 빈 화면 안내) 높이, board = 보드 내용 전체 높이 — 창 크기 단계를 고를 때 쓴다
final class CardTopsBox { var values: [String: CGFloat] = [:]; var viewport: CGFloat = 0; var board: CGFloat = 0 }

// 오른쪽 독의 터미널 열기 단추 — 설치된 앱은 그 앱 아이콘으로 열 수 있고, 없으면 흐리게 막아 둔다
struct TerminalApp: Identifiable {
    let id: String      // 번들 ID — 명령줄 도구는 명령 이름
    let name: String
    var cli = false     // 앱이 아니라 터미널 안에서 도는 명령(herdr) — 설치는 실행 파일로 판단하고, 누르면 터미널 새 창에서 실행한다
}
let terminalApps = [TerminalApp(id: "com.mitchellh.ghostty", name: "Ghostty"),
                    TerminalApp(id: "com.googlecode.iterm2", name: "iTerm2"),
                    TerminalApp(id: "com.apple.Terminal", name: "터미널"),
                    TerminalApp(id: "com.stablyai.orca", name: "Orca"),
                    TerminalApp(id: "herdr", name: "herdr", cli: true)]

// 명령줄 도구를 찾는 곳 — 보드는 로그인 셸의 PATH 를 모르고 실행되므로 흔한 설치 위치를 직접 본다
func findCommand(_ name: String) -> URL? {
    let home = NSHomeDirectory()
    let dirs = ["\(home)/.local/bin", "\(home)/.\(name)/bin", "\(home)/.cargo/bin", "\(home)/bin",
                "/opt/homebrew/bin", "/usr/local/bin"]
    return dirs.map { URL(fileURLWithPath: "\($0)/\(name)") }
        .first { FileManager.default.isExecutableFile(atPath: $0.path) }
}

// herdr 처럼 앱 아이콘이 없는 명령줄 도구의 아이콘 — 어두운 사각형에 명령 첫 글자와 프롬프트
func commandIcon(_ name: String) -> NSImage {
    NSImage(size: NSSize(width: 64, height: 64), flipped: false) { r in
        let box = NSBezierPath(roundedRect: r.insetBy(dx: 4, dy: 4), xRadius: 13, yRadius: 13)
        NSGradient(starting: NSColor(red: 0.16, green: 0.24, blue: 0.20, alpha: 1),
                   ending: NSColor(red: 0.07, green: 0.11, blue: 0.09, alpha: 1))?.draw(in: box, angle: -90)
        NSColor.white.withAlphaComponent(0.18).setStroke()
        box.lineWidth = 1.5
        box.stroke()
        let text = NSAttributedString(string: ">" + String(name.prefix(1)), attributes: [
            .font: NSFont.monospacedSystemFont(ofSize: 26, weight: .heavy),
            .foregroundColor: NSColor(red: 0.45, green: 0.95, blue: 0.60, alpha: 1)])
        let sz = text.size()
        text.draw(at: NSPoint(x: r.midX - sz.width / 2, y: r.midY - sz.height / 2))
        return true
    }
}

final class TerminalAppStore: ObservableObject {
    static let shared = TerminalAppStore()
    @Published private(set) var urls: [String: URL] = [:]       // 설치된 앱 위치
    @Published private(set) var running: Set<String> = []
    // 앱 아이콘 — iconForFile 은 느려서 화면을 그릴 때마다 부르면 보드가 멈칫한다. 설치 위치가 바뀔 때만 다시 읽는다
    private(set) var icons: [String: NSImage] = [:]
    private var timer: Timer?
    private init() {
        refresh()
        // 설치·삭제는 드물어 1분마다, 실행·종료는 알림으로 바로 반영한다
        timer = Timer.scheduledTimer(withTimeInterval: 60, repeats: true) { [weak self] _ in self?.refresh() }
        let nc = NSWorkspace.shared.notificationCenter
        for n in [NSWorkspace.didLaunchApplicationNotification, NSWorkspace.didTerminateApplicationNotification] {
            nc.addObserver(forName: n, object: nil, queue: .main) { [weak self] _ in self?.refreshRunning() }
        }
    }
    func refresh() {
        var u: [String: URL] = [:]
        for a in terminalApps {
            u[a.id] = a.cli ? findCommand(a.id) : NSWorkspace.shared.urlForApplication(withBundleIdentifier: a.id)
        }
        if u != urls {
            var ic: [String: NSImage] = [:]
            for a in terminalApps where u[a.id] != nil {
                ic[a.id] = a.cli ? commandIcon(a.name) : NSWorkspace.shared.icon(forFile: u[a.id]!.path)
            }
            icons = ic
            urls = u
        }
        refreshRunning()
    }
    private func refreshRunning() {
        let r = Set(NSWorkspace.shared.runningApplications.compactMap(\.bundleIdentifier)).intersection(terminalApps.map(\.id))
        if r != running { running = r }
    }
    func open(_ id: String) {
        guard let url = urls[id] else { return }
        if terminalApps.first(where: { $0.id == id })?.cli == true { runInTerminal(shellQuote(url.path)); return }
        let cfg = NSWorkspace.OpenConfiguration()
        cfg.activates = true
        NSWorkspace.shared.openApplication(at: url, configuration: cfg)
    }
}

func shellQuote(_ s: String) -> String { "'" + s.replacingOccurrences(of: "'", with: "'\\''") + "'" }

// 셸 명령줄을 터미널 새 창에서 실행한다 — Ghostty 가 있으면 Ghostty, 없으면 macOS 터미널
func runInTerminal(_ shellCmd: String) {
    let quote = { (t: String) in "\"" + t.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") + "\"" }
    let script: String
    if NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.mitchellh.ghostty") != nil {
        script = """
        tell application "Ghostty"
          set cfg to new surface configuration
          set initial working directory of cfg to \(quote(NSHomeDirectory()))
          set initial input of cfg to \(quote(shellCmd)) & linefeed
          new window with configuration cfg
          activate
        end tell
        """
    } else {
        script = """
        tell application "Terminal"
          do script \(quote(shellCmd))
          activate
        end tell
        """
    }
    ghosttyQueue.async { _ = runScript(script) }
}

// 오른쪽 아래 독: Ghostty·iTerm2·터미널·Orca·herdr 를 바로 연다.
// 평소엔 창 오른쪽 아래 끝의 탭만 보이다가 그 탭(창 오른쪽 끝 아래쪽)에 마우스를 대면 보드 위로 미끄러져 나오고, 독에서 벗어나면 다시 숨는다
struct EdgeDock: View {
    static let size: CGFloat = 36, gap: CGFloat = 8, pad: CGFloat = 8
    static let trigger: CGFloat = 16    // 창 오른쪽 끝에서 이 폭(탭 폭) 안에 마우스가 오면 연다
    static let bottomInset: CGFloat = 34    // 창 아래 끝과 독 사이 — 오른쪽 아래 투명도 슬라이더를 가리지 않게
    static let hintHeight: CGFloat = 26
    static var width: CGFloat { size + pad * 2 }
    static var height: CGFloat { CGFloat(terminalApps.count) * (size + gap) - gap + pad * 2 }
    // 오른쪽 끝에 마우스를 대면 열리는 높이 — 창 아래에서 독 윗변 조금 위까지만. 위쪽 끝에서는 열지 않는다
    static var triggerHeight: CGFloat { bottomInset + height + 12 }
    @ObservedObject private var pointer = PointerTracker.shared
    @ObservedObject private var store = TerminalAppStore.shared
    @State private var open = false
    @State private var overPanel = false
    @State private var closing: DispatchWorkItem?
    @State private var hovered: Int?        // 마우스가 올라간 아이콘 순서

    // 오른쪽 끝이나 독 위에 있으면 연다. 벗어나면 조금 기다렸다 닫는다 — 끝에서 독으로 옮겨 가는 사이에 닫히지 않게
    private func sync() {
        closing?.cancel()
        if pointer.atEdge || overPanel {
            if !open { open = true }
        } else if open {
            let w = DispatchWorkItem { open = false; hovered = nil }
            closing = w
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.35, execute: w)
        }
    }

    private func index(at y: CGFloat) -> Int? {
        let k = Int(((y - Self.pad) / (Self.size + Self.gap)).rounded(.down))
        guard y >= Self.pad, k >= 0, k < terminalApps.count,
              y - Self.pad - CGFloat(k) * (Self.size + Self.gap) <= Self.size else { return nil }
        return k
    }

    var body: some View {
        ZStack(alignment: .bottomTrailing) {
            if open {
                panel.transition(.move(edge: .trailing).combined(with: .opacity))
            } else {
                tab.transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
        .frame(maxHeight: .infinity, alignment: .bottom)
        .padding(.bottom, Self.bottomInset)
        .animation(.spring(response: 0.32, dampingFraction: 0.86), value: open)
        .animation(.easeOut(duration: 0.2), value: pointer.inside)
        .onChange(of: pointer.atEdge) { sync() }
    }

    // 숨어 있을 때 창 오른쪽 아래 끝에 붙은 탭 — 여기에 마우스를 대면 터미널 독이 열린다는 표시.
    // 보드에 마우스가 들어오면 밝아지며 끝까지 튀어나온다. 열고 닫기는 창 전체 추적(PointerTracker)이 맡으므로 누르지는 않는다
    private var tab: some View {
        let shape = UnevenRoundedRectangle(topLeadingRadius: 8, bottomLeadingRadius: 8)
        return VStack(spacing: 6) {
            Image(systemName: "chevron.left").font(.system(size: 8, weight: .heavy))
            Image(systemName: "terminal.fill").font(.system(size: 11, weight: .semibold))
        }
        .foregroundStyle(Color.white.opacity(pointer.inside ? 0.95 : 0.6))
        .frame(width: Self.trigger, height: 58)
        .background(shape.fill(Palette.accent.opacity(pointer.inside ? 0.5 : 0.25)))
        .overlay(shape.stroke(Color.white.opacity(0.18), lineWidth: 0.5))
        .offset(x: pointer.inside ? 0 : 4)
        .allowsHitTesting(false)
    }

    private var panel: some View {
        // hover 는 아이콘마다가 아니라 독 전체에서 받는다 — 아이콘이 커지며 추적 영역이 바뀌면 들어옴·나감이 2ms 마다 번갈아 와 보드가 멈춘다
        VStack(spacing: Self.gap) {
            ForEach(Array(terminalApps.enumerated()), id: \.element.id) { i, app in
                LauncherButton(app: app, icon: store.icons[app.id], running: store.running.contains(app.id), hovered: hovered == i)
            }
        }
        .padding(Self.pad)
        .background(
            RoundedRectangle(cornerRadius: 16)
                .fill(Color(red: 0.11, green: 0.11, blue: 0.17).opacity(0.82))
                .background(.ultraThinMaterial, in: RoundedRectangle(cornerRadius: 16))
        )
        .overlay(RoundedRectangle(cornerRadius: 16).stroke(Color.white.opacity(0.14), lineWidth: 0.5))
        .shadow(color: .black.opacity(0.45), radius: 14, x: -3, y: 4)
        .contentShape(Rectangle())
        .onContinuousHover { phase in
            var i: Int?
            if case .active(let p) = phase { i = index(at: p.y) }
            if hovered != i { hovered = i }
            let over = phase != .ended
            if over != overPanel { overPanel = over; sync() }
        }
        .onTapGesture { location in
            if let i = index(at: location.y) { TerminalAppStore.shared.open(terminalApps[i].id) }
        }
        // 힌트는 독 왼쪽에 오버레이로 띄운다 — 아이콘을 옮겨 다니면 사라졌다 나타나지 않고 그 아이콘 높이로 미끄러진다
        .overlay(alignment: .topTrailing) { hint.allowsHitTesting(false) }
        .animation(.spring(response: 0.26, dampingFraction: 0.86), value: hovered)
        .padding(.trailing, 8)
    }

    @ViewBuilder private var hint: some View {
        if let i = hovered {
            let app = terminalApps[i]
            let installed = store.urls[app.id] != nil
            let running = store.running.contains(app.id)
            HStack(spacing: 5) {
                Image(systemName: installed ? (running ? "arrow.up.forward.app" : "plus.app") : "nosign")
                    .font(.system(size: 11, weight: .semibold))
                Text(installed ? (app.cli ? "\(app.name) 실행 · 터미널 새 창" : running ? "\(app.name) 창으로 이동" : "\(app.name) 실행")
                               : "\(app.name) · 설치되지 않음")
                    .font(.system(size: 12, weight: .semibold))
                    .lineLimit(1)
                    .contentTransition(.opacity)
            }
            .fixedSize()
            .foregroundStyle(Color.white.opacity(installed ? 0.95 : 0.55))
            .padding(.horizontal, 10)
            .frame(height: Self.hintHeight)
            .background(Capsule().fill(Color(red: 0.10, green: 0.10, blue: 0.15).opacity(0.96)))
            .overlay(Capsule().stroke(Color.white.opacity(0.14), lineWidth: 0.5))
            .shadow(color: .black.opacity(0.4), radius: 8, y: 2)
            // 힌트 가운데를 그 아이콘 가운데 높이에, 오른쪽 끝을 독 왼쪽 8pt 앞에 둔다
            .offset(x: -(Self.width + 8), y: Self.pad + CGFloat(i) * (Self.size + Self.gap) + (Self.size - Self.hintHeight) / 2)
            .transition(.opacity.combined(with: .offset(x: 6)))
        }
    }
}

// 창 안 마우스 위치 — 투명 모드(마우스가 올라오면 불투명)와 오른쪽 독(오른쪽 끝에 대면 열림)이 함께 본다.
// SwiftUI hover 는 단추·줄마다 따로 잡혀 경계를 지날 때 끊기므로, 창 전체에 AppKit 추적 영역 하나를 둔다
final class PointerTracker: NSResponder, ObservableObject {
    static let shared = PointerTracker()
    @Published private(set) var inside = false
    @Published private(set) var atEdge = false
    private weak var view: NSView?

    private override init() { super.init() }
    required init?(coder: NSCoder) { fatalError() }

    func attach(_ v: NSView) {
        view = v
        v.addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseEnteredAndExited, .mouseMoved, .activeAlways, .inVisibleRect],
                                         owner: self))
    }

    override func mouseEntered(with event: NSEvent) { mouseMoved(with: event) }

    override func mouseExited(with event: NSEvent) {
        if atEdge { atEdge = false }
        setInside(false)
        HintPanel.shared.leaveAll()
    }

    override func mouseMoved(with event: NSEvent) {
        guard let v = view else { return }
        setInside(true)
        let p = v.convert(event.locationInWindow, from: nil)   // 왼쪽 아래가 원점
        let edge = p.x >= v.bounds.maxX - EdgeDock.trigger && p.y - v.bounds.minY <= EdgeDock.triggerHeight
        if edge != atEdge { atEdge = edge }
    }

    private func setInside(_ v: Bool) {
        guard v != inside else { return }
        inside = v
        WindowOpacity.shared.apply()
    }
}

// 투명 모드 — 카카오톡 PC 처럼 오른쪽 아래 슬라이더로 창 전체(글자 포함)의 투명도를 정한다.
// 마우스를 올리거나 보드가 활성 창이 되면 불투명하게 보이고, 슬라이더를 움직이는 동안은 고른 투명도를 미리 보여 준다
@MainActor final class WindowOpacity: ObservableObject {
    static let shared = WindowOpacity()
    static let range: ClosedRange<Double> = 0.3...1
    @Published var level = min(max(UserDefaults.standard.object(forKey: "opacity") as? Double ?? 1, WindowOpacity.range.lowerBound),
                               WindowOpacity.range.upperBound) {
        didSet { UserDefaults.standard.set(level, forKey: "opacity"); apply() }
    }
    @Published var previewing = false { didSet { apply() } }

    func apply() {
        guard let w = window else { return }
        // 터미널을 펼친 동안에는 늘 불투명 — 화면 전체의 터미널이 비쳐 보이게 할 까닭이 없다
        let solid = !previewing && (PointerTracker.shared.inside || w.isKeyWindow || TerminalHub.shared.expanded)
        let target = solid ? 1 : CGFloat(level)
        guard abs(w.alphaValue - target) > 0.001 else { return }
        NSAnimationContext.runAnimationGroup { ctx in
            ctx.duration = previewing ? 0 : 0.22
            w.animator().alphaValue = target
        }
    }
}

// 오른쪽 아래 투명도 슬라이더 — 왼쪽으로 갈수록 투명하다
struct OpacitySlider: View {
    var compact = false     // 미니 화면에서는 막대를 짧게
    @ObservedObject private var opacity = WindowOpacity.shared
    @State private var hover = false
    @State private var settle: DispatchWorkItem?

    var body: some View {
        HStack(spacing: 6) {
            Image(systemName: "circle.lefthalf.filled").font(.system(size: 10, weight: .semibold))
            Slider(value: $opacity.level, in: WindowOpacity.range) { editing in
                settle?.cancel()
                if editing { opacity.previewing = true; return }
                // 놓은 뒤 잠깐 고른 투명도를 더 보여 주고 불투명으로 돌아간다(마우스가 보드 위에 있으므로)
                let w = DispatchWorkItem { opacity.previewing = false }
                settle = w
                DispatchQueue.main.asyncAfter(deadline: .now() + 1.2, execute: w)
            }
            .controlSize(.mini)
            .tint(Palette.accent)
            .frame(width: compact ? 48 : 84)
            Text("\(Int((opacity.level * 100).rounded()))%")
                .font(.system(size: 10, weight: .semibold)).monospacedDigit()
                .frame(width: 32, alignment: .trailing)
        }
        .foregroundStyle(.secondary)
        .opacity(hover ? 1 : 0.6)
        .onHover { h in withAnimation(.easeOut(duration: 0.15)) { hover = h } }
        .hint("투명도 — 보드에서 마우스가 벗어나 있고 다른 앱을 쓰는 동안 이 투명도로 보입니다. 마우스를 올리거나 보드를 누르면 불투명해집니다")
    }
}

// 손가락 커서 — 보드는 초당 30번 다시 그려서 NSCursor.set() 이 곧 화살표로 되돌아간다. macOS 15 부터는 SwiftUI 포인터 모양을 쓴다
struct HandPointer: ViewModifier {
    let on: Bool
    func body(content: Content) -> some View {
        if #available(macOS 15.0, *) { content.pointerStyle(on ? .link : nil) } else { content }
    }
}

struct LauncherButton: View {
    let app: TerminalApp
    let icon: NSImage?
    let running: Bool
    let hovered: Bool
    var body: some View {
        let installed = icon != nil
        ZStack {
            if let icon {
                Image(nsImage: icon).resizable().interpolation(.high)
            } else {
                RoundedRectangle(cornerRadius: 8).fill(Color.white.opacity(0.08))
                    .overlay(Image(systemName: "terminal").font(.system(size: 15, weight: .semibold)).foregroundStyle(.secondary))
                    .padding(3)
            }
        }
        .frame(width: EdgeDock.size, height: EdgeDock.size)
        .modifier(HandPointer(on: installed))
        .saturation(installed ? 1 : 0)
        .opacity(installed ? 1 : 0.35)
        .scaleEffect(installed && hovered ? 1.12 : 1)
        .animation(.spring(response: 0.25, dampingFraction: 0.6), value: hovered)
        // 실행 중이면 Dock 처럼 바깥쪽(오른쪽)에 점
        .overlay(alignment: .trailing) {
            Circle().fill(Color.white.opacity(running ? 0.8 : 0)).frame(width: 4, height: 4)
                .offset(x: EdgeDock.pad / 2 + 2)
        }
    }
}

struct ScrollRequest: Equatable { let id: String; let n: Int }

// 목록 카드마다 스크롤 좌표의 윗변 위치
struct CardTops: PreferenceKey {
    static var defaultValue: [String: CGFloat] = [:]
    static func reduce(value: inout [String: CGFloat], nextValue: () -> [String: CGFloat]) {
        value.merge(nextValue()) { $1 }
    }
}

struct BoardView: View {
    @ObservedObject var store: Store
    @ObservedObject var usage: UsageStore
    @ObservedObject var accounts: AccountStore
    @AppStorage("pinned") private var pinned = true
    @AppStorage("usageCompact") private var usageCompact = false
    @State private var foldHover = false
    @State private var checkHover = false
    @State private var pinHover = false
    // 요약 칩: 걸러 보는 상태·스크롤 요청
    @State private var filter: String?
    @State private var scrollRequest = ScrollRequest(id: "", n: 0)
    // 목록 아래로 가려진 카드 — 카드 위치는 스크롤 좌표로 모은다
    // 스크롤·애니메이션 프레임마다 위치가 바뀌어도 화면은 "가려진 카드 목록" 이 달라질 때만 다시 그린다
    @State private var tops = CardTopsBox()
    @State private var hiddenIds: [String] = []
    @State private var moreHover = false
    // 창 크기 단계 — 중간에서는 카드를 접힌 한 줄로 보이고, 그 단계에서 눌러 펼친 카드만 따로 기억한다(저장한 접힘 상태는 그대로)
    @ObservedObject private var fit = BoardFit.shared
    @State private var mediumOpen: Set<String> = []
    @ObservedObject private var updater = Updater.shared

    private func updateHidden() {
        guard tops.viewport > 0 else { return }
        let ids = shownGroups.map(\.id).filter { (tops.values[$0] ?? 0) > tops.viewport - 24 }
        if ids != hiddenIds { hiddenIds = ids }
    }

    // 목록 칸·보드 높이는 한 번의 배치에서 함께 바뀌므로 둘 다 갱신된 뒤에 단계 판정에 넘긴다.
    // 그때 그려진 단계(tier)를 함께 넘겨, 단계가 바뀌는 사이에 앞 단계의 높이가 새 단계 값으로 잡히지 않게 한다
    private func measure(_ tier: BoardTier) {
        DispatchQueue.main.async { BoardFit.shared.measured(tier, board: tops.board, list: tops.viewport) }
    }

    // 목록 칸의 높이 — 가려진 카드 계산과 단계 판정이 함께 쓴다
    private var listHeightReader: some View {
        let tier = fit.tier
        return GeometryReader { geo in
            Color.clear.onAppear { tops.viewport = geo.size.height; updateHidden(); measure(tier) }
                .onChange(of: geo.size.height) { _, h in tops.viewport = h; updateHidden(); measure(tier) }
        }
    }

    // 칩 한 번 = 그 상태만 보기(해당 카드를 펼치고 맨 위로), 같은 칩 다시 = 모두 보기
    private func chipTapped(_ st: String) {
        if filter == st { withAnimation(.easeOut(duration: 0.18)) { filter = nil }; return }
        let ids = store.groups.filter { $0.sessions.contains { $0.state == st } }.map(\.id)
        withAnimation(.easeOut(duration: 0.18)) {
            filter = st
            ids.forEach(store.reveal)
        }
        if let first = ids.first { scrollRequest = ScrollRequest(id: first, n: scrollRequest.n + 1) }
    }

    // 걸러 보는 중이면 그 상태 세션만 남긴 카드
    private var shownGroups: [Group] {
        guard let f = filter else { return store.groups }
        return store.groups.compactMap { g in
            let ss = g.sessions.filter { $0.state == f }
            return ss.isEmpty ? nil : Group(id: g.id, sessions: ss)
        }
    }

    // 화면 아래로 가려진 카드 수와 그중 승인 대기가 있는지
    private var below: (n: Int, waiting: Bool, first: String?) {
        let ids = Set(hiddenIds)
        let hidden = shownGroups.filter { ids.contains($0.id) }
        return (hidden.count, hidden.contains { $0.sessions.contains { $0.state == "waiting" } }, hidden.first?.id)
    }

    private var titleBlock: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 6) {
                Image(nsImage: appIcon).resizable().interpolation(.high).frame(width: 24, height: 24)
                Text("Claude Board").font(.system(size: 19, weight: .bold)).lineLimit(1)
                if !updater.current.isEmpty {
                    // build-board.sh 로 만든 개발용 빌드는 버전이 1.0 이다
                    Text(updater.current == "1.0" ? "dev" : updater.current)
                        .font(.system(size: 12, weight: .semibold, design: .monospaced))
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 6).padding(.vertical, 2)
                        .background(Capsule().fill(Color.white.opacity(0.08)))
                        .lineLimit(1).fixedSize()
                        .hint("Claude Board 버전")
                }
            }
            let active = store.groups.reduce(0) { $0 + $1.sessions.filter { $0.state != "ended" }.count }
            // Claude Code 버전은 부가 정보라 세션 수 옆에 흐리게 붙인다
            HStack(spacing: 0) {
                Text("활성 세션 \(active)개").contentTransition(.numericText())
                if let v = claudeVersion() {
                    Text(" · Claude Code \(v)").foregroundStyle(.tertiary)
                        .hint("설치된 Claude Code 버전")
                }
            }
            .font(.system(size: 12)).foregroundStyle(.secondary).lineLimit(1)
        }
    }

    private var headerControls: some View {
        HStack {
            HStack(spacing: 6) {
                ForEach(["waiting", "working", "idle"], id: \.self) { st in
                    let n = store.count(st)
                    if n > 0 {
                        SummaryChip(state: st, n: n, filtering: filter == st) { chipTapped(st) }
                            .transition(.scale.combined(with: .opacity))
                    }
                }
            }
            Spacer(minLength: 6)
            AllMuteButton(muted: store.allMuted) { store.setAllMuted(!store.allMuted) }
            // 업데이트 확인 — 새 버전이 있으면 아래 알림 줄에 업데이트 단추가, 없으면 "최신 버전입니다" 가 잠깐 뜬다
            Button { updater.check(manual: true) } label: {
                Image(systemName: "arrow.triangle.2.circlepath")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(checkHover ? .primary : .secondary)
                    .opacity(updater.checking ? 0.4 : 1)
                    .padding(.horizontal, 8)
                    .frame(minWidth: 28, minHeight: 24)
                    .background(Capsule().fill(Color.white.opacity(checkHover ? 0.12 : 0.07)))
            }
            .buttonStyle(.plain)
            .clickable($checkHover)
            .disabled(updater.checking)
            .hint("업데이트 확인 — GitHub 에 새 버전이 있는지 확인합니다")
            // 모두 접기 = 경로 카드를 다 접고 사용 한도를 간략(한 줄)으로, 모두 펼치기 = 다 펴고 상세로.
            // 중간 화면은 이미 카드가 접힌 한 줄·사용 한도가 한 줄이라 숨긴다
            let all = usageCompact && store.groups.allSatisfy { store.collapsed.contains($0.id) }
            if fit.tier == .full {
            Button {
                withAnimation(foldSpring) {
                    store.setAll(collapse: !all)
                    usageCompact = !all
                }
            } label: {
                // 마우스를 올려도 폭이 변하지 않게 글자는 도움말로만 보인다 — 폭이 바뀌면 "위에 고정" 이 줄었다 늘었다 한다
                Image(systemName: all ? "rectangle.expand.vertical" : "rectangle.compress.vertical")
                    .font(.system(size: 11, weight: .semibold))
                    .foregroundStyle(foldHover ? .primary : .secondary)
                    .padding(.horizontal, 8)
                    .frame(minWidth: 28, minHeight: 24)
                    .background(Capsule().fill(Color.white.opacity(foldHover ? 0.12 : 0.07)))
            }
            .buttonStyle(.plain)
            .clickable($foldHover)
            .hint(all ? "모두 펼치기 — 경로 카드를 펴고 사용 한도를 자세히" : "모두 접기 — 경로 카드를 접고 사용 한도를 한 줄로")
            }
            Button {
                withAnimation(.spring(response: 0.3, dampingFraction: 0.6)) { setPinned(!pinned) }
            } label: {
                // 윗줄이 좁으면 글자를 빼고 압정 아이콘만 보인다
                ViewThatFits(in: .horizontal) {
                    HStack(spacing: 4) {
                        Image(systemName: pinned ? "pin.fill" : "pin.slash")
                            .rotationEffect(.degrees(pinned ? 0 : 45))
                        Text(pinned ? "위에 고정" : "고정 안 함").fixedSize()
                    }
                    Image(systemName: pinned ? "pin.fill" : "pin.slash")
                        .rotationEffect(.degrees(pinned ? 0 : 45))
                }
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(pinned ? Palette.accent : .secondary)
                .padding(.horizontal, 9)
                .frame(height: 24)
                .background(Capsule().fill(pinned ? Palette.accent.opacity(0.16) : Color.white.opacity(0.07)))
                .overlay(Capsule().stroke(pinned ? Palette.accent.opacity(0.4) : .clear, lineWidth: 0.5))
            }
            .buttonStyle(.plain)
            .clickable($pinHover)
            .hint(pinned ? "항상 위에 표시 끄기" : "항상 위에 표시")
        }
    }

    // 창 크기 단계에 따라 미니 화면 또는 전체·중간 화면
    var body: some View {
        ZStack(alignment: .bottom) {
            if fit.tier == .mini {
                MiniBoard(store: store, usage: usage)
                    .background(Backdrop())
                    .preferredColorScheme(.dark)
            } else {
                board
            }
            TerminalNotice(hub: TerminalHub.shared)
        }
    }

    private var board: some View {
        let tier = fit.tier
        let medium = tier == .medium
        return VStack(alignment: .leading, spacing: 14) {
            // 윗줄은 늘 두 줄 — 제목 아래 줄에 상태 칩(왼쪽)과 단추(오른쪽).
            // 한 줄·두 줄을 폭에 따라 바꾸면 "알림 꺼짐" 글자가 생길 때마다 단추 줄이 위아래로 오간다
            VStack(alignment: .leading, spacing: 10) {
                HStack(alignment: .center, spacing: 8) {
                    titleBlock.layoutPriority(1)
                    Spacer(minLength: 0)
                    Foreman(groups: store.groups, limits: usage.limits, now: store.now)
                }
                headerControls
            }
            .padding(.top, 6)

            if updater.latest != nil || updater.notice != nil {
                UpdateBanner(updater: updater).transition(.move(edge: .top).combined(with: .opacity))
            }

            UsagePanel(usage: usage, accounts: accounts, now: store.now, forceCompact: medium) {
                store.groups.flatMap(\.sessions).filter { $0.state == "working" || $0.state == "waiting" }
            }

            if store.groups.isEmpty {
                VStack(spacing: 10) {
                    Image(systemName: "moon.zzz.fill").font(.system(size: 30)).foregroundStyle(.secondary)
                    Text("아직 기록된 세션이 없습니다").font(.system(size: 14, weight: .medium))
                    Text("Claude Code 세션에서 요청을 보내면 여기에 나타납니다")
                        .font(.system(size: 12)).foregroundStyle(.secondary)
                }
                // 창이 낮으면 안내가 줄어들며 잘린다 — 줄지 않으면 보드가 넘쳐 아래 투명도 슬라이더가 밀려난다
                .frame(maxWidth: .infinity, minHeight: 0, maxHeight: .infinity)
                .clipped()
                .background(listHeightReader)
                .transition(.opacity)
            } else {
                if let f = filter {
                    HStack(spacing: 6) {
                        Circle().fill(Palette.color(f)).frame(width: 7, height: 7)
                        Text("\(Palette.name(f))만 보는 중").font(.system(size: 12, weight: .semibold))
                        Spacer()
                        Button("모두 보기") { withAnimation(foldSpring) { filter = nil } }
                            .buttonStyle(.plain).font(.system(size: 12, weight: .semibold)).foregroundStyle(Palette.accent)
                    }
                    .padding(.horizontal, 10).padding(.vertical, 6)
                    .background(RoundedRectangle(cornerRadius: 8).fill(Palette.color(f).opacity(0.12)))
                    .transition(.move(edge: .top).combined(with: .opacity))
                }
                ScrollViewReader { proxy in
                ScrollView(showsIndicators: true) {
                    VStack(spacing: 12) {
                        ForEach(shownGroups) { g in
                            GroupCard(g: g, now: store.now,
                                      collapsed: medium ? !mediumOpen.contains(g.id) : store.collapsed.contains(g.id),
                                      toggle: { if medium { mediumOpen.formSymmetricDifference([g.id]) } else { store.toggle(g.id) } },
                                      endedOpen: store.endedOpen.contains(g.id), toggleEnded: { store.toggleEnded(g.id) },
                                      pinned: store.pinnedGroups.contains(g.id), togglePin: { store.togglePinGroup(g.id) },
                                      remove: { withAnimation(.spring(response: 0.4, dampingFraction: 0.85)) { store.hideGroup(g) } },
                                      clearEnded: { withAnimation(.spring(response: 0.4, dampingFraction: 0.85)) { store.removeEnded(g) } })
                                .transition(.asymmetric(insertion: .move(edge: .top).combined(with: .opacity),
                                                        removal: .opacity))
                                .id(g.id)
                                .background(GeometryReader { geo in
                                    Color.clear.preference(key: CardTops.self,
                                                           value: [g.id: geo.frame(in: .named("boardScroll")).minY])
                                })
                        }
                    }
                    .padding(.bottom, 12)
                }
                .coordinateSpace(name: "boardScroll")
                .background(listHeightReader)
                .onPreferenceChange(CardTops.self) { tops.values = $0; updateHidden() }
                .onChange(of: scrollRequest) { _, r in
                    proxy.scrollTo(r.id, anchor: .top)   // 걸러 보기 전환과 겹치지 않게 애니메이션 없이
                }
                .overlay(alignment: .bottom) {
                    let b = below
                    if b.n > 0 {
                        ZStack(alignment: .bottom) {
                            // 아래 끝을 흐리게 — 목록이 더 있다는 표시
                            LinearGradient(colors: [.clear, Color(red: 0.07, green: 0.07, blue: 0.11).opacity(0.9)],
                                           startPoint: .top, endPoint: .bottom)
                                .frame(height: 56)
                                .allowsHitTesting(false)
                            let tint = b.waiting ? Palette.color("waiting") : Color.white
                            HStack(spacing: 5) {
                                Image(systemName: "chevron.down").font(.system(size: 10, weight: .bold))
                                Text(b.waiting ? "아래에 경로 \(b.n)개 더 · 승인 대기 있음" : "아래에 경로 \(b.n)개 더")
                                    .font(.system(size: 11, weight: .semibold))
                            }
                            .foregroundStyle(b.waiting ? Color.black.opacity(0.85) : Color.white.opacity(0.9))
                            .padding(.horizontal, 10).padding(.vertical, 5)
                            .background(Capsule().fill(b.waiting ? tint.opacity(moreHover ? 1 : 0.9) : tint.opacity(moreHover ? 0.22 : 0.14)))
                            .overlay(Capsule().stroke(tint.opacity(0.35), lineWidth: 0.5))
                            .contentShape(Capsule())
                            .onTapGesture {
                                if let id = b.first {
                                    withAnimation(.spring(response: 0.5, dampingFraction: 0.86)) { proxy.scrollTo(id, anchor: .top) }
                                }
                            }
                            .clickable($moreHover)
                            .hint("눌러서 아래 카드로 이동")
                            .padding(.bottom, 8)
                        }
                        .transition(.opacity)
                    }
                }
                }
                .environment(\.notifyLocked, store.allMuted)
            }

            // 카카오톡 PC 처럼 창 오른쪽 아래에 투명도 슬라이더
            HStack {
                Spacer()
                OpacitySlider()
            }
            .padding(.top, -8)
            .padding(.bottom, 6)
        }
        .padding(.horizontal, 16)
        .padding(.top, 22)
        // 최소 높이는 주지 않는다 — 주면 제목 막대 높이만큼 넘쳐 아래 투명도 슬라이더가 잘린다. 낮으면 단계가 바뀐다
        .frame(minWidth: boardMinWidth)
        .background(GeometryReader { geo in
            Color.clear.onAppear { tops.board = geo.size.height; measure(tier) }
                .onChange(of: geo.size.height) { _, h in tops.board = h; measure(tier) }
        })
        .background(Backdrop())
        // 터미널 열기 독 — 보드 위에 겹쳐 뜨므로 아래 배치는 그대로다
        .overlay(alignment: .trailing) { EdgeDock() }
        .onChange(of: store.groups) { _, _ in
            if let f = filter, store.count(f) == 0 { withAnimation(foldSpring) { filter = nil } }
        }
        .preferredColorScheme(.dark)
        .animation(.spring(response: 0.45, dampingFraction: 0.82), value: store.groups)
    }
}

// MARK: - 업데이트

// GitHub 최신 릴리스의 VERSION 과 이 앱 버전을 비교해 새 버전이 있으면 보드 위에 알린다.
// 업데이트 단추는 설치와 같은 install.sh 를 curl 로 받아 실행한다 — 스크립트가 앱을 바꿔 끼우고 보드를 다시 띄운다
let releaseRepo = "suhojang/claude-board"   // packaging/install.sh·release.sh 의 REPO 와 같게
let releaseBase = "https://github.com/\(releaseRepo)/releases/latest/download"
let changelogURL = URL(string: "https://github.com/\(releaseRepo)/blob/main/CHANGELOG.md")!

final class Updater: ObservableObject {
    static let shared = Updater()
    @Published var latest: String?   // 이 앱보다 새 버전이 나왔으면 그 버전
    @Published var running = false
    @Published var failed = false
    @Published var checking = false   // 손으로 누른 확인이 도는 중
    @Published var notice: (icon: String, text: String)?   // 손으로 확인한 결과 — 새 버전이 없을 때 알림 줄에 잠깐 뜬다
    let current = Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""   // 제목 옆 버전 표시도 쓴다
    private let logPath = NSHomeDirectory() + "/.claude/notifier/update.log"
    private var timer: Timer?
    private var noticeHide: DispatchWorkItem?
    // build-board.sh 로 만든 개발용 빌드(1.0)는 배포판으로 덮어쓰지 않게 확인하지 않는다
    private var eligible: Bool { !current.isEmpty && current != "1.0" }

    private init() {
        guard eligible else { return }
        check()
        timer = Timer.scheduledTimer(withTimeInterval: 3600, repeats: true) { [weak self] _ in self?.check() }
    }

    // manual — 업데이트 확인 단추·메뉴에서 불렀을 때. 새 버전이 없거나 확인하지 못해도 결과를 알림 줄에 보인다
    func check(manual: Bool = false) {
        if manual {
            guard !checking else { return }
            guard eligible else { show("hammer.fill", "개발용 빌드(\(current))는 업데이트를 확인하지 않습니다"); return }
            checking = true
        }
        let req = URLRequest(url: URL(string: releaseBase + "/VERSION")!, cachePolicy: .reloadIgnoringLocalCacheData,
                             timeoutInterval: 15)
        URLSession.shared.dataTask(with: req) { [weak self] data, resp, _ in
            guard let self else { return }
            var v: String?
            if (resp as? HTTPURLResponse)?.statusCode == 200, let data,
               let s = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
               s.range(of: "^[0-9]+(\\.[0-9]+)*$", options: .regularExpression) != nil { v = s }
            DispatchQueue.main.async {
                if let v {
                    let newer = v.compare(self.current, options: .numeric) == .orderedDescending
                    withAnimation(.easeOut(duration: 0.2)) { self.latest = newer ? v : nil }
                }
                guard manual else { return }
                self.checking = false
                if v == nil { self.show("exclamationmark.triangle.fill", "업데이트를 확인하지 못했습니다") }
                else if self.latest == nil { self.show("checkmark.circle.fill", "최신 버전입니다 (\(self.current))") }
            }
        }.resume()
    }

    private func show(_ icon: String, _ text: String) {
        withAnimation(.easeOut(duration: 0.2)) { notice = (icon, text) }
        noticeHide?.cancel()
        let hide = DispatchWorkItem { [weak self] in withAnimation(.easeOut(duration: 0.2)) { self?.notice = nil } }
        noticeHide = hide
        DispatchQueue.main.asyncAfter(deadline: .now() + 4, execute: hide)
    }

    func run() {
        guard !running else { return }
        running = true
        failed = false
        FileManager.default.createFile(atPath: logPath, contents: nil)
        let out = FileHandle(forWritingAtPath: logPath) ?? .nullDevice
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/bash")
        // pipefail — 받기에 실패하면 빈 스크립트가 성공으로 끝나지 않게 한다.
        // 출력은 파일로 받는다 — 스크립트가 도중에 이 앱을 끄므로 파이프로 받으면 쓰다가 끊겨 스크립트도 멈춘다
        p.arguments = ["-c", "set -o pipefail; /usr/bin/curl -fsSL '\(releaseBase)/install.sh' | /bin/bash"]
        p.standardInput = FileHandle.nullDevice
        p.standardOutput = out
        p.standardError = out
        // 성공하면 스크립트가 이 앱을 끄고 새 앱을 띄우므로 여기로 돌아오지 않는다 — 돌아오면 끄기 전에 멈춘 것이다
        p.terminationHandler = { [weak self] p in
            DispatchQueue.main.async {
                self?.running = false
                self?.failed = p.terminationStatus != 0
            }
        }
        do { try p.run() } catch { running = false; failed = true }
    }
}

// 새 버전 알림 줄 — 전체·중간 화면의 윗줄 아래에 뜬다
struct UpdateBanner: View {
    @ObservedObject var updater: Updater
    @State private var hover = false
    @State private var notesHover = false

    var body: some View {
        let tint = updater.failed ? Palette.color("waiting") : Palette.accent
        // 새 버전이 없으면 손으로 확인한 결과(최신·확인 실패)만 단추 없이 보인다
        let notice = updater.latest == nil ? updater.notice : nil
        HStack(spacing: 6) {
            Image(systemName: notice?.icon ?? (updater.failed ? "exclamationmark.triangle.fill" : "arrow.down.circle.fill"))
                .font(.system(size: 12)).foregroundStyle(tint)
            Text(notice?.text ?? (updater.failed ? "업데이트하지 못했습니다 · ~/.claude/notifier/update.log"
                 : updater.running ? "업데이트 중… 끝나면 보드가 다시 열립니다"
                 : "새 버전 \(updater.latest ?? "") 이 나왔습니다"))
                .font(.system(size: 12, weight: .semibold)).lineLimit(1).truncationMode(.middle)
            Spacer(minLength: 6)
            if updater.running {
                ProgressView().controlSize(.small)
            } else if notice == nil {
                // 무엇이 바뀌었는지 — 브라우저에서 GitHub 의 CHANGELOG.md 를 연다
                Button("변경 내용") { NSWorkspace.shared.open(changelogURL) }
                    .buttonStyle(.plain).font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(notesHover ? .primary : .secondary)
                    .clickable($notesHover)
                    .hint("GitHub 에서 CHANGELOG.md 를 엽니다")
                Rectangle().fill(Color.white.opacity(0.14)).frame(width: 0.5, height: 10).padding(.horizontal, 2)
                Button(updater.failed ? "다시 시도" : "업데이트") { updater.run() }
                    .buttonStyle(.plain).font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(tint.opacity(hover ? 1 : 0.85))
                    .clickable($hover)
                    .hint("새 버전을 받아 바꿔 끼웁니다 — 끝나면 보드가 다시 열립니다")
            }
        }
        .padding(.horizontal, 10).padding(.vertical, 6)
        .background(RoundedRectangle(cornerRadius: 8).fill(tint.opacity(0.12)))
    }
}

// MARK: - 내장 터미널

// 보드가 직접 쥐는 터미널(pty). 펼치면 창이 화면의 쓸 수 있는 영역 전체로 넓어져 [터미널 | 경계 | 보드], 접으면 지금처럼 보드만 남는다.
// 터미널 뷰는 창에서 빼지 않는다 — 창에 붙은 터미널이 하나도 없으면 엔진이 메일함 처리를 멈춰, 제목 변경이 쌓이면 pty 읽기가 막히고
// 세션 출력이 멈춘다(실측 19초). 숨길 때는 실제 크기 그대로 isHidden + setSurfaceVisible(false)(isHidden 만 하면 계속 그린다).
// 탭을 닫을 때는 controller 를 떼어 엔진 surface 를 내린다(그래야 셸이 끝난다)

let paneDividerWidth: CGFloat = 8       // 터미널과 보드 사이 경계 — 끌어서 두 영역 폭을 바꾼다
let terminalMinWidth: CGFloat = 480     // 펼쳤을 때 터미널 영역의 최소 폭
let terminalTabBarHeight: CGFloat = 40
let terminalAreaColor = NSColor(red: 0.071, green: 0.075, blue: 0.094, alpha: 1)   // 탭이 없을 때 터미널 영역 바탕
let terminalBarColor = NSColor(red: 0.10, green: 0.10, blue: 0.13, alpha: 1)        // 탭 줄과 그 위 제목 막대 띠

// 한글 조합 입력 고침 — 입력기는 조합 중 Return·Esc 에 "글자 확정" 과 "줄바꿈·취소 명령" 을 함께 보내는데 래퍼는 확정 글자가 있으면
// 명령을 버려 Return 을 두 번 눌러야 한다(Ghostty.app 1.3.1 도 같다). 입력기가 실제로 그 명령을 보냈을 때만 원래 키를 수정키 그대로
// 다시 보낸다 — 일본어 변환 확정처럼 명령이 없으면 그대로 둔다. 엔진 버전을 올리면 래퍼가 고쳤는지 다시 시험한다(고쳤으면 Enter 가 두 번 간다)
final class BoardTerminalView: AppTerminalView {
    private var composingAtKeyDown = false
    private var insertedDuringKey = false
    private var commandDuringKey: Selector?

    private func mods(_ event: NSEvent) -> TerminalInputModifiers {
        var m: TerminalInputModifiers = []
        let f = event.modifierFlags
        if f.contains(.shift) { m.insert(.shift) }
        if f.contains(.control) { m.insert(.ctrl) }
        if f.contains(.option) { m.insert(.alt) }
        if f.contains(.command) { m.insert(.super_) }
        return m
    }

    override func keyDown(with event: NSEvent) {
        // 수정키가 붙은 Return(Shift+Enter 등): 래퍼는 확정 글자를 그 키 이벤트에 실어 보내 키 지정이 글자를 덮는다 →
        // sendKey 로 조합 글자를 먼저(수정키 없는 이벤트로) 확정하고 키를 따로 보낸다
        if hasMarkedText(), event.keyCode == 36 || event.keyCode == 76, !mods(event).isEmpty {
            sendKey(.enter, modifiers: mods(event))
            return
        }
        composingAtKeyDown = hasMarkedText()
        insertedDuringKey = false
        commandDuringKey = nil
        super.keyDown(with: event)
        defer { composingAtKeyDown = false; commandDuringKey = nil }
        guard composingAtKeyDown, insertedDuringKey, let sel = commandDuringKey else { return }
        let key: TerminalKey
        switch sel {
        case #selector(NSResponder.insertNewline(_:)), #selector(NSResponder.insertLineBreak(_:)),
             #selector(NSResponder.insertNewlineIgnoringFieldEditor(_:)): key = .enter
        case #selector(NSResponder.cancelOperation(_:)): key = .escape
        default: return
        }
        sendKey(key, modifiers: mods(event))
    }

    override func insertText(_ string: Any, replacementRange: NSRange) {
        if composingAtKeyDown { insertedDuringKey = true }
        super.insertText(string, replacementRange: replacementRange)
    }

    override func doCommand(by selector: Selector) {
        if composingAtKeyDown { commandDuringKey = selector }
        super.doCommand(by: selector)
    }
}

// 로그인 + 대화형 셸의 환경 — 보드가 셸 없이 바로 띄우는 명령(claude)에 넘긴다. Finder·Dock 으로 띄운 보드의 PATH 는 짧아
// 사용자 hook 의 node 등을 못 찾고, 로그인 셸만으로는 대화형 설정에서 붙는 ~/.local/bin 이 빠진다.
// VS Code 의 셸 환경 읽기와 같은 방식으로 시작할 때 한 번, 시간 제한(10초)을 두고 읽는다
enum LoginEnvironment {
    private(set) static var values: [String: String]?
    // 터미널이 스스로 정하는 값과 그 셸에만 의미 있는 값은 넘기지 않는다
    private static func dropped(_ key: String) -> Bool {
        ["PWD", "OLDPWD", "SHLVL", "_", "COLORTERM"].contains(key) || key.hasPrefix("TERM") || key.hasPrefix("GHOSTTY_")
    }

    static func load() {
        DispatchQueue.global(qos: .utility).async {
            let shell = getpwuid(getuid()).map { String(cString: $0.pointee.pw_shell) } ?? "/bin/zsh"
            let mark = "__CB_ENV_\(UUID().uuidString)__"
            let p = Process()
            p.executableURL = URL(fileURLWithPath: shell)
            p.arguments = ["-i", "-l", "-c", "printf '%s' '\(mark)'; /usr/bin/env -0; printf '%s' '\(mark)'"]
            p.standardInput = FileHandle.nullDevice
            p.standardError = FileHandle.nullDevice
            p.environment = ProcessInfo.processInfo.environment.filter { !inheritedSessionVariable($0.key) }
            let pipe = Pipe()
            p.standardOutput = pipe
            guard (try? p.run()) != nil else { return }
            // 셸이 띄운 데몬(gitstatusd 등)이 출력 파이프를 물고 남을 수 있어 끝(EOF)을 기다리지 않고 두 번째 표지까지만 읽는다
            let deadline = Date().addingTimeInterval(10)
            var data = Data()
            let end = Data(mark.utf8)
            let reader = pipe.fileHandleForReading
            while Date() < deadline {
                let chunk = reader.availableData
                if chunk.isEmpty { break }
                data.append(chunk)
                if let first = data.range(of: end), data.range(of: end, in: first.upperBound ..< data.endIndex) != nil { break }
            }
            if p.isRunning { p.terminate() }
            guard let first = data.range(of: end), let second = data.range(of: end, in: first.upperBound ..< data.endIndex) else { return }
            var env: [String: String] = [:]
            for entry in data[first.upperBound ..< second.lowerBound].split(separator: 0) {
                guard let s = String(data: Data(entry), encoding: .utf8), let eq = s.firstIndex(of: "=") else { continue }
                let key = String(s[..<eq])
                if !dropped(key) { env[key] = String(s[s.index(after: eq)...]) }
            }
            guard env["PATH"] != nil else { return }
            DispatchQueue.main.async {
                values = env
                TerminalHub.shared.envReady = true
            }
        }
    }
}

// 터미널 설정은 보드가 직접 만든다 — 사용자 Ghostty 설정을 바탕으로 하되 theme 줄은 뺀다(엔진 번들에 테마 폴더가 없어 theme 한 줄 때문에
// 파일 전체가 조용히 거부된다). 같은 이름 테마는 엔진의 테마 목록에서 찾아 넘긴다(넘기지 않으면 래퍼 기본 테마가 사용자 색을 덮는다)
@MainActor func makeTerminalController() -> TerminalController {
    let home = NSHomeDirectory()
    // Ghostty 가 읽는 순서 — 뒤의 파일이 앞을 덮는다
    let files = ["\(home)/.config/ghostty/config", "\(home)/.config/ghostty/config.ghostty",
                 "\(home)/Library/Application Support/com.mitchellh.ghostty/config",
                 "\(home)/Library/Application Support/com.mitchellh.ghostty/config.ghostty"]
    var lines: [String] = []
    var theme: String?
    for f in files {
        guard let text = try? String(contentsOfFile: f, encoding: .utf8) else { continue }
        for line in text.components(separatedBy: .newlines) {
            let parts = line.split(separator: "=", maxSplits: 1).map { $0.trimmingCharacters(in: .whitespaces) }
            if parts.count == 2, parts[0] == "theme" {
                theme = parts[1].trimmingCharacters(in: CharacterSet(charactersIn: "\""))
                continue
            }
            // Ghostty.app 의 전역 단축키(빠른 터미널 등)는 보드 터미널과 상관없다
            if parts.count == 2, parts[0] == "keybind", parts[1].hasPrefix("global:") { continue }
            lines.append(line)
        }
    }
    lines.append("keybind = shift+enter=text:\\x1b\\r")   // Shift+Enter = 사용자 Ghostty 와 같은 바이트(1b 0d) — 기본값은 CSI 27;2;13~
    lines.append("keybind = super+q=unbind")              // ⌘Q 는 보드 종료로 — 엔진이 받으면 처리할 곳이 없어 삼킨다
    let dir = NSTemporaryDirectory() + (Bundle.main.bundleIdentifier ?? instanceName)
    try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
    let path = dir + "/board-terminal.conf"
    try? lines.joined(separator: "\n").write(toFile: path, atomically: true, encoding: .utf8)
    let c = TerminalController(configFilePath: path, theme: terminalTheme(theme))
    // 한 줄이라도 거부되면 파일 전체가 조용히 기본값으로 돌아간다 — 원인을 찾을 수 있게 남긴다
    if let issue = c.lastConfigurationIssue { boardLog("터미널 설정 거부됨: \(issue)") }
    return c
}

// ~/.claude/notifier/board.log 에 한 줄 남긴다 — 보드는 그대로 동작해야 하는 문제(단축키 등록 실패 등)를 적는다
func boardLog(_ message: String) {
    if let h = FileHandle(forWritingAtPath: NSHomeDirectory() + "/.claude/notifier/board.log") {
        h.seekToEndOfFile(); h.write("\(Date()) \(message)\n".data(using: .utf8)!); h.closeFile()
    }
}

// theme 값: "이름" 또는 "light:이름,dark:이름". 목록에 없는 이름이면 래퍼 기본(보드 창은 어두우므로 Afterglow)
func terminalTheme(_ spec: String?) -> TerminalTheme {
    guard let spec else { return .default }
    var light = spec, dark = spec
    for part in spec.split(separator: ",") {
        let kv = part.split(separator: ":", maxSplits: 1).map { $0.trimmingCharacters(in: .whitespaces) }
        guard kv.count == 2 else { continue }
        if kv[0] == "light" { light = kv[1] } else if kv[0] == "dark" { dark = kv[1] }
    }
    guard let d = GhosttyThemeCatalog.theme(named: dark) ?? GhosttyThemeCatalog.theme(named: light) else { return .default }
    let l = GhosttyThemeCatalog.theme(named: light) ?? d
    return TerminalTheme(light: l.toTerminalConfiguration(), dark: d.toTerminalConfiguration())
}

// 보드 터미널 탭 하나
final class BoardTab: NSObject, ObservableObject, Identifiable, TerminalSurfaceTitleDelegate, TerminalSurfaceCloseDelegate,
                      TerminalSurfacePwdDelegate, TerminalSurfaceClipboardConfirmationDelegate, TerminalSurfaceLifecycleDelegate {
    let id = UUID()
    let view: BoardTerminalView
    let command: String?            // 셸 없이 띄운 명령(claude …) — nil 이면 셸 탭
    let sessionId: String?          // 보드가 띄울 때 아는 세션 id(새 세션의 --session-id, 이어 연 세션)
    @Published var title = ""
    @Published var cwd: String
    @Published var session: Session?   // 이 탭에서 도는 세션(세션 파일의 tty 로 짝짓는다) — 탭의 상태 점·제목
    @Published var exited = false      // 명령이 끝났다 — 엔진의 "Process exited" 화면 대신 [다시 열기][셸 열기] 막대
    private var exitWatch: DispatchSourceProcess?

    init(_ options: TerminalSurfaceOptions, frame: NSRect, controller: TerminalController, command: String? = nil, sessionId: String? = nil) {
        cwd = options.workingDirectory ?? NSHomeDirectory()
        self.command = command
        self.sessionId = sessionId
        view = BoardTerminalView(frame: frame)
        super.init()
        view.autoresizingMask = [.width, .height]
        view.delegate = self   // 약한 참조 — 허브가 탭을 쥔다
        view.controller = controller
        view.configuration = options
    }

    // 탭 이름 — 작업 경로의 폴더 이름
    var name: String { cwd == NSHomeDirectory() ? "~" : (cwd as NSString).lastPathComponent }
    // 세션 파일의 tty 와 같은 꼴(/dev/ 를 뗀 이름)
    var tty: String { (view.ttyName ?? "").replacingOccurrences(of: "/dev/", with: "") }

    func terminalDidChangeTitle(_ title: String) { self.title = title }
    func terminalDidChangeWorkingDirectory(_ path: String) { cwd = path }
    func terminalDidDetachSurface() {}

    // 명령 탭: 엔진이 띄운 login 은 명령이 끝난 뒤에야 끝나므로 그 pid 를 지켜본다(엔진은 끝나도 닫힘 콜백을 주지 않는다)
    func terminalDidAttachSurface(_ surface: TerminalSurface) {
        guard command != nil else { return }
        watchExit(tries: 10)
    }

    private func watchExit(tries: Int) {
        guard exitWatch == nil, !exited else { return }
        guard let fg = view.foregroundPid, fg > 0 else {
            if tries > 0 { DispatchQueue.main.asyncAfter(deadline: .now() + 0.2) { self.watchExit(tries: tries - 1) } }
            return
        }
        let leader = getsid(fg)
        guard leader > 0 else { return }
        let src = DispatchSource.makeProcessSource(identifier: leader, eventMask: .exit, queue: .main)
        src.setEventHandler { [weak self] in self?.markExited() }
        src.resume()
        exitWatch = src
        if kill(leader, 0) != 0 { markExited() }   // 이미 끝난 pid 에 건 감시는 울리지 않는다
    }

    private func markExited() {
        exitWatch?.cancel()
        exitWatch = nil
        exited = true
        TerminalHub.shared.applyVisibility()
    }
    // 셸이 끝났거나(processAlive false) ⌘W 처럼 엔진이 닫기를 청했을 때
    func terminalDidClose(processAlive: Bool) { TerminalHub.shared.close(self, confirm: processAlive) }

    // 붙여넣기 보호(줄바꿈이 든 내용을 bracketed paste 가 꺼진 프로그램에)·프로그램의 클립보드 읽기(OSC 52)를 묻는다 —
    // 처리기가 없으면 붙여넣기는 알림 없이 버려진다
    func terminalDidRequestClipboardConfirmation(_ request: TerminalClipboardConfirmationRequest) {
        let a = NSAlert()
        switch request.kind {
        case .paste:
            a.messageText = "여러 줄을 붙여넣을까요?"
            a.informativeText = "줄바꿈이 들어 있어 터미널의 프로그램이 줄마다 바로 실행할 수 있습니다.\n\n" + String(request.contents.prefix(400))
            a.addButton(withTitle: "붙여넣기")
        case .osc52Read:
            a.messageText = "터미널의 프로그램이 클립보드를 읽으려고 합니다"
            a.informativeText = "허용하면 지금 클립보드 내용이 그 프로그램에 전달됩니다."
            a.addButton(withTitle: "허용")
        case .osc52Write:
            a.messageText = "터미널의 프로그램이 클립보드에 쓰려고 합니다"
            a.informativeText = String(request.contents.prefix(400))
            a.addButton(withTitle: "허용")
        }
        a.addButton(withTitle: "취소").keyEquivalent = "\u{1b}"   // NSAlert 는 영어 "Cancel" 에만 Esc 를 붙인다
        a.beginSheetModal(for: window) { request.respond(allow: $0 == .alertFirstButtonReturn) }
    }
}

@MainActor final class TerminalHub: ObservableObject {
    static let shared = TerminalHub()
    @Published private(set) var tabs: [BoardTab] = []
    @Published private(set) var selected: UUID?
    // 펼침 — 기본값은 펼침
    @Published private(set) var expanded = UserDefaults.standard.object(forKey: "terminalExpanded") as? Bool ?? true
    // 펼쳤을 때 보드 폭 — 경계를 끌어 바꾼 폭을 기억한다
    var boardWidth = CGFloat(UserDefaults.standard.object(forKey: "boardPaneWidth") as? Double ?? Double(boardLaunchWidth))
    weak var area: TerminalAreaView?
    weak var store: Store?
    // 세션 id → 그 세션이 도는 보드 탭(tty 로 짝지은 세션, 탭 안에서 시작된 claude -p) — 세션 목록을 다시 읽을 때마다 고친다
    @Published private(set) var paired: [String: UUID] = [:]
    @Published private(set) var tabOwnedHeadless: Set<String> = []
    // 다른 터미널 세션의 앱 이름(칩) — 세션마다 한 번 조상 프로세스로 찾는다(AppleScript 없이)
    @Published private(set) var appNames: [String: String] = [:]
    private var modes: [Int32: String] = [:]   // 백그라운드 세션 pid → 실행 방식(-p, --bg …)
    @Published var envReady = false   // 로그인 셸 환경을 읽었다 — 새 세션 창의 시작 단추
    // 보드 아래쪽에 잠깐 뜨는 안내 — 백그라운드 세션을 눌렀을 때 등
    @Published private(set) var notice: String?
    private var noticeHide: DispatchWorkItem?
    // 고른 탭·터미널 영역 바탕 — 엔진을 띄운 뒤에는 터미널 테마의 바탕색(탭이 터미널과 이어져 보이게)
    @Published private(set) var background = terminalAreaColor
    private var engine: TerminalController?

    // 엔진은 첫 탭을 열 때 띄운다
    var controller: TerminalController {
        if let engine { return engine }
        let c = makeTerminalController()
        engine = c
        let bg = c.backgroundColor
        background = NSColor(red: CGFloat(bg.red) / 255, green: CGFloat(bg.green) / 255, blue: CGFloat(bg.blue) / 255, alpha: 1)
        area?.container.layer?.backgroundColor = background.cgColor
        return c
    }

    func setExpanded(_ on: Bool) {
        expanded = on
        UserDefaults.standard.set(on, forKey: "terminalExpanded")
    }

    // at 을 주면 그 자리의 탭을 바꿔 끼운다([다시 열기][셸 열기])
    @discardableResult
    func open(_ options: TerminalSurfaceOptions, command: String? = nil, sessionId: String? = nil, at index: Int? = nil) -> BoardTab? {
        guard let area else { return nil }
        area.superview?.layoutSubtreeIfNeeded()   // 크기가 0 인 뷰는 셸을 띄우지 않는다
        let tab = BoardTab(options, frame: area.container.bounds, controller: controller, command: command, sessionId: sessionId)
        tab.view.isHidden = true
        area.container.addSubview(tab.view)
        if let index, index <= tabs.count { tabs.insert(tab, at: index) } else { tabs.append(tab) }
        select(tab.id)
        return tab
    }

    // 셸 탭 — login 이 환경을 스스로 꾸리므로 환경 변수를 넘기지 않는다
    @discardableResult
    func openShell(in dir: String, at index: Int? = nil) -> BoardTab? {
        open(TerminalSurfaceOptions(backend: .exec, workingDirectory: dir), at: index)
    }

    // 명령(claude …)을 셸 없이 보드 탭에서 띄운다. 엔진은 명령을 `login … bash -c "exec -l <명령>"` 으로 띄우는데 exec -l 이
    // 첫 인자 앞에 - 를 붙여(ps 의 comm 이 -claude) hook 이 claude 를 못 알아본다 — /usr/bin/env 를 앞에 두면 - 는 env 에 붙고
    // claude 는 제 이름으로 뜬다. 환경은 시작할 때 읽어 둔 로그인+대화형 셸 환경(PATH 등) — 못 읽었으면 띄우지 않는다
    @discardableResult
    func launch(_ command: String, in dir: String, sessionId: String?, at index: Int? = nil) -> BoardTab? {
        guard let env = LoginEnvironment.values else {
            toast("셸 환경을 아직 읽지 못했습니다 — 잠시 뒤 다시 시도해 주세요")
            return nil
        }
        let opts = TerminalSurfaceOptions(backend: .exec, workingDirectory: dir, envVars: env, command: "/usr/bin/env " + command)
        guard let tab = open(opts, command: command, sessionId: sessionId, at: index) else { return nil }
        reveal(tab.id)
        return tab
    }

    // 새 세션 — claude 면 보드가 세션 id 를 붙여 미리 안다
    func startSession(_ command: String, in dir: String) {
        let (cmd, id) = freshSessionCommand(command)
        launch(cmd, in: dir, sessionId: id)
    }

    // "+ 새 세션" 창 — 작업 폴더는 보드에 있는 경로(지금 탭의 경로를 먼저 고른다) + 다른 폴더
    func newSessionSheet() {
        if !expanded { setTerminalExpanded(true) }
        showBoard()
        let paths = (store?.groups.map(\.id) ?? []).filter { FileManager.default.fileExists(atPath: $0) }
        let current = tabs.first { $0.id == selected }.map { $0.session?.cwd ?? $0.cwd }
        let sheet = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 460, height: 420), styleMask: [.titled], backing: .buffered, defer: false)
        let end = { window.endSheet(sheet) }
        sheet.contentViewController = NSHostingController(rootView: NewSessionSheet(
            paths: paths, initial: current.flatMap { paths.contains($0) ? $0 : nil } ?? paths.first ?? NSHomeDirectory(),
            cancel: end, start: { cmd, dir in end(); self.startSession(cmd, in: dir) }))
        window.beginSheet(sheet)
    }

    // 탭을 보이게 한다 — 접혀 있거나 미니면 펼치고, 그 탭을 고르고, 보드를 앞으로 꺼내 터미널에 포커스
    func reveal(_ id: UUID) {
        if !expanded { setTerminalExpanded(true) }
        select(id)
        showBoard()
        focusSelected()
    }

    // 명령이 끝난 탭의 [다시 열기] — 대화가 남은 세션이면 그 resume_cmd 로, 아니면 처음 명령으로 같은 자리에서
    func relaunch(_ tab: BoardTab) {
        guard let i = tabs.firstIndex(where: { $0 === tab }), let command = tab.command else { return }
        let s = tab.sessionId.flatMap { id in store?.groups.flatMap(\.sessions).first { $0.id == id } }
        let next: (String, String?)
        if let s { next = (s.resumeCmd.isEmpty ? "claude --resume \(s.id)" : s.resumeCmd, s.id) } else { next = freshSessionCommand(command) }
        guard launch(next.0, in: s?.cwd ?? tab.cwd, sessionId: next.1, at: i) != nil else { return }
        remove(tab)
    }

    // 명령이 끝난 탭의 [셸 열기] — 그 작업 경로에서 셸
    func openShellHere(_ tab: BoardTab) {
        guard let i = tabs.firstIndex(where: { $0 === tab }) else { return }
        guard openShell(in: tab.session?.cwd ?? tab.cwd, at: i) != nil else { return }
        remove(tab)
        focusSelected()
    }

    func toast(_ text: String) {
        withAnimation(.easeOut(duration: 0.2)) { notice = text }
        noticeHide?.cancel()
        let w = DispatchWorkItem { [weak self] in withAnimation(.easeOut(duration: 0.3)) { self?.notice = nil } }
        noticeHide = w
        DispatchQueue.main.asyncAfter(deadline: .now() + 6, execute: w)
    }

    func dismissNotice() {
        noticeHide?.cancel()
        withAnimation(.easeOut(duration: 0.2)) { notice = nil }
    }

    // 세션이 어디서 도는가 — 보드 탭(그 안에서 시작된 claude -p 포함)·다른 터미널·백그라운드·끝남
    func place(_ s: Session) -> SessionPlace {
        if let id = paired[s.id] ?? tabs.first(where: { $0.sessionId == s.id })?.id {
            return .tab(id, headless: tabOwnedHeadless.contains(s.id))
        }
        if s.state == "ended" { return .ended }
        return s.headless ? .background : .terminal
    }

    // 세션 목록을 다시 읽을 때마다 탭과 짝짓는다. tty 가 같으면 그 탭의 세션, headless 세션은 조상에 탭의 claude 가 있으면
    // 그 탭 소속(탭이 닫히면 함께 끝난다 — Claude 가 Bash 도구로 띄운 claude -p 등). 끝난 세션은 같은 세션 id 로 연 탭만
    func pair(_ sessions: [Session]) {
        var byTty: [String: BoardTab] = [:]
        for t in tabs where !t.tty.isEmpty { byTty[t.tty] = t }
        var map: [String: UUID] = [:]
        // 탭의 맨 위 프로세스(엔진이 띄운 login — 그 tty 의 세션 리더). 셸 탭에서 사람이 띄운 claude -p 도 그 아래에 있다
        var tabPids: [Int32: BoardTab] = [:]
        for t in tabs { if let fg = t.view.foregroundPid, fg > 0, getsid(fg) > 0 { tabPids[getsid(fg)] = t } }
        var main: [UUID: Session] = [:]
        for s in sessions where s.state != "ended" && !s.tty.isEmpty {
            guard let t = byTty[s.tty] else { continue }
            map[s.id] = t.id
            main[t.id] = s
            if let want = t.sessionId, want != s.id { boardLog("탭 세션 id 가 다름: 띄운 \(want) · 기록 \(s.id)") }
        }
        var owned: Set<String> = []
        for s in sessions where s.state != "ended" && s.headless && s.pid > 0 {
            if let t = tabAncestor(s.pid, tabPids) { map[s.id] = t.id; owned.insert(s.id) }
        }
        for t in tabs {
            let s = main[t.id] ?? t.sessionId.flatMap { id in sessions.first { $0.id == id } }
            if t.session != s { t.session = s }
        }
        if map != paired { paired = map }
        if owned != tabOwnedHeadless { tabOwnedHeadless = owned }
        // 다른 터미널 세션의 앱 이름 — 조상 프로세스로만(AppleScript 를 쓰는 ownerApp 은 부르지 않는다)
        var names = appNames
        for s in sessions where s.state != "ended" && !s.headless && map[s.id] == nil && names[s.id] == nil && s.pid > 0 {
            if let id = ancestorApp(s.pid) {
                names[s.id] = terminalApps.first { $0.id == id }?.name
                    ?? NSRunningApplication.runningApplications(withBundleIdentifier: id).first?.localizedName ?? "터미널"
            }
        }
        if names != appNames { appNames = names }
    }

    // 세션의 실행 방식 — 프로세스 인자에서(-p·--print → -p, --bg·--background → --bg, 대화형이면 nil). 끝난 세션은 읽을 수 없다
    func mode(_ s: Session) -> String? {
        if let m = modes[s.pid] { return m.isEmpty ? nil : m }
        let args = processArgs(s.pid)
        let m = args.contains { $0 == "--bg" || $0 == "--background" } ? "--bg"
            : args.contains { $0 == "-p" || $0 == "--print" } ? "-p" : ""
        if s.pid > 0, !args.isEmpty { modes[s.pid] = m }
        return m.isEmpty ? nil : m
    }

    func select(_ id: UUID) {
        selected = id
        applyVisibility()
        focusSelected()
    }

    // 고른 탭만 그리고 나머지는 실제 크기 그대로 숨긴다. 접혀 있거나 창이 안 보이면(가려짐·최소화·닫힘·다른 데스크톱) 모두 그리지 않는다
    func applyVisibility() {
        let drawing = expanded && window.occlusionState.contains(.visible)
        for t in tabs {
            t.view.isHidden = t.id != selected
            t.view.setSurfaceVisible(t.id == selected && drawing)
        }
        area?.empty.isHidden = !tabs.isEmpty
        area?.exitBar.isHidden = !(tabs.first { $0.id == selected }?.exited ?? false)
    }

    func focusSelected() {
        guard expanded, let t = tabs.first(where: { $0.id == selected }) else { return }
        window.makeFirstResponder(t.view)
    }

    // 탭의 × — ⌘W 와 같은 엔진 동작으로 보내 셸 프롬프트처럼 물을 필요가 없으면 바로, 프로그램이 돌고 있으면 물어서 닫는다
    func requestClose(_ tab: BoardTab) {
        if !tab.view.performBindingAction("close_surface") { close(tab, confirm: false) }
    }

    // 탭 닫기 — 프로그램이 아직 돌고 있으면 먼저 묻는다
    func close(_ tab: BoardTab, confirm: Bool) {
        guard tabs.contains(where: { $0 === tab }) else { return }
        guard confirm else { remove(tab); return }
        let a = NSAlert()
        a.messageText = "탭을 닫을까요?"
        a.informativeText = "\(tab.name) 탭에서 도는 프로그램이 함께 끝납니다."
        a.addButton(withTitle: "닫기")
        a.addButton(withTitle: "취소").keyEquivalent = "\u{1b}"
        a.beginSheetModal(for: window) { if $0 == .alertFirstButtonReturn { self.remove(tab) } }
    }

    private func remove(_ tab: BoardTab) {
        guard let i = tabs.firstIndex(where: { $0 === tab }) else { return }
        tabs.remove(at: i)
        // 엔진 surface 를 내려 셸·프로그램을 끝낸다 — 창에서 빼는 것만으로는 뷰가 바로 풀리지 않아 셸이 남는다(실측)
        tab.view.controller = nil
        tab.view.removeFromSuperview()
        if selected == tab.id { selected = tabs.isEmpty ? nil : tabs[min(i, tabs.count - 1)].id }
        applyVisibility()
        focusSelected()
    }
}

// 세션이 도는 곳 — 칩·클릭 흐름·카드 구역이 쓴다
enum SessionPlace: Equatable {
    case tab(UUID, headless: Bool)   // 보드 탭(headless = 그 탭 안에서 시작된 claude -p)
    case terminal                    // 다른 터미널 앱
    case background                  // claude -p·--bg 등 터미널 없이 도는 세션
    case ended
}

// pid 의 조상 가운데 탭의 맨 위 프로세스가 있으면 그 탭
func tabAncestor(_ pid: Int32, _ tabPids: [Int32: BoardTab]) -> BoardTab? {
    var p = pid
    for _ in 0..<12 where p > 1 {
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, p]
        guard sysctl(&mib, 4, &info, &size, nil, 0) == 0, size > 0 else { return nil }
        p = info.kp_eproc.e_ppid
        if let t = tabPids[p] { return t }
    }
    return nil
}

// 프로세스 실행 인자(sysctl KERN_PROCARGS2 — state.py proc_args 와 같은 방식). 읽지 못하면 빈 배열
func processArgs(_ pid: Int32) -> [String] {
    guard pid > 0 else { return [] }
    var mib: [Int32] = [CTL_KERN, KERN_PROCARGS2, pid]
    var size = 1 << 18
    var buf = [UInt8](repeating: 0, count: size)
    guard sysctl(&mib, 3, &buf, &size, nil, 0) == 0, size > 4 else { return [] }
    let argc = Int(buf.withUnsafeBytes { $0.load(as: Int32.self) })
    var i = 4
    while i < size, buf[i] != 0 { i += 1 }   // 실행 파일 경로를 건너뛴다
    while i < size, buf[i] == 0 { i += 1 }
    var args: [String] = []
    while i < size, args.count < argc {
        var j = i
        while j < size, buf[j] != 0 { j += 1 }
        args.append(String(decoding: buf[i..<j], as: UTF8.self))
        i = j + 1
    }
    return args
}

// 새로 띄우는 claude 에 세션 id 를 붙인다(--session-id) — 보드가 그 세션을 미리 안다. claude 가 아니거나 이어 열기·이어 하기면 그대로.
// 이미 --session-id 가 있으면 새 id 로 바꾼다([다시 열기]가 대화 없이 끝난 세션을 다시 띄울 때)
func freshSessionCommand(_ command: String) -> (String, String?) {
    let has = { (pattern: String) in command.range(of: pattern, options: .regularExpression) != nil }
    guard has(#"^\s*(\S*/)?claude(\s|$)"#), !has(#"(^|\s)(--resume|-r|--continue|-c)(=|\s|$)"#) else { return (command, nil) }
    // 따옴표 안 공백이 상하지 않게 문자열을 쪼개지 않고 고친다
    let base = command.replacingOccurrences(of: #"\s--session-id(=|\s+)\S+"#, with: "", options: .regularExpression)
    let id = UUID().uuidString.lowercased()
    return (base + " --session-id " + id, id)
}

// 펼쳤을 때 보드 폭 — 창이 좁으면 터미널 최소 폭을 남기고 줄인다
@MainActor func boardPaneWidth() -> CGFloat {
    guard TerminalHub.shared.expanded else { return window.frame.width }
    return max(boardMinWidth, min(TerminalHub.shared.boardWidth, window.frame.width - paneDividerWidth - terminalMinWidth))
}

let terminalExpandedMinSize = NSSize(width: boardMinWidth + paneDividerWidth + terminalMinWidth, height: 300)

// 펼치기: 창을 화면의 쓸 수 있는 영역 전체로. 접기: 보드만 화면 오른쪽에 끈 폭으로.
// 펼친 동안에는 "위에 고정" 을 쓰지 않는다 — 화면 전체를 덮은 창이 늘 위에 있으면 다른 앱을 쓸 수 없다
@MainActor func setTerminalExpanded(_ on: Bool) {
    let hub = TerminalHub.shared
    guard hub.expanded != on, let vf = (window.screen ?? NSScreen.main)?.visibleFrame else { return }
    hub.setExpanded(on)
    if on {
        window.contentMinSize = terminalExpandedMinSize
        window.setFrame(vf, display: true)
    } else {
        // 숨는 터미널에 키 입력이 가지 않게 포커스를 거둔다
        window.makeFirstResponder(nil)
        window.contentMinSize = NSSize(width: miniWidth, height: miniMinHeight)
        window.setFrame(NSRect(x: vf.maxX - hub.boardWidth, y: vf.minY, width: hub.boardWidth, height: vf.height), display: true)
        saveRestoreFrame()
    }
    applyPin(UserDefaults.standard.bool(forKey: "pinned"))
    window.contentView?.needsLayout = true
    BoardFit.shared.update()
    WindowOpacity.shared.apply()
    hub.applyVisibility()
    hub.focusSelected()
}

// ⌃⌥T·제목 막대 단추 — 펼쳐 있어도 보드가 앞에 없으면(다른 앱을 쓰는 중) 접지 않고 앞으로 꺼내 터미널에 포커스를 준다
@MainActor func toggleTerminal() {
    let hub = TerminalHub.shared
    if hub.expanded, !(NSApp.isActive && window.isKeyWindow) {
        showBoard()
        hub.focusSelected()
        return
    }
    setTerminalExpanded(!hub.expanded)
    if hub.expanded { showBoard() }
}

// 창 내용: [터미널 영역 | 경계 | 보드]. 터미널 영역은 제목 막대 띠까지 덮고(창 바탕이 투명하다) 탭 줄은 그 아래에 둔다.
// 접힌 동안에도 터미널 영역은 창에 붙은 채 펼쳤을 때 크기로 둔다 —
// 크기가 0 이면 셸이 뜨지 않고, 크기가 바뀌면 안의 프로그램이 다시 그린다
final class BoardRootView: NSView {
    let terminal = TerminalAreaView()
    let divider = PaneDivider()
    let board: NSView

    init(board: NSView) {
        self.board = board
        super.init(frame: .zero)
        addSubview(terminal)
        addSubview(divider)
        addSubview(board)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func layout() { super.layout(); place() }
    override func resizeSubviews(withOldSize oldSize: NSSize) { place() }

    private func place() {
        guard let w = window else { return }
        let below = w.contentLayoutRect.height   // 제목 막대 아래 높이
        terminal.titlebar = bounds.height - below
        if TerminalHub.shared.expanded {
            let bw = boardPaneWidth(), tw = bounds.width - bw - paneDividerWidth
            terminal.frame = NSRect(x: 0, y: 0, width: tw, height: bounds.height)
            divider.frame = NSRect(x: tw, y: 0, width: paneDividerWidth, height: bounds.height)
            board.frame = NSRect(x: bounds.width - bw, y: 0, width: bw, height: bounds.height)
        } else {
            var size = terminal.frame.size
            if size.width < terminalMinWidth, let vf = (w.screen ?? NSScreen.main)?.visibleFrame {
                size = NSSize(width: vf.width - TerminalHub.shared.boardWidth - paneDividerWidth, height: vf.height)
            }
            terminal.frame = NSRect(x: -size.width - 100, y: 0, width: size.width, height: size.height)
            board.frame = bounds
        }
        terminal.isHidden = !TerminalHub.shared.expanded
        divider.isHidden = !TerminalHub.shared.expanded
    }
}

// 누르면 단추는 동작하되 키보드 초점은 가져가지 않는 SwiftUI 호스팅 뷰 — 탭을 누른 뒤에도 입력이 터미널로 가게
final class PassiveHostingView<Content: View>: NSHostingView<Content> {
    override var acceptsFirstResponder: Bool { false }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

// 터미널 영역 — 맨 위 제목 막대 띠, 탭 줄, 아래에 탭마다 터미널 뷰(고른 것만 보임)
final class TerminalAreaView: NSView {
    let container = NSView()
    let empty = PassiveHostingView(rootView: TerminalEmptyView())
    let exitBar = PassiveHostingView(rootView: TerminalExitBar(hub: TerminalHub.shared))
    private let tabBar = PassiveHostingView(rootView: TerminalTabBar(hub: TerminalHub.shared))
    var titlebar: CGFloat = 0 { didSet { if titlebar != oldValue { needsLayout = true } } }

    override init(frame: NSRect) {
        super.init(frame: frame)
        wantsLayer = true
        layer?.backgroundColor = terminalBarColor.cgColor
        container.wantsLayer = true
        container.layer?.backgroundColor = terminalAreaColor.cgColor
        tabBar.sizingOptions = []
        empty.sizingOptions = []
        exitBar.sizingOptions = []
        exitBar.isHidden = true
        addSubview(container)
        addSubview(empty)
        addSubview(exitBar)
        addSubview(tabBar)
    }
    required init?(coder: NSCoder) { fatalError() }

    override func layout() {
        super.layout()
        let top = bounds.height - titlebar
        tabBar.frame = NSRect(x: 0, y: top - terminalTabBarHeight, width: bounds.width, height: terminalTabBarHeight)
        container.frame = NSRect(x: 0, y: 0, width: bounds.width, height: max(0, top - terminalTabBarHeight))
        empty.frame = container.frame
        exitBar.frame = NSRect(x: 0, y: 0, width: bounds.width, height: 56)
    }
}

// 터미널과 보드 사이 경계 — 끌어서 보드 폭을 바꾸고, 놓으면 기억한다
final class PaneDivider: NSView {
    private var startX: CGFloat = 0
    private var startWidth: CGFloat = 0
    private var dragging = false { didSet { needsDisplay = true } }

    override var mouseDownCanMoveWindow: Bool { false }   // 창은 배경을 끌어 옮길 수 있게 되어 있다
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func resetCursorRects() { addCursorRect(bounds, cursor: .resizeLeftRight) }

    override func draw(_ dirtyRect: NSRect) {
        NSColor(red: 0.11, green: 0.11, blue: 0.14, alpha: 1).setFill()
        bounds.fill()
        (dragging ? NSColor(red: 0.48, green: 0.64, blue: 0.97, alpha: 1) : NSColor(white: 0.36, alpha: 1)).setFill()
        NSBezierPath(roundedRect: NSRect(x: bounds.midX - 1.5, y: bounds.midY - 22, width: 3, height: 44), xRadius: 1.5, yRadius: 1.5).fill()
    }

    override func mouseDown(with event: NSEvent) {
        startX = event.locationInWindow.x
        startWidth = boardPaneWidth()
        dragging = true
    }

    override func mouseDragged(with event: NSEvent) {
        let hub = TerminalHub.shared
        hub.boardWidth = max(boardMinWidth, min(startWidth - (event.locationInWindow.x - startX),
                                                 (window?.frame.width ?? 0) - paneDividerWidth - terminalMinWidth))
        superview?.needsLayout = true
        BoardFit.shared.update()
    }

    override func mouseUp(with event: NSEvent) {
        dragging = false
        UserDefaults.standard.set(Double(TerminalHub.shared.boardWidth), forKey: "boardPaneWidth")
    }
}

struct TerminalTabBar: View {
    @ObservedObject var hub: TerminalHub
    @State private var newHover = false

    var body: some View {
        HStack(spacing: 2) {
            ForEach(hub.tabs) { t in TerminalTabButton(tab: t, selected: t.id == hub.selected, background: hub.background) }
            Button { hub.newSessionSheet() } label: {
                HStack(spacing: 5) {
                    Image(systemName: "plus").font(.system(size: 10, weight: .bold))
                    Text("새 세션")
                }
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(.primary.opacity(newHover ? 1 : 0.8))
                .padding(.horizontal, 11)
                .frame(height: 26)
                .background(RoundedRectangle(cornerRadius: 8).fill(Color.white.opacity(newHover ? 0.08 : 0)))
                .overlay(RoundedRectangle(cornerRadius: 8).strokeBorder(Color.white.opacity(0.22), style: StrokeStyle(lineWidth: 1, dash: [3, 2])))
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .clickable($newHover)
            .padding(.leading, 6)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 8)
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(Color(nsColor: terminalBarColor))
        .overlay(alignment: .bottom) { Rectangle().fill(Color.white.opacity(0.07)).frame(height: 1) }
        .preferredColorScheme(.dark)
    }
}

struct TerminalTabButton: View {
    @ObservedObject var tab: BoardTab
    let selected: Bool
    let background: NSColor
    @State private var hover = false
    @State private var closeHover = false

    var body: some View {
        HStack(spacing: 8) {
            // 짝지은 세션의 상태 색과 제목 — 셸 탭이나 아직 요청 전이면 터미널 제목
            Circle().fill(Palette.color(tab.exited ? "ended" : tab.session?.state ?? "idle")).frame(width: 8, height: 8)
            Text(tab.name).font(.system(size: 13, weight: .semibold)).lineLimit(1).fixedSize()
            Text(tab.session.map(doing) ?? tab.title).font(.system(size: 12)).foregroundStyle(.secondary).lineLimit(1)
            Spacer(minLength: 0)
            Button { TerminalHub.shared.requestClose(tab) } label: {
                Image(systemName: "xmark").font(.system(size: 8, weight: .bold))
                    .foregroundStyle(closeHover ? .primary : .secondary)
                    .frame(width: 16, height: 16)
                    .background(Circle().fill(Color.white.opacity(closeHover ? 0.14 : 0)))
            }
            .buttonStyle(.plain)
            .opacity(hover || selected ? 1 : 0)
            .onHover { closeHover = $0 }
            .hint("탭 닫기")
        }
        .padding(.leading, 14).padding(.trailing, 8)
        .frame(minWidth: 120, maxWidth: 250, maxHeight: .infinity)
        .background(selected ? Color(nsColor: background) : Color.white.opacity(hover ? 0.04 : 0))
        .overlay(alignment: .top) { Rectangle().fill(selected ? Palette.color("working") : .clear).frame(height: 2) }
        .foregroundStyle(selected ? .primary : .secondary)
        .contentShape(Rectangle())
        .onTapGesture { TerminalHub.shared.select(tab.id) }
        .onHover { hover = $0 }
    }
}

// 명령(claude)이 끝난 탭의 아래 막대 — 끝난 화면 위에 [다시 열기][셸 열기]
struct TerminalExitBar: View {
    @ObservedObject var hub: TerminalHub
    @State private var hover = false
    @State private var shellHover = false

    var body: some View {
        if let tab = hub.tabs.first(where: { $0.id == hub.selected }) {
            HStack(spacing: 10) {
                Image(systemName: "stop.circle").foregroundStyle(.secondary)
                Text("\(tab.session.map { $0.topic.isEmpty ? "세션" : $0.topic } ?? "명령") — 끝났습니다")
                    .font(.system(size: 12, weight: .semibold)).lineLimit(1)
                Spacer(minLength: 8)
                Button { hub.relaunch(tab) } label: {
                    Label("다시 열기", systemImage: "arrow.clockwise")
                        .padding(.horizontal, 12).frame(height: 28)
                        .background(Capsule().fill(Palette.color("working").opacity(hover ? 0.45 : 0.32)))
                }
                .buttonStyle(.plain).clickable($hover)
                .hint("같은 대화를 이 탭에서 이어 엽니다")
                Button { hub.openShellHere(tab) } label: {
                    Label("셸 열기", systemImage: "terminal")
                        .padding(.horizontal, 12).frame(height: 28)
                        .background(Capsule().fill(Color.white.opacity(shellHover ? 0.18 : 0.1)))
                }
                .buttonStyle(.plain).clickable($shellHover)
                .hint("이 작업 경로에서 셸을 엽니다")
            }
            .font(.system(size: 12, weight: .semibold))
            .padding(.horizontal, 16)
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(Color(nsColor: terminalBarColor).opacity(0.96))
            .overlay(alignment: .top) { Rectangle().fill(Color.white.opacity(0.08)).frame(height: 1) }
            .preferredColorScheme(.dark)
        }
    }
}

// 새 세션 기본 명령 — 승인 대기가 거의 없게 권한 확인을 건너뛰는 것도 의도한 것(2026-10-17 사용자)
let defaultSessionCommand = "claude --dangerously-skip-permissions"

struct NewSessionSheet: View {
    let paths: [String]
    let cancel: () -> Void
    let start: (String, String) -> Void
    @ObservedObject private var hub = TerminalHub.shared
    @State private var dir: String
    @State private var others: [String] = []
    @State private var command = defaultSessionCommand

    init(paths: [String], initial: String, cancel: @escaping () -> Void, start: @escaping (String, String) -> Void) {
        self.paths = paths
        self.cancel = cancel
        self.start = start
        _dir = State(initialValue: initial)
        _others = State(initialValue: paths.contains(initial) ? [] : [initial])
    }

    private func row(_ p: String) -> some View {
        let on = p == dir
        return Button { dir = p } label: {
            HStack(spacing: 10) {
                Image(systemName: on ? "largecircle.fill.circle" : "circle")
                    .foregroundStyle(on ? Palette.color("working") : .secondary)
                VStack(alignment: .leading, spacing: 1) {
                    Text((p as NSString).lastPathComponent).font(.system(size: 13, weight: .semibold))
                    Text(p.replacingOccurrences(of: NSHomeDirectory(), with: "~")).font(.system(size: 11)).foregroundStyle(.secondary)
                        .lineLimit(1).truncationMode(.middle)
                }
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10).padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 8).fill(on ? Palette.color("working").opacity(0.16) : .clear))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func pick() {
        let panel = NSOpenPanel()
        panel.canChooseDirectories = true
        panel.canChooseFiles = false
        panel.directoryURL = URL(fileURLWithPath: dir)
        guard panel.runModal() == .OK, let p = panel.url?.path else { return }
        if !paths.contains(p), !others.contains(p) { others.append(p) }
        dir = p
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("새 세션").font(.system(size: 16, weight: .bold))
            VStack(alignment: .leading, spacing: 6) {
                Text("작업 폴더 — 보드에 있는 경로").font(.system(size: 12, weight: .semibold)).foregroundStyle(.secondary)
                ScrollView {
                    VStack(spacing: 2) { ForEach(paths + others, id: \.self) { row($0) } }
                }
                .frame(height: min(CGFloat(paths.count + others.count) * 46, 230))   // 스크롤 영역은 높이를 정해 주지 않으면 0 으로 접힌다
                Button("다른 폴더 고르기…", action: pick)
            }
            VStack(alignment: .leading, spacing: 6) {
                Text("실행할 명령").font(.system(size: 12, weight: .semibold)).foregroundStyle(.secondary)
                TextField("", text: $command).font(.system(size: 13, design: .monospaced)).textFieldStyle(.roundedBorder)
                Text("셸을 거치지 않고 바로 실행합니다 — claude 면 보드가 --session-id 를 붙입니다")
                    .font(.system(size: 11)).foregroundStyle(.secondary)
            }
            HStack {
                if !hub.envReady { Text("셸 환경을 읽는 중…").font(.system(size: 11)).foregroundStyle(.secondary) }
                Spacer()
                Button("취소", action: cancel).keyboardShortcut(.cancelAction)
                Button("새 탭에서 시작") { start(command.trimmingCharacters(in: .whitespaces), dir) }
                    .keyboardShortcut(.defaultAction)
                    .disabled(!hub.envReady || command.trimmingCharacters(in: .whitespaces).isEmpty)
            }
        }
        .padding(18)
        .frame(width: 460)
    }
}

// 보드 아래쪽에 잠깐 뜨는 안내 — 백그라운드 세션을 눌렀을 때 등
struct TerminalNotice: View {
    @ObservedObject var hub: TerminalHub
    @State private var hover = false
    var body: some View {
        if let text = hub.notice {
            HStack(alignment: .top, spacing: 10) {
                Text(text).font(.system(size: 12)).fixedSize(horizontal: false, vertical: true)
                Spacer(minLength: 0)
                Button { hub.dismissNotice() } label: {
                    Image(systemName: "xmark").font(.system(size: 9, weight: .bold))
                        .frame(width: 22, height: 22)
                        .background(Circle().fill(Color.white.opacity(hover ? 0.2 : 0.1)))
                }
                .buttonStyle(.plain).clickable($hover)
            }
            .padding(12)
            .background(RoundedRectangle(cornerRadius: 12).fill(Color(red: 0.18, green: 0.16, blue: 0.24)))
            .overlay(RoundedRectangle(cornerRadius: 12).stroke(Color(red: 0.36, green: 0.30, blue: 0.48), lineWidth: 1))
            .shadow(color: .black.opacity(0.4), radius: 12, y: 6)
            .padding(.horizontal, 14).padding(.bottom, 34)
            .transition(.move(edge: .bottom).combined(with: .opacity))
            .preferredColorScheme(.dark)
        }
    }
}

// 탭이 하나도 없을 때
struct TerminalEmptyView: View {
    @State private var hover = false
    var body: some View {
        VStack(spacing: 12) {
            Text("보드에서 띄운 세션이 없습니다").font(.system(size: 14, weight: .semibold))
            Text("보드에서 시작하거나 이어 연 세션이 여기 탭으로 열립니다").font(.system(size: 12)).foregroundStyle(.secondary)
            Button { TerminalHub.shared.newSessionSheet() } label: {
                HStack(spacing: 6) { Image(systemName: "plus"); Text("새 세션") }
                    .font(.system(size: 12, weight: .semibold))
                    .padding(.horizontal, 14).frame(height: 30)
                    .background(Capsule().fill(Color.white.opacity(hover ? 0.16 : 0.1)))
            }
            .buttonStyle(.plain)
            .clickable($hover)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .preferredColorScheme(.dark)
    }
}

// 제목 막대의 "터미널" 단추 — 미니 제목 막대(폭 300)에는 자리가 없어 숨긴다
struct TerminalToggleButton: View {
    @ObservedObject private var hub = TerminalHub.shared
    @State private var hover = false
    var body: some View {
        let on = hub.expanded
        Button(action: toggleTerminal) {
            HStack(spacing: 5) {
                Image(systemName: "rectangle.leadinghalf.inset.filled")
                Text(on ? "터미널 펼침" : "터미널 접힘").fixedSize()
                Text("⌃⌥T").font(.system(size: 10, weight: .medium))
                    .padding(.horizontal, 4)
                    .background(RoundedRectangle(cornerRadius: 4).fill(Color.white.opacity(0.08)))
            }
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(on ? Palette.color("working") : .primary.opacity(hover ? 1 : 0.8))
            .padding(.horizontal, 8)
            .frame(height: 20)
            .background(Capsule().fill(on ? Palette.color("working").opacity(0.18) : Color.white.opacity(hover ? 0.30 : 0.18)))
            .overlay(Capsule().stroke(on ? Palette.color("working").opacity(0.45) : Color.white.opacity(0.18), lineWidth: 0.5))
            .frame(height: 28)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint(on ? "터미널 접기 — 보드만 오른쪽에 남깁니다 (⌃⌥T)" : "터미널 펼치기 — 보드에서 띄운 세션을 왼쪽 탭에서 봅니다 (⌃⌥T)")
        .fixedSize()
    }
}
let terminalToggleWidth: CGFloat = 146
var terminalToggleSink: AnyCancellable?

@MainActor func addTerminalToggle() {
    let vc = NSTitlebarAccessoryViewController()
    let host = FirstClickHostingView(rootView: TerminalToggleButton())
    host.frame = NSRect(x: 0, y: 0, width: terminalToggleWidth, height: 28)
    vc.view = host
    vc.layoutAttribute = .leading
    window.addTitlebarAccessoryViewController(vc)
    // 왼쪽 부속에는 vc.isHidden 이 듣지 않는다 — 뷰를 숨기고 폭을 0 으로 줄여 자리도 비운다
    terminalToggleSink = BoardFit.shared.$tier.sink { t in
        host.isHidden = t == .mini
        host.frame.size.width = t == .mini ? 0 : terminalToggleWidth
    }
}

// MARK: - 창

var window: NSWindow!

// 보드가 다른 앱 뒤에 있을 때도 첫 클릭이 창 활성화에만 쓰이지 않고 바로 단추에 닿게 한다
final class FirstClickHostingView<Content: View>: NSHostingView<Content> {
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
}

// 창을 손으로 줄일 때 최소 크기 아래로 내려가지 않게 한다
final class SizeGuard: NSObject, NSWindowDelegate {
    func windowWillResize(_ sender: NSWindow, to frameSize: NSSize) -> NSSize {
        let min = sender.frameRect(forContentRect: NSRect(origin: .zero, size: sender.contentMinSize)).size
        return NSSize(width: max(frameSize.width, min.width), height: max(frameSize.height, min.height))
    }
}
let sizeGuard = SizeGuard()
let boardMinWidth: CGFloat = 360        // 전체·중간 화면이 무너지지 않는 폭 — 더 좁으면 미니로 그린다
let boardLaunchWidth: CGFloat = 454   // 실행할 때의 가로 폭 — 2026-10-13 사용자가 맞춰 둔 폭

// MARK: - 창 크기 단계

// 창 크기에 따라 보드를 세 단계로 그린다.
// 전체 = 기본 화면, 중간 = 사용 한도는 한 줄·경로 카드는 접힌 한 줄 요약, 미니 = 캐릭터 한 줄과 경로마다 한 줄.
// 제목 오른쪽 캐릭터는 세 단계 모두에서 보인다
enum BoardTier { case full, medium, mini }

let miniWidth: CGFloat = 300            // 최대축소 폭이자 창의 최소 폭
let miniMinHeight: CGFloat = 144        // 창의 최소 높이 — 미니에서 캐릭터 줄·경로 두 줄·아래 줄이 보이는 높이
let miniRowHeight: CGFloat = 26         // 미니의 경로 한 줄 높이 — 최대축소할 때 경로 수만큼 창 높이를 잡는다
let fullListMin: CGFloat = 120          // 전체로 그리려면 남아야 하는 목록 칸 — 펼친 카드 하나쯤
let mediumListMin: CGFloat = 56         // 중간으로 그리려면 남아야 하는 목록 칸 — 접힌 카드 하나

@MainActor final class BoardFit: ObservableObject {
    static let shared = BoardFit()
    @Published private(set) var tier: BoardTier = .full
    // 단계마다 목록 칸을 뺀 보드 내용 높이 — 그 단계로 그려졌을 때만 잴 수 있어 잰 값을 기억한다(재기 전엔 어림값)
    private var fixed: [BoardTier: CGFloat] = [.full: 330, .medium: 200]

    // 그려진 단계의 보드 높이와 목록 칸 높이를 받는다 — 지금 단계로 그린 값만 쓴다
    func measured(_ t: BoardTier, board: CGFloat, list: CGFloat) {
        guard t == tier, t != .mini, board > 0, abs((fixed[t] ?? 0) - (board - list)) > 0.5 else { return }
        fixed[t] = board - list
        update()
    }

    // 창 크기나 잰 값이 바뀌면 단계를 다시 고른다 — 창 크기는 바꾸지 않는다
    func update() {
        guard let w = window else { return }
        let h = w.frame.height, titlebar = h - w.contentLayoutRect.height   // 보드 내용은 제목 막대 아래에서 시작한다
        let fits = { (t: BoardTier, list: CGFloat) in h >= (self.fixed[t] ?? 0) + list + titlebar }
        let t: BoardTier = boardPaneWidth() < boardMinWidth ? .mini   // 터미널을 펼쳤으면 창이 아니라 보드 칸의 폭
            : fits(.full, fullListMin) ? .full : fits(.medium, mediumListMin) ? .medium : .mini
        guard t != tier else { return }
        tier = t
        UserDefaults.standard.set(t == .mini, forKey: "boardMini")   // 미니로 둔 채 끄면 다음에도 그 크기로 연다
    }
}

// 미니에서 ⤢ 로 돌아갈 창 위치 — 최대축소할 때, 미니가 아닌 크기로 손으로 바꾼 뒤, 처음 크기로 붙일 때, 미니가 아닌 채 실행할 때 기억한다.
// 창이 줄어드는 애니메이션 중간 크기가 기억되지 않게 크기가 바뀔 때마다 기억하지는 않는다
func saveRestoreFrame() { UserDefaults.standard.set(NSStringFromRect(window.frame), forKey: "boardRestoreFrame") }

// 최대축소 — 오른쪽 위 모서리는 그대로 두고 폭은 miniWidth, 높이는 경로 수(최대 6줄)에 맞춘다
@MainActor func enterMini(rows: Int) {
    setTerminalExpanded(false)   // 터미널을 펼쳤으면 먼저 접는다 — 미니에서 돌아갈 위치는 접힌 보드
    saveRestoreFrame()
    let f = window.frame
    let h = miniMinHeight + CGFloat(min(max(rows, 2), 6) - 2) * miniRowHeight   // 최소 높이에 두 줄이 들어간다
    window.setFrame(NSRect(x: f.maxX - miniWidth, y: f.maxY - h, width: miniWidth, height: h), display: true, animate: true)
}

// 미니에서 원래 크기로 — 기억한 위치가 없거나 지금 연결된 화면 밖이면 처음 크기로 오른쪽에 붙인다
@MainActor func leaveMini() {
    if let s = UserDefaults.standard.string(forKey: "boardRestoreFrame") {
        let r = NSRectFromString(s)
        if r.width >= boardMinWidth, NSScreen.screens.contains(where: { $0.visibleFrame.intersects(r) }) {
            window.setFrame(r, display: true, animate: true)
            return
        }
    }
    snapRight()
}

// 제목 막대 오른쪽 — 미니에서는 상태별 세션 수와 원래 크기로 되돌리기 단추, 전체·중간에서는 같은 자리에 최대축소 단추
func addMiniTitlebar(_ store: Store) {
    let vc = NSTitlebarAccessoryViewController()
    let host = FirstClickHostingView(rootView: MiniTitleBar(store: store))
    host.frame = NSRect(x: 0, y: 0, width: MiniTitleBar.width, height: 28)   // 제목 막대 높이와 같게 — 더 높으면 제목 막대가 커진다
    vc.view = host
    vc.layoutAttribute = .trailing
    window.addTitlebarAccessoryViewController(vc)
}

struct MiniTitleBar: View {
    static let width: CGFloat = 176     // 미니 폭 300 에서 창 단추·붙이기 단추를 뺀 자리
    @ObservedObject var store: Store
    @ObservedObject private var fit = BoardFit.shared
    @State private var hover = false

    // 상태별 세션 수 — 자리가 모자라면 글자를 빼고 수만
    private func chips(labels: Bool) -> some View {
        HStack(spacing: 4) {
            ForEach(["waiting", "working", "idle"], id: \.self) { st in
                let n = store.count(st)
                if n > 0 {
                    HStack(spacing: 3) {
                        Circle().fill(Palette.color(st)).frame(width: 6, height: 6)
                        Text(labels ? "\(n) \(["waiting": "승인", "working": "작업", "idle": "대기"][st] ?? "")" : "\(n)")
                            .monospacedDigit()
                    }
                    .font(.system(size: 10.5, weight: .bold))
                    .padding(.horizontal, 6).padding(.vertical, 2)
                    .background(Capsule().fill(Palette.color(st).opacity(0.15)))
                    .fixedSize()
                    .hint("\(Palette.name(st)) \(n)개")
                }
            }
        }
    }

    var body: some View {
        let mini = fit.tier == .mini
        HStack(spacing: 5) {
            Spacer(minLength: 0)
            if mini {
                ViewThatFits(in: .horizontal) { chips(labels: true); chips(labels: false) }
            }
            // 미니 = 원래 크기로, 전체·중간 = 최대축소(창을 가장 작게 줄여 경로마다 한 줄로 본다)
            Button { if mini { leaveMini() } else { enterMini(rows: store.groups.count) } } label: {
                Image(systemName: mini ? "arrow.up.left.and.arrow.down.right" : "arrow.down.right.and.arrow.up.left")
                    .font(.system(size: 10, weight: .bold))
                    .foregroundStyle(hover ? .primary : .secondary)
                    .frame(width: 24, height: 20)
                    .background(Capsule().fill(Color.white.opacity(hover ? 0.16 : 0.08)))
                    .contentShape(Capsule())
            }
            .buttonStyle(.plain)
            .clickable($hover)
            .hint(mini ? "원래 크기로 되돌리기" : "최대축소 — 창을 가장 작게 줄이고 경로마다 한 줄로 봅니다")
        }
        .padding(.trailing, 8)
        .frame(width: Self.width, height: 28)
        .preferredColorScheme(.dark)
    }
}

// 미니 화면 — 경로마다 한 줄(상태 · 이름 · 지금 하는 일), 아래 줄에 사용 한도와 투명도 슬라이더
struct MiniBoard: View {
    @ObservedObject var store: Store
    @ObservedObject var usage: UsageStore

    // 사용 한도 한 줄 — 앞에서 n 개만
    private func limits(_ n: Int) -> some View {
        HStack(spacing: 6) {
            ForEach(Array(usage.limits.prefix(n).enumerated()), id: \.element.id) { i, l in
                if i > 0 { Rectangle().fill(Color.white.opacity(0.14)).frame(width: 0.5, height: 10) }
                CompactLimit(limit: l, level: 2, now: store.now)
            }
        }
        .fixedSize()
    }

    var body: some View {
        VStack(spacing: 4) {
            // 캐릭터와 말풍선 — 최소 크기에서도 보인다
            HStack(spacing: 0) {
                Spacer(minLength: 0)
                Foreman(groups: store.groups, limits: usage.limits, now: store.now, compact: true)
            }
            if store.groups.isEmpty {
                Text("아직 기록된 세션이 없습니다").font(.system(size: 11)).foregroundStyle(.secondary)
                    .frame(maxWidth: .infinity, minHeight: 0, maxHeight: .infinity)
            } else {
                ScrollView(showsIndicators: false) {
                    VStack(spacing: 2) {
                        ForEach(store.groups) { g in MiniRow(g: g, now: store.now) }
                    }
                }
            }
            HStack(spacing: 6) {
                // 자리가 모자라면 한도를 앞의 둘만 — 투명도 슬라이더는 늘 보인다
                ViewThatFits(in: .horizontal) { limits(usage.limits.count); limits(2); Color.clear.frame(width: 0, height: 0) }
                Spacer(minLength: 4)
                OpacitySlider(compact: true)
            }
        }
        .padding(.horizontal, 10)
        .padding(.top, 2)
        .padding(.bottom, 6)
        .animation(.spring(response: 0.45, dampingFraction: 0.82), value: store.groups)
    }
}

// 미니 화면의 경로 한 줄 — 누르면 대표 세션의 터미널 앱을, 두 번 누르면 그 세션의 터미널 창을 앞으로
struct MiniRow: View {
    let g: Group
    let now: Date
    @State private var hover = false

    var body: some View {
        let top = g.active.first
        let first = g.sessions.first
        let live = top ?? g.sessions.first { $0.state != "ended" }
        HStack(spacing: 8) {
            StateIcon(state: g.urgent).scaleEffect(0.8).frame(width: 15, height: 15)
            Text(g.name).font(.system(size: 12, weight: .semibold)).lineLimit(1).layoutPriority(1)
            HStack(spacing: 4) {
                // 줄에 보이는 세션이 터미널에서 도는지 백그라운드로 도는지 — 아이콘만
                if let s = top ?? first { KindLabel(headless: s.headless, compact: true) }
                if let top {
                    Text(doing(top)).lineLimit(1).foregroundStyle(Palette.color(top.state))
                    if g.active.count > 1 { Text("+\(g.active.count - 1)").fixedSize().foregroundStyle(Palette.color(top.state)) }
                } else if let s = first {
                    Text("\(Palette.name(s.state)) · " + (s.state == "idle" ? "\(elapsed(s.since, now)) 전"
                                                          : s.state == "ended" ? clock(s.updated) : s.title))
                        .lineLimit(1).foregroundStyle(.secondary)
                }
            }
            .font(.system(size: 11))
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 8)
        .frame(height: miniRowHeight - 2)
        .background(RoundedRectangle(cornerRadius: 7).fill(Color.white.opacity(hover ? 0.07 : 0)))
        .modifier(Shimmer(active: g.urgent == "working", color: Palette.color("working")))
        .contentShape(Rectangle())
        .onHover { h in withAnimation(.easeOut(duration: 0.15)) { hover = h } }
        // 세션 줄과 같게 클릭 횟수로 나눈다 — 두 제스처로 나누면 한 번 클릭이 0.5초 늦는다
        .onTapGesture {
            guard let s = live else { return }
            openSession(s, double: (NSApp.currentEvent?.clickCount ?? 1) >= 2)
        }
        .hint(live == nil ? g.path : "\(g.path) — 눌러서 터미널 앱, 두 번 눌러서 그 세션으로 이동")
    }
}

// 처음 실행할 때의 크기(폭 boardLaunchWidth, 높이는 화면의 쓸 수 있는 높이 전체)로 되돌려 지금 화면의 오른쪽 끝에 붙인다.
// 터미널을 펼쳤으면 접는다 — "보드만 오른쪽에" 라는 뜻을 그대로 둔다(펼치기는 ⌃⌥T)
@MainActor func snapRight() {
    setTerminalExpanded(false)
    guard let vf = (window.screen ?? NSScreen.main)?.visibleFrame else { return }
    let f = NSRect(x: vf.maxX - boardLaunchWidth, y: vf.minY, width: boardLaunchWidth, height: vf.height)
    window.setFrame(f, display: true, animate: true)
    saveRestoreFrame()
}

// 창 단추(닫기·최소화·확대) 바로 오른쪽에 붙는 "처음 크기로 오른쪽에 붙이기" 단추
struct SnapButton: View {
    @State private var hover = false
    var body: some View {
        Button(action: snapRight) {
            Image(systemName: "rectangle.righthalf.inset.filled")
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(.primary.opacity(hover ? 1 : 0.8))
                .frame(width: 26, height: 18)
                .background(Capsule().fill(Color.white.opacity(hover ? 0.30 : 0.18)))
                .overlay(Capsule().stroke(Color.white.opacity(0.18), lineWidth: 0.5))
                .scaleEffect(hover ? 1.12 : 1)
                .animation(.spring(response: 0.25, dampingFraction: 0.6), value: hover)
                .frame(width: 34, height: 28)   // 누를 수 있는 영역은 캡슐보다 넉넉하게
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .clickable($hover)
        .hint("처음 크기로 되돌려 화면 오른쪽에 붙이기 (⌃⌥B)")
        .padding(.leading, 4)
    }
}

// 전역 단축키 — ⌃⌥B: 어느 앱에서든 보드를 처음 크기로 오른쪽에 붙이고 앞으로 꺼낸다(포커스는 지금 앱에 남긴다).
// ⌃⌥T: 터미널 펼치기·접기(toggleTerminal). Carbon 단축키는 손쉬운 사용 권한이 필요 없다
func registerHotKeys() {
    var spec = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
    InstallEventHandler(GetApplicationEventTarget(), { _, event, _ in
        var key = EventHotKeyID()
        GetEventParameter(event, EventParamName(kEventParamDirectObject), EventParamType(typeEventHotKeyID), nil,
                          MemoryLayout<EventHotKeyID>.size, nil, &key)
        DispatchQueue.main.async {
            if key.id == 2 {
                toggleTerminal()
            } else {
                window.orderFrontRegardless()
                snapRight()
            }
        }
        return noErr
    }, 1, &spec, nil, nil)
    for (code, n, name) in [(kVK_ANSI_B, UInt32(1), "⌃⌥B"), (kVK_ANSI_T, UInt32(2), "⌃⌥T")] {
        var ref: EventHotKeyRef?
        let id = EventHotKeyID(signature: OSType(0x43424F44), id: n)   // 'CBOD'
        let status = RegisterEventHotKey(UInt32(code), UInt32(controlKey | optionKey), id, GetApplicationEventTarget(), 0, &ref)
        // 다른 앱이 먼저 잡은 단축키면 등록에 실패한다 — 보드는 그대로 동작하고 로그만 남긴다
        if status != noErr { boardLog("전역 단축키 \(name) 등록 실패 (\(status))") }
    }
}

func addSnapButton() {
    let vc = NSTitlebarAccessoryViewController()
    let host = NSHostingView(rootView: SnapButton())
    host.frame = NSRect(x: 0, y: 0, width: 38, height: 28)
    vc.view = host
    vc.layoutAttribute = .leading
    window.addTitlebarAccessoryViewController(vc)
}

func activateTerminal() {
    if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.mitchellh.ghostty") {
        NSWorkspace.shared.openApplication(at: url, configuration: NSWorkspace.OpenConfiguration())
    }
}

// 더블 클릭한 세션의 Ghostty 터미널로 포커스를 옮긴다.
// Ghostty AppleScript 사전에는 tty 가 없으므로, 그 tty 에 잠시 표지 제목을 써서 터미널을 찾은 뒤 원래 제목으로 되돌린다.
let ghosttyQueue = DispatchQueue(label: "com.claude.board.ghostty")

func runScript(_ source: String) -> NSAppleEventDescriptor? {
    var err: NSDictionary?
    let r = NSAppleScript(source: source)?.executeAndReturnError(&err)
    // 권한 거부 같은 AppleScript 오류는 board.log 에 남긴다 — 보드 화면에는 드러나지 않기 때문
    if let err {
        let line = "\(Date()) \(err[NSAppleScript.errorNumber] ?? "") \(err[NSAppleScript.errorMessage] ?? "")\n"
        let log = (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/board.log")
        if let h = FileHandle(forWritingAtPath: log) { h.seekToEndOfFile(); h.write(Data(line.utf8)); try? h.close() }
        else { FileManager.default.createFile(atPath: log, contents: Data(line.utf8)) }
    }
    return r
}

func writeTitle(_ tty: String, _ title: String) {
    guard let h = FileHandle(forWritingAtPath: "/dev/\(tty)") else { return }
    h.write("\u{1b}]2;\(title)\u{07}".data(using: .utf8)!)
    try? h.close()
}

// 지원하는 터미널 앱 — Ghostty 는 AppleScript 에 tty 가 없어 표지 제목으로 찾고, iTerm2·Terminal 은 tty 로 바로 찾는다
let ghosttyId = "com.mitchellh.ghostty", itermId = "com.googlecode.iterm2", terminalId = "com.apple.Terminal"

func activateApp(_ bundleId: String) {
    if let app = NSRunningApplication.runningApplications(withBundleIdentifier: bundleId).first {
        app.activate()
    } else if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: bundleId) {
        NSWorkspace.shared.openApplication(at: url, configuration: NSWorkspace.OpenConfiguration())
    }
}

func isRunning(_ bundleId: String) -> Bool {
    !NSRunningApplication.runningApplications(withBundleIdentifier: bundleId).isEmpty
}

// claude 프로세스의 조상을 따라 올라가 그 세션을 띄운 앱을 찾는다.
// iTerm2 는 셸을 별도 서버 프로세스(부모가 launchd)에서 돌린다 — 그 서버 실행 경로로 iTerm2 로 판별한다. 못 찾으면 nil
func ancestorApp(_ pid: Int32) -> String? {
    var p = pid
    for _ in 0..<12 where p > 1 {
        if let id = NSRunningApplication(processIdentifier: p)?.bundleIdentifier { return id }
        // 앱 안의 보조 프로세스(예: Orca Helper)는 실행 파일 경로에서 가장 바깥 .app 번들을 찾는다
        var buf = [CChar](repeating: 0, count: 4096)
        if proc_pidpath(p, &buf, UInt32(buf.count)) > 0, let r = String(cString: buf).range(of: ".app/"),
           let id = Bundle(path: String(String(cString: buf)[..<r.lowerBound]) + ".app")?.bundleIdentifier {
            return id
        }
        // iTerm2 의 셸 서버는 앱 번들 밖(~/Library/Application Support/iTerm2/iTermServer-x.y.z)에 있다
        if String(cString: buf).contains("/Application Support/iTerm2/iTermServer") { return itermId }
        var info = kinfo_proc()
        var size = MemoryLayout<kinfo_proc>.stride
        var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, p]
        guard sysctl(&mib, 4, &info, &size, nil, 0) == 0, size > 0 else { return nil }
        p = info.kp_eproc.e_ppid
    }
    return nil
}

// tty 로 iTerm2·Terminal 의 탭을 찾아 앞으로 가져온다 — 찾으면 true
func focusByTty(_ bundleId: String, _ tty: String) -> Bool {
    let dev = "/dev/" + tty
    let script: String
    if bundleId == itermId {
        script = """
        tell application id "\(itermId)"
          repeat with w in windows
            repeat with t in tabs of w
              repeat with s in sessions of t
                if tty of s is "\(dev)" then
                  select w
                  tell t to select
                  tell s to select
                  activate
                  return "ok"
                end if
              end repeat
            end repeat
          end repeat
          return ""
        end tell
        """
    } else {
        script = """
        tell application id "\(terminalId)"
          repeat with w in windows
            repeat with t in tabs of w
              if tty of t is "\(dev)" then
                set selected tab of w to t
                set index of w to 1
                activate
                return "ok"
              end if
            end repeat
          end repeat
          return ""
        end tell
        """
    }
    return runScript(script)?.stringValue == "ok"
}

// 세션을 띄운 터미널 앱 — 조상으로 못 찾으면 실행 중인 iTerm2·Terminal 에서 tty 를 찾는다. 세션마다 기억한다
// ghosttyQueue 와 따로 잠근다 — ghosttyQueue 안(중단 등)에서 ownerApp 을 불러도 같은 큐에 sync 를 걸어 교착하지 않게
var ownerCache: [String: String] = [:]
let ownerLock = NSLock()

func ownerApp(_ s: Session) -> String? {
    if let c = ownerLock.withLock({ ownerCache[s.id] }) { return c }
    var found = s.pid > 0 ? ancestorApp(s.pid) : nil
    if found == nil, !s.tty.isEmpty {
        found = [itermId, terminalId].first { isRunning($0) && ttyExists($0, s.tty) }
    }
    if let f = found { ownerLock.withLock { ownerCache[s.id] = f } }
    return found
}

func ttyExists(_ bundleId: String, _ tty: String) -> Bool {
    let dev = "/dev/" + tty
    let script = bundleId == itermId
        ? "tell application id \"\(itermId)\" to return (count of (every session of every tab of every window whose tty is \"\(dev)\")) as text"
        : "tell application id \"\(terminalId)\" to return (count of (every tab of every window whose tty is \"\(dev)\")) as text"
    return (Int(runScript(script)?.stringValue ?? "") ?? 0) > 0
}

// 한 번 클릭: 그 세션의 터미널 앱만 앞으로 가져온다
func activateOwner(_ s: Session) {
    DispatchQueue.global(qos: .userInitiated).async {
        let app = s.headless || s.state == "ended" ? nil : ownerApp(s)
        DispatchQueue.main.async { app.map(activateApp) ?? activateTerminal() }
    }
}

func focusSession(_ s: Session) {
    guard !s.headless, !s.tty.isEmpty, s.state != "ended" else { return activateTerminal() }
    DispatchQueue.global(qos: .userInitiated).async {
        let app = ownerApp(s)
        if app == itermId || app == terminalId {
            if focusByTty(app!, s.tty) { return }
            DispatchQueue.main.async { activateApp(app!) }
            return
        }
        if let app, app != ghosttyId {   // 그 밖의 앱(VS Code 등)은 앱만 앞으로 가져온다
            DispatchQueue.main.async { activateApp(app) }
            return
        }
        focusGhostty(s)
    }
}

// 세션이 도는 Ghostty 터미널의 id 를 찾는다 — 그 tty 에 잠깐 표지 제목을 써서 찾고 원래 제목으로 되돌린다. ghosttyQueue 안에서 부른다
func ghosttyTerminalId(_ s: Session) -> String? {
    // tell 안에서 tab 은 Ghostty 의 탭 객체 이름으로 읽혀 글자 "tab" 이 들어간다 — 탭 문자는 character id 9 로 쓴다
    let list = """
    tell application "Ghostty"
      set out to ""
      repeat with t in terminals
        set out to out & (id of t) & (character id 9) & (name of t) & linefeed
      end repeat
      return out
    end tell
    """
    var before: [String: String] = [:]
    for line in (runScript(list)?.stringValue ?? "").split(separator: "\n") {
        let parts = line.split(separator: "\t", maxSplits: 1, omittingEmptySubsequences: false)
        if parts.count == 2 { before[String(parts[0])] = String(parts[1]) }
    }
    let marker = "CBFIND-\(UUID().uuidString.prefix(8))"
    writeTitle(s.tty, marker)
    var found: String?
    for _ in 0..<10 {
        Thread.sleep(forTimeInterval: 0.05)
        found = runScript("""
        tell application "Ghostty"
          repeat with t in terminals
            if name of t is "\(marker)" then return id of t
          end repeat
          return ""
        end tell
        """)?.stringValue
        if let f = found, !f.isEmpty { break }
    }
    if let f = found, !f.isEmpty, let old = before[f] {
        writeTitle(s.tty, old)
        return f
    }
    writeTitle(s.tty, (s.cwd as NSString).lastPathComponent)
    return nil
}

// Ghostty 터미널에 키·글자를 보낸다. 단계: ("key", 키 이름) | ("text", 붙여 넣을 글자) | ("wait", 초)
func ghosttySend(_ id: String, _ steps: [(String, String)]) -> Bool {
    let esc = { (t: String) in t.replacingOccurrences(of: "\\", with: "\\\\").replacingOccurrences(of: "\"", with: "\\\"") }
    let body = steps.map { kind, v in
        switch kind {
        case "key": return "send key \"\(esc(v))\" to t"
        case "text": return "input text \"\(esc(v))\" to t"
        default: return "delay \(v)"
        }
    }.joined(separator: "\n      ")
    return runScript("""
    tell application "Ghostty"
      repeat with t in terminals
        if (id of t as text) is "\(id)" then
          \(body)
          return "ok"
        end if
      end repeat
      return ""
    end tell
    """)?.stringValue == "ok"
}

// 터미널에 직접 키를 보낼 수 있는 세션인가 — 지금은 Ghostty 에서 도는 터미널 세션만
func canDrive(_ s: Session) -> Bool {
    !s.headless && !s.tty.isEmpty && s.state != "ended" && ownerApp(s) == ghosttyId
}

let sessionsDir = (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/sessions")
let muteAllPath = (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/mute-all")
// 알림 패널 위치 — 모서리를 고르면 그 값(top-left 등)을 남기고, notify.py 가 읽어 claude-notifier --position 으로 넘긴다. 없으면 가운데
let notifyPositionPath = (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/notify-position")

// 전체 알림이 꺼졌는지 — 세션 스위치를 잠그는 데 쓴다
private struct NotifyLockedKey: EnvironmentKey { static let defaultValue = false }
extension EnvironmentValues {
    var notifyLocked: Bool {
        get { self[NotifyLockedKey.self] }
        set { self[NotifyLockedKey.self] = newValue }
    }
}

func muteMark(_ s: Session) -> String { (sessionsDir as NSString).appendingPathComponent("\(s.id).mute") }

extension Notification.Name { static let boardReload = Notification.Name("com.claude.board.reload") }

// 알림 끄기 표지는 세션 기록이 지워질 때 state.py prune 이 함께 치운다
func setMuted(_ s: Session, _ on: Bool) {
    // 화면은 MuteToggle 이 먼저 바꾸고, 표지 파일 반영과 다시 읽기는 다음 틱에 한다
    DispatchQueue.main.async {
        if on {
            FileManager.default.createFile(atPath: muteMark(s), contents: Data())
        } else {
            try? FileManager.default.removeItem(atPath: muteMark(s))
        }
        NotificationCenter.default.post(name: .boardReload, object: nil)
    }
}

func pausedMark(_ s: Session) -> String { (sessionsDir as NSString).appendingPathComponent("\(s.id).paused") }
let carryOnPrompt = "보드에서 일시중지했던 세션을 재개합니다. 중단된 지점부터 직전 작업을 이어서 진행해 주세요."

// 일시중지: Esc 로 지금 응답을 멈춘다. 그때까지 한 작업은 대화 기록에 남아 있으므로 재개하면 바로 이어 간다
func pauseSession(_ s: Session) {
    ghosttyQueue.async {
        guard let id = ghosttyTerminalId(s), ghosttySend(id, [("key", "escape")]) else { return }
        let mark = ["paused_at": Date().timeIntervalSince1970, "topic": s.topic] as [String: Any]
        if let data = try? JSONSerialization.data(withJSONObject: mark) {
            FileManager.default.createFile(atPath: pausedMark(s), contents: data)
        }
    }
}

// 재개: 살아 있는 세션은 그 터미널에 이어서 진행하라는 요청을 넣고, 꺼진 세션은 새 pane 에서 이어 연다
func resumePaused(_ s: Session) {
    ghosttyQueue.async {
        if s.pid > 0, kill(s.pid, 0) == 0, let id = ghosttyTerminalId(s),
           ghosttySend(id, [("text", carryOnPrompt), ("wait", "0.2"), ("key", "enter")]) {
            try? FileManager.default.removeItem(atPath: pausedMark(s))
            return
        }
        DispatchQueue.main.async { resumeSession(s, carryOn: true) }
    }
}

// 중단: 확인을 받은 뒤 세션을 끝낸다. 터미널 세션은 Esc → /exit, 백그라운드 세션은 SIGTERM.
// 대화 기록은 남으므로 종료 세션 메뉴의 "세션 다시 열기"로 이어 열 수 있다
func stopSession(_ s: Session, confirm: Bool = true) {
    if confirm {
    let alert = NSAlert()
    alert.messageText = "세션을 중단할까요?"
    alert.informativeText = "\(s.topic.isEmpty ? s.id : s.topic)\n\n진행 중인 응답을 멈추고 세션을 끝냅니다. 대화 기록은 남아서 나중에 다시 열 수 있습니다."
    alert.addButton(withTitle: "중단")
    alert.addButton(withTitle: "취소")
    alert.buttons.first?.hasDestructiveAction = true
    NSApp.activate(ignoringOtherApps: true)
    guard alert.runModal() == .alertFirstButtonReturn else { return }
    }
    try? FileManager.default.removeItem(atPath: pausedMark(s))
    ghosttyQueue.async {
        if canDrive(s), let id = ghosttyTerminalId(s),
           ghosttySend(id, [("key", "escape"), ("wait", "0.4"), ("text", "/exit"), ("wait", "0.2"), ("key", "enter")]) {
            return
        }
        if s.pid > 0 { kill(s.pid, SIGTERM) }
    }
}

func focusGhostty(_ s: Session) {
    ghosttyQueue.async {
        // 표지를 쓰기 전 제목을 기억해 둔다 — 터미널 id 와 제목을 줄 단위로 받는다
        let list = """
        tell application "Ghostty"
          set out to ""
          repeat with t in terminals
            set out to out & (id of t) & (character id 9) & (name of t) & linefeed
          end repeat
          return out
        end tell
        """
        var before: [String: String] = [:]
        for line in (runScript(list)?.stringValue ?? "").split(separator: "\n") {
            let parts = line.split(separator: "\t", maxSplits: 1, omittingEmptySubsequences: false)
            if parts.count == 2 { before[String(parts[0])] = String(parts[1]) }
        }
        let marker = "CBFOCUS-\(UUID().uuidString.prefix(8))"
        writeTitle(s.tty, marker)
        var found: String?
        for _ in 0..<10 {
            Thread.sleep(forTimeInterval: 0.05)
            found = runScript("""
            tell application "Ghostty"
              repeat with t in terminals
                if name of t is "\(marker)" then
                  focus t
                  activate
                  return id of t
                end if
              end repeat
              return ""
            end tell
            """)?.stringValue
            if let f = found, !f.isEmpty { break }
        }
        if let f = found, let old = before[f] {
            writeTitle(s.tty, old)
        } else {
            // Ghostty 밖(다른 터미널)에서 돌고 있는 세션이다 — 표지 제목만 지우고 앱만 앞으로 가져온다
            writeTitle(s.tty, (s.cwd as NSString).lastPathComponent)
            DispatchQueue.main.async { activateTerminal() }
        }
    }
}

// 대화 기록 위치 — hook 이 알려 준 경로가 없으면 ~/.claude/projects/<작업 경로의 영숫자 외 문자를 - 로 바꾼 이름>/<세션 ID>.jsonl
func transcriptPath(_ given: String?, _ cwd: String, _ id: String) -> String {
    if let g = given, !g.isEmpty { return g }
    let slug = cwd.replacingOccurrences(of: "[^A-Za-z0-9]", with: "-", options: .regularExpression)
    return NSHomeDirectory() + "/.claude/projects/\(slug)/\(id).jsonl"
}

// 끝난 세션을 보드 새 탭에서 같은 작업 경로로 이어 연다. 대화 이력은 claude --resume 이 불러오고,
// 실행 인자·마지막 권한 모드·모델은 state.py 가 기록한 resume_cmd 에 담겨 있다(-p 는 넘기지 않아 대화형으로 열린다)
@MainActor func resumeSession(_ s: Session, carryOn: Bool = false) {
    var cmd = s.resumeCmd.isEmpty ? "claude --resume \(s.id)" : s.resumeCmd
    // 일시중지했던 세션은 첫 요청으로 이어서 진행하라는 문구를 넘겨 바로 다음 작업을 시작하게 한다
    if carryOn {
        cmd += " '" + carryOnPrompt.replacingOccurrences(of: "'", with: "'\\''") + "'"
        try? FileManager.default.removeItem(atPath: pausedMark(s))
    }
    TerminalHub.shared.launch(cmd, in: s.cwd, sessionId: s.id)
}

// 세션을 한 번 눌렀을 때(목록·미니·알림 패널 "세션으로 이동") — 보드 탭이면 펼치고 그 탭으로, 끝난 세션은 보드 새 탭에서 이어 열기,
// 백그라운드 세션은 안내만, 다른 터미널 세션은 지금처럼 그 터미널로(두 번 누르면 그 세션의 터미널 창까지)
@MainActor func openSession(_ s: Session, double: Bool = false) {
    let hub = TerminalHub.shared
    switch hub.place(s) {
    case .tab(let id, _):
        hub.reveal(id)
    case .ended:
        // 끝난 백그라운드 세션은 누르면 아무것도 하지 않는다 — 메뉴의 "세션 다시 열기"로만 보드 탭에서 연다
        if !s.headless { resumeSession(s, carryOn: s.paused) }
    case .background:
        hub.toast("\(s.title) — claude \(hub.mode(s) ?? "-p") 로 실행된 세션이라 터미널이 없습니다. 보드에서는 상태만 보여 주고, 업데이트·종료에도 영향받지 않습니다.")
    case .terminal:
        double ? focusSession(s) : activateOwner(s)
    }
}

// 터미널을 펼친 동안에는 고정하지 않는다 — 화면 전체를 덮은 창이 늘 위에 있으면 다른 앱을 쓸 수 없다. 접으면 다시 고정
@MainActor func applyPin(_ pinned: Bool) {
    window.level = pinned && !TerminalHub.shared.expanded ? .floating : .normal
}

@MainActor func setPinned(_ pinned: Bool) {
    UserDefaults.standard.set(pinned, forKey: "pinned")
    applyPin(pinned)
}

func showBoard() {
    window.makeKeyAndOrderFront(nil)
    NSApp.activate(ignoringOtherApps: true)
}

final class AppDelegate: NSObject, NSApplicationDelegate, NSMenuItemValidation {
    let store = Store()
    let usage = UsageStore()
    let accounts = AccountStore()
    lazy var autoSwitcher = AutoSwitcher(store: store, usage: usage, accounts: accounts)
    var statusItem: NSStatusItem!
    private var statusShown = ""

    func applicationDidFinishLaunching(_ notification: Notification) {
        UserDefaults.standard.register(defaults: ["pinned": true, "mascot": Mascot.tralalero.rawValue])
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: boardLaunchWidth, height: 620),
                          styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
                          backing: .buffered, defer: false)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isMovableByWindowBackground = true
        window.isOpaque = false
        window.backgroundColor = .clear
        window.appearance = NSAppearance(named: .darkAqua)
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        window.hidesOnDeactivate = false
        window.isReleasedWhenClosed = false   // 닫아도 앱은 남고, Dock·메뉴 막대 아이콘으로 다시 연다
        window.title = instanceName == "claude-board" ? "Claude 세션 보드" : "Claude 세션 보드 (시험 빌드)"

        let blur = NSVisualEffectView()
        blur.material = .hudWindow
        blur.blendingMode = .behindWindow
        blur.state = .active
        let host = FirstClickHostingView(rootView: BoardView(store: store, usage: usage, accounts: accounts))
        host.translatesAutoresizingMaskIntoConstraints = false
        // 화면이 바뀔 때마다 SwiftUI 전체의 이상적 크기를 재서 창에 알리지 않는다 — 창 크기·최소 크기는 직접 정한다.
        // 켜 두면 반짝임·회전 애니메이션 프레임마다 창 전체 배치를 다시 계산해 클릭 반응이 늦어진다
        host.sizingOptions = []
        blur.addSubview(host)
        NSLayoutConstraint.activate([
            host.leadingAnchor.constraint(equalTo: blur.leadingAnchor),
            host.trailingAnchor.constraint(equalTo: blur.trailingAnchor),
            host.topAnchor.constraint(equalTo: blur.topAnchor),
            host.bottomAnchor.constraint(equalTo: blur.bottomAnchor),
        ])
        let root = BoardRootView(board: blur)
        window.contentView = root
        TerminalHub.shared.area = root.terminal
        TerminalHub.shared.store = store
        PointerTracker.shared.attach(blur)
        LoginEnvironment.load()
        // 손으로 바꿔도 자동 전환이 전환 시각을 알아야 한다 — 그 뒤 잠시 난 한도 오류는 이전 계정 토큰 탓이다
        accounts.onSwitch = { [usage, autoSwitcher] in
            usage.reset()
            autoSwitcher.switched()
        }
        let nc = NotificationCenter.default
        // 가려지거나 숨은 동안에는 늘 도는 애니메이션 시계를 멈춘다
        // 터미널도 창이 안 보이는 동안(가려짐·최소화·닫힘·다른 데스크톱)은 그리지 않는다
        nc.addObserver(forName: NSWindow.didChangeOcclusionStateNotification, object: window, queue: .main) { _ in
            AmbientTicker.shared.running = window.occlusionState.contains(.visible)
            MainActor.assumeIsolated { TerminalHub.shared.applyVisibility() }
        }
        // 투명 모드: 보드가 활성 창이 되면 불투명, 다른 앱으로 넘어가면 고른 투명도
        for n in [NSWindow.didBecomeKeyNotification, NSWindow.didResignKeyNotification] {
            nc.addObserver(forName: n, object: window, queue: .main) { _ in MainActor.assumeIsolated { WindowOpacity.shared.apply() } }
        }
        // 미니 화면 크기보다 창을 더 줄이지 못하게 한다 — 그보다 크면 창 크기에 따라 전체·중간·미니로 그린다
        // 터미널을 펼쳤으면 터미널 최소 폭과 보드 최소 폭이 들어가는 크기까지
        window.contentMinSize = TerminalHub.shared.expanded ? terminalExpandedMinSize : NSSize(width: miniWidth, height: miniMinHeight)
        window.delegate = sizeGuard   // contentMinSize 만으로는 손으로 줄일 때 막히지 않아 한 번 더 막는다

        // 저장된 창 위치 끝에 macOS 타일 배치 기록({"tilingState":…})이 붙어 있으면 창을 띄우는 순간 macOS 가 그 타일 크기로
        // 되돌린다(2026-10-16 확인 — 미니로 둔 300×144 가 화면 높이로, 폭 454 가 타일 폭 385 로 열렸다). 크기·위치 부분만 남긴다
        let frameKey = "NSWindow Frame ClaudeBoard"
        if let saved = UserDefaults.standard.string(forKey: frameKey), let brace = saved.firstIndex(of: "{") {
            UserDefaults.standard.set(String(saved[..<brace]), forKey: frameKey)
        }
        if !window.setFrameUsingName("ClaudeBoard") { window.center() }
        window.setFrameAutosaveName("ClaudeBoard")   // 저장된 프레임을 다시 적용하므로 폭 조정은 이 뒤에 한다
        // 실행할 때는 가로 위치만 마지막 상태로 두고, 폭은 늘 boardLaunchWidth, 높이는 그 화면의 쓸 수 있는 높이 전체(메뉴 막대·Dock 제외)로 연다
        // (화면 분할 상태가 섞여 전체 폭으로 열리는 것도 막는다). 미니로 두고 껐으면 그 크기·위치 그대로 연다.
        // 터미널을 펼친 채로 껐으면(기본값) 화면의 쓸 수 있는 영역 전체로 연다
        if TerminalHub.shared.expanded, let vf = (window.screen ?? NSScreen.main)?.visibleFrame {
            window.setFrame(vf, display: false)
        } else if !UserDefaults.standard.bool(forKey: "boardMini") {
            var f = window.frame
            f.size.width = boardLaunchWidth
            if let vf = (window.screen ?? NSScreen.main)?.visibleFrame {
                f.origin.y = vf.minY
                f.size.height = vf.height
            }
            window.setFrame(f, display: false)
        }
        addSnapButton()
        addTerminalToggle()
        addMiniTitlebar(store)
        BoardFit.shared.update()   // 창을 띄우기 전에 단계를 정한다 — 미니로 열 때 전체 화면이 한 번 비치지 않게
        // 미니에서 돌아갈 위치는 접힌 보드의 위치만 기억한다
        if BoardFit.shared.tier != .mini, !TerminalHub.shared.expanded { saveRestoreFrame() }
        // 실행 크기를 정한 뒤부터 창 크기를 따라 단계를 고른다
        nc.addObserver(forName: NSWindow.didResizeNotification, object: window, queue: .main) { _ in
            MainActor.assumeIsolated { BoardFit.shared.update() }
        }
        nc.addObserver(forName: NSWindow.didEndLiveResizeNotification, object: window, queue: .main) { _ in
            MainActor.assumeIsolated { if BoardFit.shared.tier != .mini, !TerminalHub.shared.expanded { saveRestoreFrame() } }
        }
        registerHotKeys()
        applyPin(UserDefaults.standard.bool(forKey: "pinned"))
        WindowOpacity.shared.apply()
        buildMainMenu()
        buildStatusItem()
        // 셸 설정에서 --background 로 띄우면 창만 앞에 보이고 포커스는 터미널에 남긴다
        if CommandLine.arguments.contains("--background") { window.orderFrontRegardless() } else { showBoard() }

        DistributedNotificationCenter.default().addObserver(forName: .init("\(instanceName).show"), object: nil,
                                                            queue: .main) { _ in showBoard() }
        DistributedNotificationCenter.default().addObserver(forName: .init("\(instanceName).action"), object: nil,
                                                            queue: .main) { n in
            if let a = n.userInfo?["action"] as? String, let id = n.userInfo?["id"] as? String { MainActor.assumeIsolated { performAction(a, id) } }
        }
        // Ghostty 가 실행되면 보드 창도 함께 띄운다 — 포커스는 Ghostty 에 그대로 둔다
        NSWorkspace.shared.notificationCenter.addObserver(forName: NSWorkspace.didLaunchApplicationNotification,
                                                          object: nil, queue: .main) { note in
            let app = note.userInfo?[NSWorkspace.applicationUserInfoKey] as? NSRunningApplication
            if app?.bundleIdentifier == "com.mitchellh.ghostty" { window.orderFrontRegardless() }
        }
    }

    // 상단 앱 메뉴: 보드 보기 · 항상 위에 고정 · 종료
    func buildMainMenu() {
        let main = NSMenu()
        let appItem = NSMenuItem()
        main.addItem(appItem)
        let appMenu = NSMenu()
        appMenu.addItem(withTitle: "세션 보드 보기", action: #selector(showBoardAction), keyEquivalent: "0")
        appMenu.addItem(withTitle: "항상 위에 고정", action: #selector(togglePin), keyEquivalent: "p")
        // 제목 오른쪽 캐릭터 고르기 — 캐릭터를 오른쪽 클릭해도 같은 목록이 나온다. 체크 표시는 validateMenuItem 이 단다
        let mascotMenu = NSMenu(title: "캐릭터")   // 하위 메뉴를 달면 항목 이름이 하위 메뉴 제목을 따른다
        for m in Mascot.allCases {
            mascotMenu.addItem(withTitle: m.name, action: #selector(pickMascot(_:)), keyEquivalent: "").representedObject = m.rawValue
        }
        appMenu.addItem(withTitle: "캐릭터", action: nil, keyEquivalent: "").submenu = mascotMenu
        // 작업 완료·승인 대기 알림 패널이 뜨는 자리 — 체크 표시는 validateMenuItem 이 단다
        let positionMenu = NSMenu(title: "알림 위치")
        for (value, name) in [("center", "중앙 (기본)"), ("top-left", "왼쪽 상단"), ("top-right", "오른쪽 상단"),
                              ("bottom-left", "왼쪽 하단"), ("bottom-right", "오른쪽 하단")] {
            positionMenu.addItem(withTitle: name, action: #selector(pickNotifyPosition(_:)), keyEquivalent: "").representedObject = value
        }
        appMenu.addItem(withTitle: "알림 위치", action: nil, keyEquivalent: "").submenu = positionMenu
        appMenu.addItem(withTitle: "업데이트 확인…", action: #selector(checkForUpdates), keyEquivalent: "")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Claude Board 가리기", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        appMenu.addItem(withTitle: "Claude Board 종료", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appItem.submenu = appMenu

        let windowItem = NSMenuItem()
        main.addItem(windowItem)
        let windowMenu = NSMenu(title: "윈도우")
        windowMenu.addItem(withTitle: "닫기", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        windowMenu.addItem(withTitle: "최소화", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(.separator())
        // 단축키 표시용 — 실제 동작은 전역 단축키(registerSnapHotKey)가 받는다
        let snap = ActionItem("처음 크기로 오른쪽에 붙이기") { snapRight() }
        snap.keyEquivalent = "b"
        snap.keyEquivalentModifierMask = [.control, .option]
        windowMenu.addItem(snap)
        let term = ActionItem("터미널 펼치기·접기") { toggleTerminal() }
        term.keyEquivalent = "t"
        term.keyEquivalentModifierMask = [.control, .option]
        windowMenu.addItem(term)
        windowItem.submenu = windowMenu
        NSApp.mainMenu = main
    }

    // 상단 메뉴 막대 아이콘: 누르면 보드를 열고 닫는다. 승인 대기·작업 중 세션 수를 함께 보여 준다
    func buildStatusItem() {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = statusItem.button {
            // 공유 이미지의 크기를 바꾸면 보드 제목 옆 아이콘에도 번지므로 복사해서 줄인다
            let icon = appIcon.copy() as! NSImage
            icon.size = NSSize(width: 18, height: 18)
            icon.accessibilityDescription = "Claude Board"
            button.image = icon
            button.toolTip = "Claude Board — 누르면 보드를 열고 닫습니다"   // 보드 밖이라 시스템 툴팁
            button.imagePosition = .imageLeading
            button.action = #selector(toggleBoard)
            button.target = self
        }
        let timer = Timer(timeInterval: 1, repeats: true) { [weak self] _ in self?.updateStatusTitle() }
        RunLoop.main.add(timer, forMode: .common)
        updateStatusTitle()
    }

    func updateStatusTitle() {
        let waiting = store.count("waiting"), working = store.count("working")
        // 매초 같은 제목을 다시 넣으면 메뉴 막대가 매초 다시 그려진다 — 숫자가 바뀔 때만 넣는다
        guard "\(waiting)|\(working)" != statusShown else { return }
        statusShown = "\(waiting)|\(working)"
        let title = NSMutableAttributedString()
        let font = NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .semibold)
        if waiting > 0 {
            title.append(NSAttributedString(string: " \(waiting)", attributes: [
                .font: font, .foregroundColor: NSColor(red: 1.0, green: 0.66, blue: 0.36, alpha: 1)]))
        }
        if working > 0 {
            title.append(NSAttributedString(string: " \(working)", attributes: [
                .font: font, .foregroundColor: NSColor(red: 0.45, green: 0.66, blue: 1.0, alpha: 1)]))
        }
        statusItem.button?.attributedTitle = title
    }

    @objc func toggleBoard() {
        if window.isVisible && NSApp.isActive && window.isKeyWindow {
            window.orderOut(nil)
        } else {
            showBoard()
        }
    }

    @objc func showBoardAction() { showBoard() }

    @objc func togglePin() { MainActor.assumeIsolated { setPinned(!UserDefaults.standard.bool(forKey: "pinned")) } }

    @objc func pickMascot(_ item: NSMenuItem) { UserDefaults.standard.set(item.representedObject as? String, forKey: "mascot") }

    // 중앙(기본)을 고르면 표지 파일을 지운다 — 파일이 없으면 notify.py 가 가운데로 띄운다
    @objc func pickNotifyPosition(_ item: NSMenuItem) {
        guard let value = item.representedObject as? String else { return }
        if value == "center" {
            try? FileManager.default.removeItem(atPath: notifyPositionPath)
        } else {
            try? value.write(toFile: notifyPositionPath, atomically: true, encoding: .utf8)
        }
    }

    // 결과는 보드의 알림 줄에 뜨므로 보드를 먼저 연다
    @objc func checkForUpdates() {
        showBoard()
        Updater.shared.check(manual: true)
    }

    func validateMenuItem(_ item: NSMenuItem) -> Bool {
        if item.action == #selector(togglePin) { item.state = UserDefaults.standard.bool(forKey: "pinned") ? .on : .off }
        if item.action == #selector(pickMascot(_:)) {
            let current = UserDefaults.standard.string(forKey: "mascot") ?? Mascot.tralalero.rawValue
            item.state = item.representedObject as? String == current ? .on : .off
        }
        if item.action == #selector(pickNotifyPosition(_:)) {
            let current = (try? String(contentsOfFile: notifyPositionPath, encoding: .utf8))?
                .trimmingCharacters(in: .whitespacesAndNewlines) ?? "center"
            item.state = item.representedObject as? String == current ? .on : .off
        }
        return true
    }

    // Dock 아이콘을 누르면 닫아 둔 창을 다시 연다
    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows flag: Bool) -> Bool {
        showBoard()
        return true
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { false }
}

// 시험 빌드(build-board.sh --dev, 번들 id com.claude.board.dev)는 설치된 보드와 함께 떠야 하므로 잠금 파일·분산 알림 이름을 따로 쓴다
let instanceName = Bundle.main.bundleIdentifier == "com.claude.board.dev" ? "claude-board-dev" : "claude-board"

// 명령줄 동작: claude-board --action pause|resume|stop|reopen|focus <세션 id>
// 떠 있는 보드에 알림으로 넘긴다 — Ghostty 제어 권한을 가진 그 보드가 실행한다
if let i = CommandLine.arguments.firstIndex(of: "--action"), i + 2 < CommandLine.arguments.count {
    DistributedNotificationCenter.default().postNotificationName(
        .init("\(instanceName).action"), object: nil,
        userInfo: ["action": CommandLine.arguments[i + 1], "id": CommandLine.arguments[i + 2]], deliverImmediately: true)
    exit(0)
}

@MainActor func performAction(_ action: String, _ id: String) {
    // 떠 있는 보드의 Store 를 쓴다 — 새로 만들면 매번 1초 타이머와 알림 관찰자가 남아 계속 돈다
    guard let s = (NSApp.delegate as? AppDelegate)?.store.groups.flatMap(\.sessions).first(where: { $0.id == id }) else { return }
    switch action {
    case "pause": pauseSession(s)
    case "resume": resumePaused(s)
    case "stop": stopSession(s, confirm: false)
    case "reopen": resumeSession(s, carryOn: true)
    case "focus": openSession(s, double: true)   // 보드 탭 세션은 펼치고 그 탭으로, 다른 터미널은 그 세션의 터미널 창으로
    default: break
    }
}

// 이미 떠 있으면 새로 띄우지 않는다
let lockPath = NSTemporaryDirectory() + "\(instanceName).lock"
let lockFd = open(lockPath, O_CREAT | O_RDWR, 0o600)
if lockFd >= 0, flock(lockFd, LOCK_EX | LOCK_NB) != 0 {
    // 이미 떠 있는 창을 앞으로 불러오고 끝낸다
    DistributedNotificationCenter.default().postNotificationName(.init("\(instanceName).show"), object: nil,
                                                                 userInfo: nil, deliverImmediately: true)
    exit(0)
}

// 터미널 엔진의 리소스 번들(terminfo·셸 통합)은 .app 의 Contents/Resources 에 있다 — .app 맨 위는 서명이 거부한다.
// SwiftPM 이 만든 Bundle.module 은 .app 맨 위와 빌드한 컴퓨터의 빌드 경로만 찾으므로(못 찾으면 첫 터미널에서 죽는다,
// 빌드 경로가 남은 개발 Mac 에서는 멀쩡해 보인다), Bundle.module 이 한 번 정해지는 동안만 main 번들의 bundleURL 을
// Contents/Resources 로 돌려준다. 한 번 정해지면 바뀌지 않으므로 첫 TerminalController 전에 한 번 부른다
func resolveResourceBundleInApp() {
    guard Bundle.main.bundlePath.hasSuffix(".app"), let res = Bundle.main.resourceURL else { return }
    let sel = #selector(getter: Bundle.bundleURL)
    guard let m = class_getInstanceMethod(Bundle.self, sel) else { return }
    typealias Fn = @convention(c) (AnyObject, Selector) -> NSURL
    let original = method_getImplementation(m)
    let origFn = unsafeBitCast(original, to: Fn.self)
    let main = Bundle.main
    let block: @convention(block) (AnyObject) -> NSURL = { obj in obj === main ? res as NSURL : origFn(obj, sel) }
    method_setImplementation(m, imp_implementationWithBlock(block))
    _ = GhosttyRuntimeResources.directoryURL   // 여기서 Bundle.module 이 한 번 정해진다
    method_setImplementation(m, original)
}
resolveResourceBundleInApp()

// Claude Code 세션 안에서 보드를 띄우면(open -a·업데이트 단추) 그 세션의 표지 변수(CLAUDECODE·CLAUDE_CODE_CHILD_SESSION·
// CLAUDE_CODE_SESSION_ID 등)가 보드 환경에 그대로 들어온다(open 은 띄운 쪽 환경을 넘긴다, 실측). 보드 탭의 claude·셸이 그것을
// 물려받으면 claude 가 자식 세션으로 여겨 대화 기록을 저장하지 않는다 — 시작할 때 지운다.
// 사용자가 셸 설정에 둔 CLAUDE_* 값은 로그인 셸 환경을 읽을 때(셸 탭은 셸이 뜰 때) 다시 들어온다
func inheritedSessionVariable(_ key: String) -> Bool {
    key.hasPrefix("CLAUDE") || key == "AI_AGENT" || key.hasPrefix("CODEX_COMPANION_")
}
for key in ProcessInfo.processInfo.environment.keys where inheritedSessionVariable(key) { unsetenv(key) }

let app = NSApplication.shared
app.setActivationPolicy(.regular)
let delegate = AppDelegate()
app.delegate = delegate
app.run()
