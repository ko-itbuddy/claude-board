// Ghostty 의 맨 앞 창(레이어 0)을 찍는다: shot <출력.png> [제목에 들어갈 글자]
import AppKit
let out = CommandLine.arguments[1], want = CommandLine.arguments.count > 2 ? CommandLine.arguments[2] : ""
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
let w = list.first { ($0[kCGWindowOwnerName as String] as? String) == "Ghostty" && ($0[kCGWindowLayer as String] as? Int) == 0
    && (want.isEmpty || (($0[kCGWindowName as String] as? String) ?? "").contains(want)) }
guard let n = w?[kCGWindowNumber as String] as? Int else { print("창 없음"); exit(1) }
print("window", n, w?[kCGWindowName as String] ?? "")
let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture"); p.arguments = ["-x", "-o", "-l\(n)", out]
try! p.run(); p.waitUntilExit()
