// 시험 빌드 창을 직접 조작한다 — 매번 맨 앞 앱과 초점 창 제목을 확인하고 다르면 멈춘다(사용자 창에 입력이 가지 않게).
// ui <pid> <제목 표지> front                    앱을 앞으로
// ui <pid> <제목 표지> click <x> <y>            화면 좌표(왼쪽 위 원점, 포인트)에서 클릭 — 그 자리의 맨 위 창이 대상 앱 것인지도 확인
// ui <pid> <제목 표지> drag <x1> <y1> <x2> <y2>
// ui <pid> <제목 표지> type <글자>              유니코드 글자를 키 이벤트로 (입력기를 거치지 않게 ABC 에서)
// ui <pid> <제목 표지> key <keycode> [cmd,shift,ctrl,opt]
// ui <pid> <제목 표지> frame                    창 위치·크기(화면 좌표) 출력
import AppKit
import ApplicationServices

let a = CommandLine.arguments
let pid = pid_t(a[1])!, marker = a[2], cmd = a[3]
guard let app = NSRunningApplication(processIdentifier: pid) else { print("대상 앱 없음"); exit(2) }
let src = CGEventSource(stateID: .hidSystemState)

func focusedWindow() -> AXUIElement? {
    var w: CFTypeRef?
    guard AXUIElementCopyAttributeValue(AXUIElementCreateApplication(pid), kAXFocusedWindowAttribute as CFString, &w) == .success else { return nil }
    return (w as! AXUIElement)
}
func title(_ w: AXUIElement?) -> String {
    guard let w else { return "" }
    var t: CFTypeRef?
    AXUIElementCopyAttributeValue(w, kAXTitleAttribute as CFString, &t)
    return t as? String ?? ""
}
func guardFocus(_ at: String) {
    let front = NSWorkspace.shared.frontmostApplication?.processIdentifier
    let t = title(focusedWindow())
    if front != pid || (marker != "*" && !t.contains(marker)) { print("멈춤(\(at)): 맨 앞 pid=\(front ?? -1) 대상=\(pid) 제목=\(t)"); exit(3) }
}
func topWindowPid(at p: CGPoint) -> pid_t? {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    for w in list {   // 앞에서 뒤 순서
        guard let b = w[kCGWindowBounds as String] as? [String: CGFloat], (0...8).contains(w[kCGWindowLayer as String] as? Int ?? 0),
              (w[kCGWindowAlpha as String] as? Double ?? 1) > 0.01 else { continue }
        // 창 크기 조절 가장자리는 창 밖 몇 포인트까지 잡히므로 다른 앱 창은 8포인트 넓혀 본다
        let r = CGRect(x: b["X"]!, y: b["Y"]!, width: b["Width"]!, height: b["Height"]!)
        let mine = (w[kCGWindowOwnerPID as String] as? pid_t) == pid
        if (mine ? r : r.insetBy(dx: -8, dy: -8)).contains(p) { return w[kCGWindowOwnerPID as String] as? pid_t }
    }
    return nil
}
func mouse(_ type: CGEventType, _ p: CGPoint) {
    CGEvent(mouseEventSource: src, mouseType: type, mouseCursorPosition: p, mouseButton: .left)!.post(tap: .cghidEventTap)
}
func flags(_ s: String) -> CGEventFlags {
    var f: CGEventFlags = []
    for m in s.split(separator: ",") {
        switch m { case "cmd": f.insert(.maskCommand); case "shift": f.insert(.maskShift); case "ctrl": f.insert(.maskControl); case "opt": f.insert(.maskAlternate); default: break }
    }
    return f
}

