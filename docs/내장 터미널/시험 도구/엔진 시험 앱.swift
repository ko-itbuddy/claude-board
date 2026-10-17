// 내장 터미널 시험 앱 (버리는 코드) — libghostty-spm 2.2.2026101502
// 사용: Spike <mode> [인자…]   결과는 spike-macos/out/ 에 남는다
import AppKit
import GhosttyKit
import GhosttyTerminal
import GhosttyTheme

// 조합 중 입력기가 글자 확정과 함께 보낸 줄바꿈·취소 명령을 래퍼가 버리는 것을 고친다.
// 입력기가 실제로 그 명령을 보냈을 때만, 원래 키를 수정키 그대로 다시 보낸다(일본어 변환 확정처럼 명령이 없으면 그대로 둔다)
final class IMEFixTerminalView: AppTerminalView {
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
            log("imefix: 수정키 Return mods=\(mods(event).rawValue) — 조합 글자 먼저 확정")
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
        log("imefix: \(NSStringFromSelector(sel)) → \(key) mods=\(mods(event).rawValue)")
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

let base = "/private/tmp/claude-501/-Users-max-mackbook-pro-local-workspace-claude-board/46cf03bc-65b6-46f6-86c6-d810b037f6f0/scratchpad/spike-macos"
let outDir = base + "/out"
let scripts = base + "/scripts"
let argv = CommandLine.arguments
let mode = argv.count > 1 ? argv[1] : "multi"
let tag = ([mode] + argv.dropFirst(2)).joined(separator: "-")
let logPath = "\(outDir)/\(tag).log"
let t0 = Date()
try? FileManager.default.createDirectory(atPath: outDir, withIntermediateDirectories: true)
FileManager.default.createFile(atPath: logPath, contents: Data())
let logHandle = FileHandle(forWritingAtPath: logPath)!

func log(_ s: String) {
    let line = String(format: "[%7.3f] ", Date().timeIntervalSince(t0)) + s + "\n"
    logHandle.write(line.data(using: .utf8)!)
    FileHandle.standardError.write(line.data(using: .utf8)!)
}

func after(_ t: Double, _ f: @escaping @MainActor () -> Void) {
    DispatchQueue.main.asyncAfter(deadline: .now() + t) { MainActor.assumeIsolated { f() } }
}

func sh(_ cmd: String) -> String {
    let p = Process()
    p.executableURL = URL(fileURLWithPath: "/bin/sh")
    p.arguments = ["-c", cmd]
    let pipe = Pipe()
    p.standardOutput = pipe
    p.standardError = pipe
    try? p.run()
    p.waitUntilExit()
    return String(decoding: pipe.fileHandleForReading.readDataToEndOfFile(), as: UTF8.self)
}

func readFile(_ p: String) -> String { (try? String(contentsOfFile: p, encoding: .utf8)) ?? "<없음>" }

// 감시견: 어떤 경우에도 시험 앱이 남지 않게 한다
func watchdog(_ seconds: Double) {
    Thread.detachNewThread {
        Thread.sleep(forTimeInterval: seconds)
        log("watchdog exit")
        exit(0)
    }
}

// TerminalSurface 의 private `surface`(ghostty_surface_t) 를 Mirror 로 꺼낸다 — 시험용 (화면 글자 읽기)
func rawSurface(_ s: TerminalSurface?) -> ghostty_surface_t? {
    guard let s else { return nil }
    for c in Mirror(reflecting: s).children where c.label == "surface" {
        if let p = c.value as? ghostty_surface_t { return p }
    }
    return nil
}

func viewportText(_ s: ghostty_surface_t?) -> String {
    guard let surface = s else { return "<surface 없음>" }
    let size = ghostty_surface_size(surface)
    guard size.columns > 0 else { return "" }
    var lines: [String] = []
    for row in 0 ..< UInt32(size.rows) {
        let sel = ghostty_selection_s(
            top_left: ghostty_point_s(tag: GHOSTTY_POINT_VIEWPORT, coord: GHOSTTY_POINT_COORD_EXACT, x: 0, y: row),
            bottom_right: ghostty_point_s(tag: GHOSTTY_POINT_VIEWPORT, coord: GHOSTTY_POINT_COORD_EXACT, x: UInt32(size.columns) - 1, y: row),
            rectangle: false)
        var out = ghostty_text_s()
        guard ghostty_surface_read_text(surface, sel, &out) else { lines.append("<read 실패>"); continue }
        defer { ghostty_surface_free_text(surface, &out) }
        if let p = out.text, out.text_len > 0 {
            lines.append(String(decoding: UnsafeBufferPointer(start: p, count: Int(out.text_len)).map { UInt8(bitPattern: $0) }, as: UTF8.self))
        } else { lines.append("") }
    }
    return lines.joined(separator: "\n")
}

// 창 캡처 — screencapture -l (화면 기록 권한이 없으면 실패할 수 있음) + cacheDisplay 기반 snapshotImage
@MainActor func capture(_ name: String, _ term: Term?) {
    if let w = window {
        let path = "\(outDir)/\(tag)-\(name).png"
        let r = sh("/usr/sbin/screencapture -x -o -l \(w.windowNumber) '\(path)' 2>&1; ls -la '\(path)' 2>&1")
        log("screencapture \(name): \(r.trimmingCharacters(in: .whitespacesAndNewlines))")
    }
    if let term, let img = term.view.snapshotImage(), let tiff = img.tiffRepresentation,
       let rep = NSBitmapImageRep(data: tiff), let png = rep.representation(using: .png, properties: [:]) {
        let p = "\(outDir)/\(tag)-\(name)-snap.png"
        try? png.write(to: URL(fileURLWithPath: p))
        log("snapshotImage \(name): \(png.count) bytes")
    }
}

// MARK: - 터미널 하나

class Term: NSObject, TerminalSurfaceTitleDelegate, TerminalSurfaceCloseDelegate, TerminalSurfaceLifecycleDelegate,
    TerminalSurfacePwdDelegate, TerminalSurfaceCommandFinishedDelegate, TerminalSurfaceBellDelegate, TerminalSurfaceGridResizeDelegate {
    let name: String
    let view: AppTerminalView
    var surface: TerminalSurface?
    var onAttach: (@MainActor (Term) -> Void)?
    var titleCount = 0
    var lastTitle = ""
    var quietTitles = false
    var attachedAt: Double?
    var cell: (Int, Int) = (0, 0)

    init(_ name: String, _ view: AppTerminalView) {
        self.name = name
        self.view = view
    }

    func terminalDidChangeTitle(_ title: String) {
        titleCount += 1
        lastTitle = title
        if !quietTitles { log("[\(name)] title=\(title.debugDescription)") }
    }
    func terminalDidClose(processAlive: Bool) { log("[\(name)] close processAlive=\(processAlive)") }
    func terminalDidAttachSurface(_ surface: TerminalSurface) {
        self.surface = surface
        attachedAt = Date().timeIntervalSince(t0)
        log("[\(name)] attach surface")
        onAttach?(self)
    }
    func terminalDidDetachSurface() { log("[\(name)] detach surface"); surface = nil }
    func terminalDidChangeWorkingDirectory(_ path: String) { log("[\(name)] pwd=\(path)") }
    func terminalDidFinishCommand(exitCode: Int?, durationNanos: UInt64) {
        log("[\(name)] commandFinished exit=\(exitCode.map(String.init) ?? "nil") dur=\(durationNanos / 1_000_000)ms")
    }
    func terminalDidRingBell() { log("[\(name)] bell") }
    func terminalDidResize(_ size: TerminalGridMetrics) {
        cell = (Int(size.cellWidthPixels), Int(size.cellHeightPixels))
        log("[\(name)] grid \(size.columns)x\(size.rows) cell=\(size.cellWidthPixels)x\(size.cellHeightPixels)px")
    }
    var raw: ghostty_surface_t? { rawSurface(surface) }
}

final class ClipTerm: Term, TerminalSurfaceClipboardConfirmationDelegate {
    func terminalDidRequestClipboardConfirmation(_ request: TerminalClipboardConfirmationRequest) {
        log("[\(name)] clipboardConfirmation kind=\(request.kind) contents=\(request.contents.debugDescription) → allow")
        request.respond(allow: true)
    }
}

// 한글 IME 흉내: interpretKeyEvents 안에서 실제 입력기가 하는 호출(setMarkedText/insertText/doCommand)을 대신한다
final class IMESimView: AppTerminalView {
    var pending: [(IMESimView) -> Void] = []
    override func interpretKeyEvents(_ eventArray: [NSEvent]) {
        if !pending.isEmpty {
            let a = pending.removeFirst()
            a(self)
        } else {
            super.interpretKeyEvents(eventArray)
        }
    }
}

// 실제 입력기 호출을 기록하고 그대로 넘기는 뷰
final class IMELogView: AppTerminalView {
    override func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        log("  IME setMarkedText(\((string as? NSAttributedString)?.string ?? (string as? String) ?? "?"), sel=\(selectedRange))")
        super.setMarkedText(string, selectedRange: selectedRange, replacementRange: replacementRange)
    }
    override func insertText(_ string: Any, replacementRange: NSRange) {
        log("  IME insertText(\((string as? NSAttributedString)?.string ?? (string as? String) ?? "?"))")
        super.insertText(string, replacementRange: replacementRange)
    }
    override func unmarkText() { log("  IME unmarkText"); super.unmarkText() }
    override func doCommand(by selector: Selector) { log("  IME doCommand(\(selector))"); super.doCommand(by: selector) }
}

