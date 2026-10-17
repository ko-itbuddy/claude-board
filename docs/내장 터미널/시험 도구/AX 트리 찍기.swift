// axdump <pid> [최대 깊이] — 초점 창의 AX 트리(역할·제목·설명·값·도움말·위치)를 찍는다
import AppKit
let a = CommandLine.arguments
let pid = pid_t(a[1])!, maxDepth = a.count > 2 ? Int(a[2])! : 30
func attr(_ e: AXUIElement, _ n: String) -> CFTypeRef? { var v: CFTypeRef?; AXUIElementCopyAttributeValue(e, n as CFString, &v); return v }
func s(_ e: AXUIElement, _ n: String) -> String { (attr(e, n) as? String) ?? "" }
func pos(_ e: AXUIElement) -> String {
    guard let p = attr(e, kAXPositionAttribute), let z = attr(e, kAXSizeAttribute) else { return "" }
    var pt = CGPoint.zero, sz = CGSize.zero
    AXValueGetValue(p as! AXValue, .cgPoint, &pt); AXValueGetValue(z as! AXValue, .cgSize, &sz)
    return "@\(Int(pt.x)),\(Int(pt.y)) \(Int(sz.width))x\(Int(sz.height))"
}
func walk(_ e: AXUIElement, _ d: Int) {
    let line = [s(e, kAXRoleAttribute), s(e, kAXTitleAttribute), s(e, kAXDescriptionAttribute), (attr(e, kAXValueAttribute) as? String) ?? "", s(e, kAXHelpAttribute)].map { $0.replacingOccurrences(of: "\n", with: " ") }
    print(String(repeating: " ", count: d) + line.joined(separator: " | ") + " " + pos(e))
    guard d < maxDepth, let kids = attr(e, kAXChildrenAttribute) as? [AXUIElement] else { return }
    for k in kids { walk(k, d + 1) }
}
let app = AXUIElementCreateApplication(pid)
guard let w = attr(app, kAXFocusedWindowAttribute) else { print("창 없음"); exit(1) }
walk(w as! AXUIElement, 0)
