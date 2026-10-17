// pid 의 창을 찍는다: winshot <pid> <출력.png> [all] — 레이어 0 창 중 가장 큰 것 (all 이면 레이어 무관 목록만 출력)
import AppKit
let pid = Int32(CommandLine.arguments[1])!, out = CommandLine.arguments[2]
let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
let mine = list.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid }
for w in mine {
    let b = w[kCGWindowBounds as String] as? [String: Any] ?? [:]
    print("win", w[kCGWindowNumber as String] ?? "", "layer", w[kCGWindowLayer as String] ?? "", "onscreen", w[kCGWindowIsOnscreen as String] ?? false,
          "name", w[kCGWindowName as String] ?? "", "bounds", b["X"] ?? "", b["Y"] ?? "", b["Width"] ?? "", b["Height"] ?? "")
}
if CommandLine.arguments.count > 3 { exit(0) }
let cand = mine.filter { ($0[kCGWindowIsOnscreen as String] as? Bool) == true }.max { a, b in
    let A = a[kCGWindowBounds as String] as? [String: Any] ?? [:], B = b[kCGWindowBounds as String] as? [String: Any] ?? [:]
    return ((A["Width"] as? Double) ?? 0) * ((A["Height"] as? Double) ?? 0) < ((B["Width"] as? Double) ?? 0) * ((B["Height"] as? Double) ?? 0)
}
guard let n = cand?[kCGWindowNumber as String] as? Int else { print("창 없음"); exit(1) }
let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture"); p.arguments = ["-x", "-o", "-l\(n)", out]
try! p.run(); p.waitUntilExit(); print("찍음", n, out)