// MARK: - 창과 공통 도구

let app = NSApplication.shared
app.setActivationPolicy(.regular)
var window: NSWindow?
var terms: [Term] = []
var controller: TerminalController!

@MainActor func makeWindow(_ w: CGFloat = 1000, _ h: CGFloat = 640) -> NSWindow {
    let win = NSWindow(contentRect: NSRect(x: 80, y: 120, width: w, height: h),
                       styleMask: [.titled, .closable, .resizable, .miniaturizable], backing: .buffered, defer: false)
    win.title = "Spike \(tag)"
    win.isReleasedWhenClosed = false
    win.contentView = NSView(frame: NSRect(x: 0, y: 0, width: w, height: h))
    win.makeKeyAndOrderFront(nil)
    window = win
    return win
}

@MainActor @discardableResult
func addTerm(_ name: String, _ opts: TerminalSurfaceOptions, frame: NSRect? = nil, view: AppTerminalView? = nil,
             clip: Bool = false, ctrl: TerminalController? = nil, onAttach: (@MainActor (Term) -> Void)? = nil) -> Term {
    let v = view ?? AppTerminalView(frame: frame ?? window!.contentView!.bounds)
    if view != nil { v.frame = frame ?? window!.contentView!.bounds }
    v.autoresizingMask = [.width, .height]
    let t = clip ? ClipTerm(name, v) : Term(name, v)
    t.onAttach = onAttach
    v.delegate = t
    v.controller = ctrl ?? controller
    v.configuration = opts
    window!.contentView!.addSubview(v)
    terms.append(t)
    return t
}

@MainActor func whenAttached(_ t: Term, _ f: @escaping @MainActor (Term) -> Void) {
    if t.surface != nil { f(t) } else { t.onAttach = f }
}

