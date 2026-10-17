// 대조용: 글 입력 칸 하나짜리 창. 0.2초마다 내용과 조합 중 글자를 파일에 쓴다. 60초 뒤 저절로 끝난다
import AppKit
let out = CommandLine.arguments[1]
let app = NSApplication.shared
app.setActivationPolicy(.regular)
let win = NSWindow(contentRect: NSRect(x: 200, y: 200, width: 500, height: 200), styleMask: [.titled, .closable], backing: .buffered, defer: false)
win.title = "IME-CTRL"
let tv = NSTextView(frame: win.contentView!.bounds)
tv.font = .systemFont(ofSize: 20)
win.contentView!.addSubview(tv)
win.makeKeyAndOrderFront(nil)
win.makeFirstResponder(tv)
app.activate(ignoringOtherApps: true)
Timer.scheduledTimer(withTimeInterval: 0.2, repeats: true) { _ in
    let s = tv.string, mr = tv.markedRange()
    let marked = mr.location == NSNotFound || mr.length == 0 ? "" : (s as NSString).substring(with: mr)
    try? "\(s.unicodeScalars.map { String(format: "%04X", $0.value) }.joined(separator: " "))\t\(s)\tmarked=\(marked)\tsource=\(tv.inputContext?.selectedKeyboardInputSource ?? "nil")\n".write(toFile: out, atomically: true, encoding: .utf8)
}
DispatchQueue.main.asyncAfter(deadline: .now() + 60) { exit(0) }
app.run()