switch cmd {
case "front":
    app.activate()
    Thread.sleep(forTimeInterval: 0.8)
    guardFocus("front")
    print("앞으로 옴")
case "frame":
    var pos: CFTypeRef?, size: CFTypeRef?
    guard let w = focusedWindow() else { print("창 없음"); exit(1) }
    AXUIElementCopyAttributeValue(w, kAXPositionAttribute as CFString, &pos)
    AXUIElementCopyAttributeValue(w, kAXSizeAttribute as CFString, &size)
    var p = CGPoint.zero, s = CGSize.zero
    AXValueGetValue(pos as! AXValue, .cgPoint, &p); AXValueGetValue(size as! AXValue, .cgSize, &s)
    print("frame \(Int(p.x)) \(Int(p.y)) \(Int(s.width)) \(Int(s.height)) title=\(title(w))")
case "click", "drag":
    guardFocus(cmd)
    let p = CGPoint(x: Double(a[4])!, y: Double(a[5])!)
    guard topWindowPid(at: p) == pid else { print("멈춤: 그 자리 맨 위 창이 대상 앱 것이 아님 (\(topWindowPid(at: p) ?? -1))"); exit(3) }
    mouse(.mouseMoved, p); Thread.sleep(forTimeInterval: 0.15)
    mouse(.leftMouseDown, p); Thread.sleep(forTimeInterval: 0.08)
    if cmd == "drag" {
        let q = CGPoint(x: Double(a[6])!, y: Double(a[7])!)
        for i in 1 ... 20 {
            let t = Double(i) / 20
            mouse(.leftMouseDragged, CGPoint(x: p.x + (q.x - p.x) * t, y: p.y + (q.y - p.y) * t)); Thread.sleep(forTimeInterval: 0.02)
        }
        mouse(.leftMouseUp, q)
    } else {
        mouse(.leftMouseUp, p)
    }
    print("\(cmd) 함")
case "type":
    // US ANSI 자판 키 번호로 친다(글자만 실어 보내면 터미널이 키 번호 0 = A 키로 읽는다). 입력기는 ABC 여야 한다
    let plain: [Character: CGKeyCode] = ["a":0,"s":1,"d":2,"f":3,"h":4,"g":5,"z":6,"x":7,"c":8,"v":9,"b":11,"q":12,"w":13,"e":14,"r":15,
        "y":16,"t":17,"1":18,"2":19,"3":20,"4":21,"6":22,"5":23,"=":24,"9":25,"7":26,"-":27,"8":28,"0":29,"]":30,"o":31,"u":32,"[":33,
        "i":34,"p":35,"l":37,"j":38,"'":39,"k":40,";":41,"\\":42,",":43,"/":44,"n":45,"m":46,".":47," ":49,"`":50]
    let shifted: [Character: Character] = ["!":"1","@":"2","#":"3","$":"4","%":"5","^":"6","&":"7","*":"8","(":"9",")":"0","_":"-","+":"=",
        "{":"[","}":"]","|":"\\",":":";","\"":"'","<":",",">":".","?":"/","~":"`"]
    for ch in a[4] {
        guardFocus("type")
        var shift = false
        var base = ch
        if let b = shifted[ch] { base = b; shift = true } else if ch.isUppercase { base = Character(ch.lowercased()); shift = true }
        guard let code = plain[base] else { print("칠 수 없는 글자: \(ch)"); exit(4) }
        if shift { let e = CGEvent(keyboardEventSource: src, virtualKey: 56, keyDown: true)!; e.flags = .maskShift; e.post(tap: .cghidEventTap) }
        for down in [true, false] {
            let e = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: down)!
            e.flags = shift ? .maskShift : []
            e.post(tap: .cghidEventTap)
        }
        if shift { let e = CGEvent(keyboardEventSource: src, virtualKey: 56, keyDown: false)!; e.flags = []; e.post(tap: .cghidEventTap) }
        Thread.sleep(forTimeInterval: 0.012)
    }
    print("입력함")
case "key":
    guardFocus("key")
    let f = a.count > 5 ? flags(a[5]) : []
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: src, virtualKey: CGKeyCode(a[4])!, keyDown: down)!
        e.flags = f
        e.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.03)
    }
    print("키 \(a[4]) 보냄")
case "press":
    // 초점 창(시트 포함)에서 제목이 같은 단추를 손쉬운 사용으로 누른다
    guardFocus("press")
    func find(_ e: AXUIElement, _ depth: Int) -> AXUIElement? {
        var role: CFTypeRef?, t: CFTypeRef?, kids: CFTypeRef?
        AXUIElementCopyAttributeValue(e, kAXRoleAttribute as CFString, &role)
        AXUIElementCopyAttributeValue(e, kAXTitleAttribute as CFString, &t)
        if role as? String == "AXButton", t as? String == a[4] { return e }
        guard depth < 8, AXUIElementCopyAttributeValue(e, kAXChildrenAttribute as CFString, &kids) == .success else { return nil }
        for k in kids as? [AXUIElement] ?? [] { if let f = find(k, depth + 1) { return f } }
        return nil
    }
    guard let w = focusedWindow(), let b = find(w, 0) else { print("단추 없음: \(a[4])"); exit(1) }
    print(AXUIElementPerformAction(b, kAXPressAction as CFString) == .success ? "누름: \(a[4])" : "누르기 실패")
