import AppKit
let p = CGPoint(x: Double(CommandLine.arguments[1])!, y: Double(CommandLine.arguments[2])!)
let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
for w in list {
    guard let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { continue }
    if CGRect(x: b["X"]!, y: b["Y"]!, width: b["Width"]!, height: b["Height"]!).contains(p) {
        print(w[kCGWindowOwnerPID as String]!, w[kCGWindowOwnerName as String] ?? "", "layer", w[kCGWindowLayer as String]!, "alpha", w[kCGWindowAlpha as String] ?? "", "name", w[kCGWindowName as String] ?? "", b)
    }
}