@MainActor func show(_ t: Term) {
    for o in terms where o !== t { o.view.isHidden = true; o.view.setSurfaceVisible(false) }
    t.view.isHidden = false
    t.view.setSurfaceVisible(true)
    window?.makeFirstResponder(t.view)
}

@MainActor func ttyShort(_ t: Term) -> String { (t.view.ttyName ?? "").replacingOccurrences(of: "/dev/", with: "") }

@MainActor func psTree(_ t: Term) -> String {
    let tty = ttyShort(t)
    guard !tty.isEmpty else { return "<tty 없음>" }
    return sh("ps -o pid=,ppid=,pgid=,tpgid=,tty=,comm= -t \(tty)")
}

func cpuSeconds() -> Double {
    var u = rusage()
    getrusage(RUSAGE_SELF, &u)
    return Double(u.ru_utime.tv_sec) + Double(u.ru_utime.tv_usec) / 1e6 + Double(u.ru_stime.tv_sec) + Double(u.ru_stime.tv_usec) / 1e6
}

func footprintMB() -> Double {
    var info = task_vm_info_data_t()
    var count = mach_msg_type_number_t(MemoryLayout<task_vm_info_data_t>.size / MemoryLayout<integer_t>.size)
    let kr = withUnsafeMutablePointer(to: &info) { $0.withMemoryRebound(to: integer_t.self, capacity: Int(count)) { task_info(mach_task_self_, task_flavor_t(TASK_VM_INFO), $0, &count) } }
    return kr == KERN_SUCCESS ? Double(info.phys_footprint) / 1_048_576 : -1
}

func rssKB(_ pid: Int32) -> Int { Int(sh("ps -o rss= -p \(pid)").trimmingCharacters(in: .whitespacesAndNewlines)) ?? -1 }

@MainActor func keyEvent(_ code: UInt16, _ chars: String, _ ign: String, _ mods: NSEvent.ModifierFlags = [], up: Bool = false) -> NSEvent {
    NSEvent.keyEvent(with: up ? .keyUp : .keyDown, location: .zero, modifierFlags: mods,
                     timestamp: ProcessInfo.processInfo.systemUptime, windowNumber: window!.windowNumber, context: nil,
                     characters: chars, charactersIgnoringModifiers: ign, isARepeat: false, keyCode: code)!
}

@MainActor func hwKey(_ v: AppTerminalView, _ code: UInt16, _ chars: String, _ ign: String, _ mods: NSEvent.ModifierFlags = []) {
    v.keyDown(with: keyEvent(code, chars, ign, mods))
    v.keyUp(with: keyEvent(code, chars, ign, mods, up: true))
}

// 단계별로 시각을 남기며 차례로 실행한다 (분석기가 시각으로 바이트와 맞춘다)
@MainActor func runSteps(_ file: String, _ gap: Double, _ steps: [(String, @MainActor () -> Void)], then: @escaping @MainActor () -> Void) {
    FileManager.default.createFile(atPath: file, contents: Data())
    let h = FileHandle(forWritingAtPath: file)!
    func step(_ i: Int) {
        guard i < steps.count else { then(); return }
        let (label, f) = steps[i]
        let line = "{\"t\": \(String(format: "%.4f", Date().timeIntervalSince1970)), \"step\": \"\(label)\"}\n"
        h.write(line.data(using: .utf8)!)
        f()
        after(gap) { step(i + 1) }
    }
    step(0)
}

let py = "/usr/bin/python3"

// MARK: - .app 안의 리소스 번들
// SwiftPM(CLT) 이 만든 Bundle.module 은 Bundle.main.bundleURL/<이름>.bundle(= .app 맨 위)만 찾고, 없으면 빌드 경로로 간다.
// .app 맨 위에는 서명 때문에 둘 수 없으므로, 처음 한 번 Bundle.module 이 정해지는 동안만 main 번들의 bundleURL 을 Contents/Resources 로 돌려준다
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
    let dir = GhosttyRuntimeResources.directoryURL // 여기서 Bundle.module 이 한 번 정해진다
    method_setImplementation(m, original)
    log("Bundle.main 클래스=\(type(of: main)) 리소스 번들 → \(dir?.path ?? "nil") / 되돌린 bundleURL=\(Bundle.main.bundleURL.path)")
}

// MARK: - 시험들