case "move":
    // 대상 앱의 초점 창을 옮긴다(화면 좌표, 왼쪽 위)
    guardFocus("move")
    guard let w = focusedWindow() else { exit(1) }
    var p = CGPoint(x: Double(a[4])!, y: Double(a[5])!)
    let v = AXValueCreate(.cgPoint, &p)!
    print(AXUIElementSetAttributeValue(w, kAXPositionAttribute as CFString, v) == .success ? "옮김" : "옮기기 실패")
case "globalkey":
    // 전역 단축키 시험용 — 맨 앞이 Finder 일 때만 보낸다(다른 앱에 글자가 들어가지 않게)
    guard NSWorkspace.shared.frontmostApplication?.bundleIdentifier == "com.apple.finder" else { print("멈춤: 맨 앞이 Finder 가 아님"); exit(3) }
    let f = a.count > 5 ? flags(a[5]) : []
    for down in [true, false] {
        let e = CGEvent(keyboardEventSource: src, virtualKey: CGKeyCode(a[4])!, keyDown: down)!
        e.flags = f
        e.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.03)
    }
    print("전역 키 \(a[4]) 보냄")
case "presslike":
    // 초점 창에서 제목·설명에 글자가 들어간 단추를 누른다(SwiftUI 단추는 라벨 글자가 설명에 들어 있다)
    guardFocus("presslike")
    func text(_ e: AXUIElement, _ attr: String) -> String { var v: CFTypeRef?; AXUIElementCopyAttributeValue(e, attr as CFString, &v); return v as? String ?? "" }
    var hits: [AXUIElement] = []
    func walk(_ e: AXUIElement, _ depth: Int) {
        var role: CFTypeRef?, kids: CFTypeRef?
        AXUIElementCopyAttributeValue(e, kAXRoleAttribute as CFString, &role)
        if role as? String == "AXButton", (text(e, kAXTitleAttribute) + "|" + text(e, kAXDescriptionAttribute)).contains(a[4]) { hits.append(e) }
        guard depth < 14, AXUIElementCopyAttributeValue(e, kAXChildrenAttribute as CFString, &kids) == .success else { return }
        for k in kids as? [AXUIElement] ?? [] { walk(k, depth + 1) }
    }
    guard let w = focusedWindow() else { exit(1) }
    walk(w, 0)
    for h in hits { print("후보:", text(h, kAXTitleAttribute), "/", text(h, kAXDescriptionAttribute)) }
    guard let b = hits.first else { print("단추 없음: \(a[4])"); exit(1) }
    print(AXUIElementPerformAction(b, kAXPressAction as CFString) == .success ? "누름" : "누르기 실패")
case "clickwin":
    // 앞 앱 확인 없이, 그 자리 맨 위 창이 대상 프로세스 것일 때만 클릭(활성화하지 않는 알림 패널용)
    let p = CGPoint(x: Double(a[4])!, y: Double(a[5])!)
    let wins = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    let inside = wins.contains { w in
        guard (w[kCGWindowOwnerPID as String] as? pid_t) == pid, let b = w[kCGWindowBounds as String] as? [String: CGFloat] else { return false }
        return CGRect(x: b["X"]!, y: b["Y"]!, width: b["Width"]!, height: b["Height"]!).insetBy(dx: 6, dy: 6).contains(p)
    }
    guard inside else { print("멈춤: 대상 창 안쪽이 아님"); exit(3) }
    mouse(.mouseMoved, p); Thread.sleep(forTimeInterval: 0.3)
    mouse(.leftMouseDown, p); Thread.sleep(forTimeInterval: 0.08); mouse(.leftMouseUp, p)
    print("clickwin 함")
default: print("알 수 없는 명령")
}
