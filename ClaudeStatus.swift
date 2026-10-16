// Claude Code 세션 상태 메뉴 막대 앱
// ~/.claude/notifier/sessions/*.json (state.py 가 기록)을 2초마다 읽어 요약을 메뉴 막대에 띄운다
import AppKit

struct Session {
    let id: String
    let project: String
    let cwd: String
    var state: String
    let since: Date
    let updated: Date
    let pid: Int32?
    let headless: Bool
    let message: String
    let topic: String

    static let order = ["waiting": 0, "working": 1, "idle": 2, "ended": 3]
    static let icons = ["working": "🔵", "waiting": "🟠", "idle": "🟢", "ended": "⚫"]
    static let names = ["working": "작업 중", "waiting": "승인·입력 대기", "idle": "대기", "ended": "종료"]
}

final class StatusApp: NSObject, NSApplicationDelegate, NSMenuDelegate {
    let dir = (NSHomeDirectory() as NSString).appendingPathComponent(".claude/notifier/sessions")
    let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
    let menu = NSMenu()
    var sessions: [Session] = []

    func applicationDidFinishLaunching(_ notification: Notification) {
        menu.delegate = self
        item.menu = menu
        refresh()
        let timer = Timer(timeInterval: 2, repeats: true) { [weak self] _ in self?.refresh() }
        RunLoop.main.add(timer, forMode: .common)
    }

    func load() -> [Session] {
        let fm = FileManager.default
        guard let files = try? fm.contentsOfDirectory(atPath: dir) else { return [] }
        var result: [Session] = []
        for name in files where name.hasSuffix(".json") {
            let path = (dir as NSString).appendingPathComponent(name)
            guard let data = fm.contents(atPath: path),
                  let j = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { continue }
            let pid = (j["pid"] as? NSNumber).map { Int32(truncating: $0) }
            var s = Session(id: j["session_id"] as? String ?? name,
                            project: j["project"] as? String ?? "?",
                            cwd: j["cwd"] as? String ?? "",
                            state: j["state"] as? String ?? "idle",
                            since: Date(timeIntervalSince1970: j["since"] as? Double ?? 0),
                            updated: Date(timeIntervalSince1970: j["updated"] as? Double ?? 0),
                            pid: pid,
                            headless: j["headless"] as? Bool ?? false,
                            message: j["last_message"] as? String ?? "",
                            topic: j["topic"] as? String ?? "")
            // SessionEnd 없이 프로세스가 사라진 세션도 종료로 본다
            if s.state != "ended", let p = s.pid, kill(p, 0) != 0, errno == ESRCH { s.state = "ended" }
            // 종료된 지 30분이 지난 세션은 정리한다
            if s.state == "ended", Date().timeIntervalSince(s.updated) > 1800 {
                try? fm.removeItem(atPath: path)
                continue
            }
            result.append(s)
        }
        return result.sorted {
            let a = Session.order[$0.state] ?? 9, b = Session.order[$1.state] ?? 9
            return a != b ? a < b : $0.since > $1.since
        }
    }

    func refresh() {
        sessions = load()
        var parts: [String] = []
        for state in ["working", "waiting", "idle"] {
            let n = sessions.filter { $0.state == state }.count
            if n > 0 { parts.append("\(Session.icons[state]!)\(n)") }
        }
        let text = parts.isEmpty ? "✳︎" : "✳︎ " + parts.joined(separator: " ")
        item.button?.attributedTitle = NSAttributedString(string: text, attributes: [
            .font: NSFont.monospacedDigitSystemFont(ofSize: 13, weight: .medium),
        ])
    }

    func elapsed(_ since: Date) -> String {
        let s = Int(Date().timeIntervalSince(since))
        if s < 60 { return "\(s)초" }
        if s < 3600 { return "\(s / 60)분 \(s % 60)초" }
        return "\(s / 3600)시간 \(s % 3600 / 60)분"
    }

    func menuNeedsUpdate(_ menu: NSMenu) {
        refresh()
        menu.removeAllItems()

        let header = NSMenuItem(title: "Claude Code 세션 \(sessions.filter { $0.state != "ended" }.count)개", action: nil, keyEquivalent: "")
        header.isEnabled = false
        menu.addItem(header)
        menu.addItem(.separator())

        if sessions.isEmpty {
            let empty = NSMenuItem(title: "기록된 세션이 없습니다", action: nil, keyEquivalent: "")
            empty.isEnabled = false
            menu.addItem(empty)
        }

        let time = DateFormatter()
        time.dateFormat = "HH:mm"
        for s in sessions {
            let status: String
            switch s.state {
            case "working": status = "작업 중 · \(elapsed(s.since))째"
            case "waiting": status = "승인·입력 대기 · \(elapsed(s.since))째"
            case "idle": status = "대기 · \(elapsed(s.since)) 전 완료"
            default: status = "종료 · \(time.string(from: s.updated))"
            }
            let title = NSMutableAttributedString(
                string: "\(Session.icons[s.state] ?? "")  \(s.project)",
                attributes: [.font: NSFont.systemFont(ofSize: 14, weight: .semibold)])
            if !s.topic.isEmpty {
                title.append(NSAttributedString(string: "  \(s.topic)", attributes: [
                    .font: NSFont.systemFont(ofSize: 12),
                    .foregroundColor: NSColor.labelColor,
                ]))
            }
            if s.headless {
                title.append(NSAttributedString(string: "  백그라운드", attributes: [
                    .font: NSFont.systemFont(ofSize: 11, weight: .medium),
                    .foregroundColor: NSColor.secondaryLabelColor,
                ]))
            }
            title.append(NSAttributedString(string: "\n      \(status)", attributes: [
                .font: NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .regular),
                .foregroundColor: NSColor.secondaryLabelColor,
            ]))
            if !s.message.isEmpty, s.state != "ended" {
                let msg = s.message.count > 60 ? String(s.message.prefix(60)) + "…" : s.message
                title.append(NSAttributedString(string: "\n      \(msg)", attributes: [
                    .font: NSFont.systemFont(ofSize: 11),
                    .foregroundColor: NSColor.tertiaryLabelColor,
                ]))
            }
            let mi = NSMenuItem(title: s.project, action: #selector(openTerminal), keyEquivalent: "")
            mi.attributedTitle = title
            mi.target = self
            mi.toolTip = s.cwd
            menu.addItem(mi)
        }

        menu.addItem(.separator())
        let clean = NSMenuItem(title: "종료된 세션 지우기", action: #selector(clearEnded), keyEquivalent: "")
        clean.target = self
        menu.addItem(clean)
        let quit = NSMenuItem(title: "상태 표시 끝내기", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        menu.addItem(quit)
    }

    @objc func openTerminal() {
        if let url = NSWorkspace.shared.urlForApplication(withBundleIdentifier: "com.mitchellh.ghostty") {
            NSWorkspace.shared.openApplication(at: url, configuration: NSWorkspace.OpenConfiguration())
        }
    }

    @objc func clearEnded() {
        for s in sessions where s.state == "ended" {
            try? FileManager.default.removeItem(atPath: (dir as NSString).appendingPathComponent("\(s.id).json"))
        }
        refresh()
    }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let delegate = StatusApp()
app.delegate = delegate
app.run()