@MainActor func runMode() {
if ProcessInfo.processInfo.environment["SPIKE_NO_FIX"] == nil { resolveResourceBundleInApp() }
switch mode {

// 터미널 여러 개 + 셸에 첫 입력 + 작업 경로·환경 변수 + 짝짓기(pid·tty)   (항목 3·6)
case "multi":
    let n = argv.count > 2 ? Int(argv[2])! : 5
    watchdog(40)
    controller = TerminalController()
    _ = makeWindow()
    for i in 0 ..< n {
        let dir = "\(base)/work/t\(i)"
        try? FileManager.default.createDirectory(atPath: dir, withIntermediateDirectories: true)
        try? FileManager.default.removeItem(atPath: "\(outDir)/multi-\(i).txt")
        let uuid = UUID().uuidString
        let opts = TerminalSurfaceOptions(backend: .exec, workingDirectory: dir, envVars: ["CLAUDE_BOARD_TERMINAL": uuid])
        let cmd = "printf '%s\\n' \"i=\(i) pwd=$PWD term=$CLAUDE_BOARD_TERMINAL tty=$(tty) pid=$$ ppid=$PPID TERM=$TERM TERMINFO=$TERMINFO GHOSTTY_RESOURCES_DIR=$GHOSTTY_RESOURCES_DIR LANG=$LANG\" > \(outDir)/multi-\(i).txt; infocmp xterm-ghostty >/dev/null 2>&1 && echo terminfo-ok >> \(outDir)/multi-\(i).txt"
        log("t\(i) uuid=\(uuid) cwd=\(dir)")
        addTerm("t\(i)", opts, onAttach: { t in
            // 짝수: 붙자마자(셸이 뜨기 전) / 홀수: 2초 뒤(프롬프트가 뜬 뒤)
            let delay = i % 2 == 0 ? 0.0 : 2.0
            after(delay) {
                let p = t.view.paste(text: cmd)
                let k = t.view.sendKey(.enter)
                log("[\(t.name)] first input after \(delay)s paste=\(p) enter=\(k)")
            }
        })
    }
    // command: 대안 — 셸 대신 명령을 돌리고 끝나면 셸로 남게
    try? FileManager.default.removeItem(atPath: "\(outDir)/multi-cmd.txt")
    let cmdAlt = addTerm("cmd", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
        command: "/bin/zsh -lc 'echo cmd-alt pid=$$ tty=$(tty) pwd=$PWD > \(outDir)/multi-cmd.txt; exec /bin/zsh -l'"))
    // 크기 0 인 뷰 — 접힌 상태에서 만든 터미널이 언제 셸을 띄우는가
    let zero = addTerm("zero", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work"), frame: .zero)
    zero.view.autoresizingMask = []
    show(terms[0])
    after(8) {
        log("--- 8s: 결과 파일과 엔진이 알려 주는 pid·tty 비교")
        for t in terms {
            log("[\(t.name)] attachedAt=\(t.attachedAt.map { String(format: "%.2f", $0) } ?? "nil") ttyName=\(t.view.ttyName ?? "nil") foregroundPid=\(t.view.foregroundPid.map(String.init) ?? "nil") title=\(t.lastTitle.debugDescription)")
        }
        for i in 0 ..< n { log("multi-\(i).txt: " + readFile("\(outDir)/multi-\(i).txt").trimmingCharacters(in: .whitespacesAndNewlines)) }
        log("multi-cmd.txt: " + readFile("\(outDir)/multi-cmd.txt").trimmingCharacters(in: .whitespacesAndNewlines))
        log("ps tree t0:\n" + psTree(terms[0]))
        log("ps tree cmd:\n" + psTree(cmdAlt))
        // 앞에서 도는 프로그램의 pid: sh 가 자기 pid 를 적고 sleep 으로 exec
        for (i, t) in terms.prefix(n).enumerated() {
            try? FileManager.default.removeItem(atPath: "\(outDir)/fg-\(i).txt")
            t.view.paste(text: "sh -c 'echo $$ > \(outDir)/fg-\(i).txt; exec sleep 6'")
            t.view.sendKey(.enter)
        }
        // 크기 0 뷰에 크기를 준다
        zero.view.frame = window!.contentView!.bounds
        log("[zero] frame 을 창 크기로 바꿈")
    }
    after(11) {
        log("--- 11s: foreground 프로그램 pid 비교")
        for (i, t) in terms.prefix(n).enumerated() {
            log("[\(t.name)] foregroundPid=\(t.view.foregroundPid.map(String.init) ?? "nil") fg-\(i).txt=\(readFile("\(outDir)/fg-\(i).txt").trimmingCharacters(in: .whitespacesAndNewlines))")
        }
        log("ps tree t1:\n" + psTree(terms[1]))
        log("[zero] attachedAt=\(zero.attachedAt.map { String(format: "%.2f", $0) } ?? "nil") ttyName=\(zero.view.ttyName ?? "nil")")
        capture("t0", terms[0])
    }
    after(13) { NSApp.terminate(nil) }

// 자원: 터미널 N 개일 때 RSS·놀 때 CPU   (항목 5)  policy: vis = 숨긴 것은 setSurfaceVisible(false)+isHidden, hid = isHidden 만, all = 모두 보임(겹침)
case "res":
    let n = Int(argv[2])!
    let policy = argv.count > 3 ? argv[3] : "vis"
    watchdog(45)
    controller = TerminalController()
    _ = makeWindow()
    let me = getpid()
    log(String(format: "rss before terminals=%dKB footprint=%.1fMB", rssKB(me), footprintMB()))
    for i in 0 ..< n { addTerm("t\(i)", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work")) }
    if n > 0 {
        switch policy {
        case "hid": for t in terms.dropFirst() { t.view.isHidden = true }; window?.makeFirstResponder(terms[0].view)
        case "all": window?.makeFirstResponder(terms[0].view)
        default: show(terms[0])
        }
    }
    var c0 = 0.0
    after(12) { c0 = cpuSeconds(); log(String(format: "rss settled=%dKB footprint=%.1fMB", rssKB(me), footprintMB())) }
    after(32) {
        let c1 = cpuSeconds()
        let kids = sh("for t in $(ps -o tty= -p $(pgrep -P \(me) | tr '\\n' ',' | sed 's/,$//') 2>/dev/null); do :; done; ps -A -o rss=,ppid=,comm= | awk '$2==\(me){s+=$1;c++} END{print s\"KB in \"c\" direct children\"}'")
        let shells = sh("ps -A -o pid=,rss=,tty=,comm= | awk 'NR>0' | grep -E 'ttys' | wc -l")
        log(String(format: "RESULT n=%d policy=%@ footprint=%.1fMB rss=%dKB idleCPU=%.2f%% (%.3fs/20s) children=%@ ttyProcs=%@", n, policy, footprintMB(), rssKB(me),
                   (c1 - c0) / 20 * 100, c1 - c0, kids.trimmingCharacters(in: .whitespacesAndNewlines), shells.trimmingCharacters(in: .whitespacesAndNewlines)))
        NSApp.terminate(nil)
    }

// 숨긴 터미널도 출력을 계속 받는가   (항목 4)  how: none | hidden | invisible | both | remove | orderout
case "hidden":
    let how = argv[2]
    watchdog(45)
    controller = TerminalController()
    _ = makeWindow()
    let streamLog = "\(outDir)/hidden-\(how)-stream.jsonl"
    let a = addTerm("stream", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                     envVars: argv.count > 3 ? ["STREAM_NOTITLE": "1"] : [:],
                                                     command: "\(py) \(scripts)/stream.py \(streamLog) 20"))
    let b = addTerm("shell", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work"))
    a.quietTitles = true
    show(a)
    var titlesAtHide = 0
    after(3) {
        titlesAtHide = a.titleCount
        log("--- 3s: 숨김(\(how)) titleCount=\(a.titleCount)")
        for t in [a, b] {
            switch how {
            case "hidden": t.view.isHidden = true
            case "invisible": t.view.setSurfaceVisible(false)
            case "both": t.view.isHidden = true; t.view.setSurfaceVisible(false)
            case "remove": t.view.removeFromSuperview()
            default: break
            }
        }
        if how == "orderout" { window?.orderOut(nil) }
    }
    var cpuHide = 0.0
    after(5) { cpuHide = cpuSeconds() }
    after(15) {
        let last = viewportText(a.raw).components(separatedBy: "\n").filter { $0.hasPrefix("MARK-") }.last ?? "<없음>"
        log("--- 15s(숨긴 중): titleCount=\(a.titleCount) (숨긴 뒤 \(a.titleCount - titlesAtHide)) 화면 격자의 마지막 줄=\(last.prefix(30))")
        log(String(format: "숨긴 중 CPU 5~15s: %.3fs/10s", cpuSeconds() - cpuHide))
        log("stream 로그 끝: " + sh("tail -3 \(streamLog)").replacingOccurrences(of: "\n", with: " | "))
    }
    after(23) {
        log("--- 23s: 다시 보임 titleCount=\(a.titleCount)")
        if how == "orderout" { window?.makeKeyAndOrderFront(nil) }
        for t in [a, b] {
            if how == "remove" { window!.contentView!.addSubview(t.view) }
            t.view.setSurfaceVisible(true)
        }
        show(a)
    }
    after(25) {
        let text = viewportText(a.raw)
        let marks = text.components(separatedBy: "\n").filter { $0.hasPrefix("MARK-") }
        log("--- 25s: 화면 격자 MARK 줄 수=\(marks.count) 마지막=\(marks.last?.prefix(30) ?? "<없음>") STREAM-DONE=\(text.contains("STREAM-DONE")) titleCount=\(a.titleCount) lastTitle=\(a.lastTitle)")
        log("stream 로그(막힘 0.2s 넘은 것·끝):\n" + sh("grep -v tick \(streamLog)"))
        capture("shown", a)
    }
    after(27) { NSApp.terminate(nil) }

// 키 전달 바이트   (항목 7)  cfg: default | user (사용자 설정에서 theme 줄 뺀 것)
case "keys":
    let cfg = argv.count > 2 ? argv[2] : "default"
    watchdog(90)
    if cfg == "user" {
        let src = readFile(NSHomeDirectory() + "/Library/Application Support/com.mitchellh.ghostty/config.ghostty")
        let p = "\(base)/work/config-notheme.ghostty"
        try? src.components(separatedBy: "\n").filter { !$0.hasPrefix("theme") }.joined(separator: "\n").write(toFile: p, atomically: true, encoding: .utf8)
        controller = TerminalController(configFilePath: p, theme: TerminalTheme())
        log("user config issue=\(controller.lastConfigurationIssue ?? "nil") source=\(controller.currentConfigSource)")
    } else {
        controller = TerminalController()
    }
    _ = makeWindow()
    let variants: [(String, String)] = [("legacy", ""), ("bp", "--bp"), ("kitty1", "--kitty 1 --bp"), ("kitty31", "--kitty 31")]
    func runVariant(_ k: Int) {
        guard k < variants.count else { after(0.5) { NSApp.terminate(nil) }; return }
        let (vname, flags) = variants[k]
        let recFile = "\(outDir)/keys-\(cfg)-\(vname).jsonl"
        try? FileManager.default.removeItem(atPath: recFile)
        for t in terms { t.view.removeFromSuperview() }
        let t = addTerm(vname, TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                      command: "\(py) \(scripts)/rec.py \(recFile) \(flags) --seconds 12"))
        window?.makeFirstResponder(t.view)
        whenAttached(t) { t in
            after(1.5) {
                let v = t.view
                runSteps("\(outDir)/keys-\(cfg)-\(vname)-steps.jsonl", 0.35, [
                    ("api Esc", { v.sendKey(.escape) }),
                    ("api Enter", { v.sendKey(.enter) }),
                    ("api Shift+Enter", { v.sendKey(.enter, modifiers: .shift) }),
                    ("api Ctrl+C", { v.sendKey(.c, modifiers: .ctrl) }),
                    ("api paste abc", { v.paste(text: "abc") }),
                    ("api paste x\\ny", { v.paste(text: "x\ny") }),
                    ("hw Esc", { hwKey(v, 53, "\u{1b}", "\u{1b}") }),
                    ("hw Enter", { hwKey(v, 36, "\r", "\r") }),
                    ("hw Shift+Enter", { hwKey(v, 36, "\r", "\r", .shift) }),
                    ("hw Ctrl+C", { hwKey(v, 8, "\u{3}", "c", .control) }),
                    ("hw a", { hwKey(v, 0, "a", "a") }),
                    ("end", {}),
                ]) { after(0.5) { runVariant(k + 1) } }
            }
        }
    }
    runVariant(0)

// 한글 조합 (항목 8) — IME 호출 흉내
case "ime":
    watchdog(40)
    controller = TerminalController()
    _ = makeWindow()
    let recFile = "\(outDir)/ime-rec.jsonl"
    try? FileManager.default.removeItem(atPath: recFile)
    let v = IMESimView(frame: window!.contentView!.bounds)
    let t = addTerm("ime", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                  command: "\(py) \(scripts)/rec.py \(recFile) --bp --seconds 25"), view: v)
    window?.makeFirstResponder(v)
    let nf = NSRange(location: NSNotFound, length: 0)
    func mark(_ s: String) -> (IMESimView) -> Void { { $0.setMarkedText(s, selectedRange: NSRange(location: (s as NSString).length, length: 0), replacementRange: nf) } }
    func state(_ label: String) { log("\(label): hasMarkedText=\(v.hasMarkedText()) markedRange=\(v.markedRange()) firstRect=\(v.firstRect(forCharacterRange: NSRange(location: 0, length: 0), actualRange: nil))") }
    whenAttached(t) { _ in
        after(1.5) {
            runSteps("\(outDir)/ime-steps.jsonl", 0.6, [
                ("g → ㅎ", { v.pending = [mark("ㅎ")]; hwKey(v, 5, "g", "g"); state("ㅎ") }),
                ("k → 하", { v.pending = [mark("하")]; hwKey(v, 40, "k", "k"); state("하") }),
                ("s → 한", { v.pending = [mark("한")]; hwKey(v, 1, "s", "s"); state("한"); capture("marked-han", t) }),
                ("space → 한+공백 확정", { v.pending = [{ $0.insertText("한", replacementRange: nf); $0.insertText(" ", replacementRange: nf) }]; hwKey(v, 49, " ", " "); state("확정 뒤") }),
                ("r → ㄱ", { v.pending = [mark("ㄱ")]; hwKey(v, 15, "r", "r") }),
                ("k → 가", { v.pending = [mark("가")]; hwKey(v, 40, "k", "k") }),
                ("r → 각", { v.pending = [mark("각")]; hwKey(v, 15, "r", "r"); state("각") }),
                ("Return(insertText 각 + insertNewline)", { v.pending = [{ $0.insertText("각", replacementRange: nf); $0.doCommand(by: #selector(NSResponder.insertNewline(_:))) }]; hwKey(v, 36, "\r", "\r"); state("Return 뒤") }),
                ("r → ㄱ (조합 중 sendKey)", { v.pending = [mark("ㄱ")]; hwKey(v, 15, "r", "r") }),
                ("api sendKey Enter (조합 중)", { v.sendKey(.enter); state("sendKey 뒤") }),
                ("direct setMarkedText 한 (keyDown 밖)", { mark("한")(v); state("direct mark") }),
                ("direct insertText 한 (keyDown 밖)", { v.insertText("한", replacementRange: nf); log("NSApp.currentEvent=\(String(describing: NSApp.currentEvent?.type))"); state("direct insert") }),
                ("end", {}),
            ]) {
                after(1) { capture("after", t); log("화면:\n" + viewportText(t.raw)) }
                after(2) { NSApp.terminate(nil) }
            }
        }
    }

// 실제 한글 입력기(2벌식)를 이 앱의 입력 문맥에만 골라 합성 키를 넣어 본다
case "ime-real":
    watchdog(40)
    controller = TerminalController()
    _ = makeWindow()
    let recFile = "\(outDir)/ime-real-rec.jsonl"
    try? FileManager.default.removeItem(atPath: recFile)
    let v = IMELogView(frame: window!.contentView!.bounds)
    let t = addTerm("ime", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                  command: "\(py) \(scripts)/rec.py \(recFile) --bp --seconds 25"), view: v)
    window?.makeFirstResponder(v)
    NSApp.activate(ignoringOtherApps: true)
    let ctx = v.inputContext
    let before = ctx?.selectedKeyboardInputSource
    log("inputContext=\(ctx != nil) sources=\(ctx?.keyboardInputSources ?? []) selected=\(before ?? "nil")")
    let ko = (ctx?.keyboardInputSources ?? []).first { $0.contains("Korean.2Set") || $0.contains("2SetKorean") }
    whenAttached(t) { _ in
        after(1.5) {
            if let ko { ctx?.selectedKeyboardInputSource = ko }
            log("선택한 입력기=\(ctx?.selectedKeyboardInputSource ?? "nil")")
            func k(_ code: UInt16, _ ch: String) -> @MainActor () -> Void { { hwKey(v, code, ch, ch) } }
            runSteps("\(outDir)/ime-real-steps.jsonl", 0.5, [
                ("g", k(5, "g")), ("k", k(40, "k")), ("s", k(1, "s")), ("space", k(49, " ")),
                ("r", k(15, "r")), ("k", k(40, "k")), ("r", k(15, "r")), ("Return(조합 중)", k(36, "\r")),
                ("g", k(5, "g")), ("k", k(40, "k")), ("s", k(1, "s")), ("m", k(46, "m")), ("f", k(3, "f")), ("Return(조합 중) 2", k(36, "\r")),
                ("end", {}),
            ]) {
                if let before { ctx?.selectedKeyboardInputSource = before }
                log("입력기 되돌림=\(ctx?.selectedKeyboardInputSource ?? "nil")")
                capture("after", t)
                after(1) { NSApp.terminate(nil) }
            }
        }
    }

// 고침을 넣은 한글 입력 시험 — 사용자 설정(theme 뺌, Shift+Enter 키 지정 포함)으로
case "ime-fix":
    let usrc = readFile(NSHomeDirectory() + "/Library/Application Support/com.mitchellh.ghostty/config.ghostty")
    let ucfg = "\(base)/work/config-notheme.ghostty"
    try? usrc.components(separatedBy: "\n").filter { !$0.hasPrefix("theme") }.joined(separator: "\n").write(toFile: ucfg, atomically: true, encoding: .utf8)
    controller = TerminalController(configFilePath: ucfg, theme: TerminalTheme())
    let win = makeWindow(800, 600)
    let recFile = "\(outDir)/ime-fix-rec.jsonl"
    let fixView = IMEFixTerminalView(frame: NSRect(x: 0, y: 0, width: 800, height: 600))
    let l = addTerm("rec", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                  command: "\(py) \(scripts)/rec.py \(recFile) --bp --echo --seconds 600"), view: fixView)
    win.title = "한글 입력 시험(고침)"
    win.makeFirstResponder(l.view)
    NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification, object: win, queue: .main) { _ in NSApp.terminate(nil) }
    watchdog(600)

// 사람이 직접 한글을 쳐 보는 창 — 왼쪽: 받은 바이트를 보여 주는 기록기, 오른쪽: zsh
case "ime-manual":
    controller = TerminalController()
    let win = makeWindow(1200, 700)
    let recFile = "\(outDir)/ime-manual-rec.jsonl"
    let half = NSRect(x: 0, y: 0, width: 600, height: 700)
    let l = addTerm("rec", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                  command: "\(py) \(scripts)/rec.py \(recFile) --bp --echo --seconds 1800"), frame: half)
    l.view.autoresizingMask = [.height]
    let r = addTerm("zsh", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work"), frame: half.offsetBy(dx: 600, dy: 0))
    r.view.autoresizingMask = [.height]
    win.title = "한글 입력 시험 — 왼쪽: 받은 바이트(hex), 오른쪽: zsh. 창을 닫으면 끝"
    win.makeFirstResponder(l.view)
    NotificationCenter.default.addObserver(forName: NSWindow.willCloseNotification, object: win, queue: .main) { _ in NSApp.terminate(nil) }
    watchdog(1800)

// 설정 (항목 9)
case "config":
    watchdog(40)
    controller = TerminalController() // ghostty_init
    let userPath = NSHomeDirectory() + "/Library/Application Support/com.mitchellh.ghostty/config.ghostty"
    let lines = readFile(userPath).components(separatedBy: "\n").filter { !$0.trimmingCharacters(in: .whitespaces).isEmpty }
    func diag(_ path: String) -> [String] {
        let c = ghostty_config_new()!
        ghostty_config_load_file(c, path)
        ghostty_config_finalize(c)
        let n = ghostty_config_diagnostics_count(c)
        let r = (0 ..< n).compactMap { i -> String? in ghostty_config_get_diagnostic(c, i).message.map { String(cString: $0) } }
        ghostty_config_free(c)
        return r
    }
    log("--- 줄마다 따로 불러 진단")
    for (k, line) in lines.enumerated() {
        let p = "\(base)/work/cfg-line-\(k).ghostty"
        try? line.write(toFile: p, atomically: true, encoding: .utf8)
        let d = diag(p)
        log("\(d.isEmpty ? "OK  " : "DIAG") \(line)  \(d.isEmpty ? "" : d.description)")
    }
    log("--- 파일 전체 진단: \(diag(userPath))")
    let c1 = TerminalController(configFilePath: userPath, theme: TerminalTheme())
    log("TerminalController(configFilePath: 사용자 파일) issue=\(c1.lastConfigurationIssue ?? "nil") source=\(c1.currentConfigSource) background=\(c1.backgroundColor)")
    let noTheme = "\(base)/work/config-notheme.ghostty"
    try? lines.filter { !$0.hasPrefix("theme") }.joined(separator: "\n").write(toFile: noTheme, atomically: true, encoding: .utf8)
    let c2 = TerminalController(configFilePath: noTheme, theme: TerminalTheme())
    log("theme 줄 뺀 파일 issue=\(c2.lastConfigurationIssue ?? "nil") source=\(c2.currentConfigSource) background=\(c2.backgroundColor)")
    let def = GhosttyThemeCatalog.theme(named: "Catppuccin Mocha")
    log("GhosttyThemeCatalog Catppuccin Mocha: \(def.map { "bg=\($0.background) fg=\($0.foreground) palette=\($0.palette.count)" } ?? "없음")")
    let c3 = TerminalController(configFilePath: noTheme, theme: def?.toTerminalTheme() ?? TerminalTheme())
    log("theme 줄 뺀 파일 + 카탈로그 테마 issue=\(c3.lastConfigurationIssue ?? "nil") background=\(c3.backgroundColor)")
    log("c3 renderedConfig 앞부분:\n" + c3.renderedConfig.prefix(600))
    let c4 = TerminalController(configFilePath: noTheme) // 기본 theme(.default) 를 그대로 두면?
    log("theme 인자 생략(기본 Afterglow/Alabaster) background=\(c4.backgroundColor)")
    _ = makeWindow(1200, 500)
    let half = NSRect(x: 0, y: 0, width: 600, height: 500)
    let left = addTerm("default", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                         command: "/bin/sh -c 'printf \"default 설정 ABC 한글 →\\\\n\"; sleep 30'"), frame: half, ctrl: controller)
    let right = addTerm("user", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                       command: "/bin/sh -c 'printf \"사용자 설정(테마=카탈로그) ABC 한글 →\\\\n\"; sleep 30'"), frame: half.offsetBy(dx: 600, dy: 0), ctrl: c3)
    left.view.autoresizingMask = []
    right.view.autoresizingMask = []
    after(3) {
        log("cell default=\(left.cell) user=\(right.cell) fontSize default=\(left.view.fontSize.map { "\($0)" } ?? "nil") user=\(right.view.fontSize.map { "\($0)" } ?? "nil")")
        capture("fonts", nil)
    }
    after(4) { NSApp.terminate(nil) }

// 클립보드 (항목 10)  deleg: none | allow
case "clip":
    let deleg = argv.count > 2 ? argv[2] : "none"
    watchdog(40)
    let pb = NSPasteboard.general
    let saved = pb.string(forType: .string)
    defer {}
    controller = TerminalController()
    _ = makeWindow()
    let recFile = "\(outDir)/\(tag).jsonl"
    try? FileManager.default.removeItem(atPath: recFile)
    let t = addTerm("clip", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                   command: "\(py) \(scripts)/rec.py \(recFile) --clip --seconds 20 \(argv.count > 3 ? "--bp" : "")"), clip: deleg == "allow")
    window?.makeFirstResponder(t.view)
    func cmd(_ code: UInt16, _ ch: String) -> Bool {
        t.view.performKeyEquivalent(with: keyEvent(code, ch, ch, .command))
    }
    func restore() {
        pb.clearContents()
        if let saved { pb.setString(saved, forType: .string) }
        log("사용자 클립보드 되돌림 (\(saved?.count ?? 0)자)")
    }
    whenAttached(t) { _ in
        after(1.0) {
            pb.clearContents(); pb.setString("a\nb", forType: .string)
            log("Cmd+V (rec.py 는 bracketed paste 꺼짐, 여러 줄) handled=\(cmd(9, "v"))")
        }
        after(4.0) {
            let sel = t.view.performBindingAction("select_all")
            pb.clearContents(); pb.setString("before-copy", forType: .string)
            let h = cmd(8, "c")
            after(0.3) { log("select_all=\(sel) Cmd+C handled=\(h) → pasteboard=\(pb.string(forType: .string)?.prefix(200).debugDescription ?? "nil") (COPYME 포함=\(pb.string(forType: .string)?.contains("COPYME") ?? false))") }
        }
        after(4.6) { pb.clearContents(); pb.setString("before-osc52", forType: .string) }
        after(6.5) { log("OSC 52 쓰기 뒤 pasteboard=\(pb.string(forType: .string).debugDescription)") }
        after(8.5) {
            pb.clearContents(); pb.setString("one-line", forType: .string)
            log("Cmd+V 한 줄 handled=\(cmd(9, "v"))")
        }
        after(9.5) {
            log("받은 바이트:\n" + sh("cat \(recFile)"))
            restore()
            NSApp.terminate(nil)
        }
    }

// 셸 종료·제목 콜백 (항목 11)
case "exit":
    watchdog(30)
    controller = TerminalController()
    _ = makeWindow()
    let t = addTerm("sh", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work"))
    let w = addTerm("wait", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                   command: "/bin/sh -c 'printf \"\\\\033]2;wait-title\\\\007\"; exit 3'", waitAfterCommand: true))
    let nw = addTerm("nowait", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                      command: "/bin/sh -c 'exit 4'", waitAfterCommand: false))
    let nw2 = addTerm("nowait-slow", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                      command: "/bin/sh -c 'sleep 1; exit 4'", waitAfterCommand: false))
    let nw3 = addTerm("nowait-slow0", TerminalSurfaceOptions(backend: .exec, workingDirectory: "\(base)/work",
                                                      command: "/bin/sh -c 'sleep 1; exit 0'", waitAfterCommand: false))
    after(6.5) {
        log("nowait-slow 화면:\n" + viewportText(nw2.raw).trimmingCharacters(in: .whitespacesAndNewlines))
        log("nowait-slow0 화면:\n" + viewportText(nw3.raw).trimmingCharacters(in: .whitespacesAndNewlines))
        log("ghostty_surface_process_exited: slow=\(nw2.raw.map { ghostty_surface_process_exited($0) } ?? false) slow0=\(nw3.raw.map { ghostty_surface_process_exited($0) } ?? false) nowait=\(nw.raw.map { ghostty_surface_process_exited($0) } ?? false)")
    }
    show(t)
    after(3) {
        log("nowait 화면:\n" + viewportText(nw.raw).trimmingCharacters(in: .whitespacesAndNewlines))
        t.view.paste(text: "printf '\\033]2;spike-title\\007'"); t.view.sendKey(.enter)
    }
    after(4) { t.view.paste(text: "cd /tmp && false"); t.view.sendKey(.enter) }
    after(5.5) {
        log("wait 화면:\n" + viewportText(w.raw).trimmingCharacters(in: .whitespacesAndNewlines))
        t.view.paste(text: "exit"); t.view.sendKey(.enter)
    }
    after(8) {
        log("8s: sh surface=\(t.surface != nil) ttyName=\(t.view.ttyName ?? "nil") wait surface=\(w.surface != nil) nowait surface=\(nw.surface != nil)")
        NSApp.terminate(nil)
    }

default:
    log("알 수 없는 mode \(mode)")
    exit(2)
}
}
MainActor.assumeIsolated { runMode() }

final class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_: NSApplication) -> Bool { mode == "ime-manual" || mode == "ime-fix" }
}
let delegate = AppDelegate()
app.delegate = delegate
app.activate(ignoringOtherApps: true)
app.run()
