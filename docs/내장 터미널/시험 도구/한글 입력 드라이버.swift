// 한글 입력기 시험 드라이버 — 시스템 수준(HID) 키 이벤트로 두벌식 입력기를 거쳐 친다.
// 사용: drive <spike|ghostty> <창 제목 표지> <스크린샷 경로>
// 키를 하나 보낼 때마다 맨 앞 앱과 초점 창 제목을 확인하고, 다르면 바로 멈춘다(다른 창에 글자가 들어가지 않게).
// 끝나면(멈춰도) 원래 입력기로 되돌린다.
import AppKit
import ApplicationServices
import Carbon

let args = CommandLine.arguments
let marker = args[2], shot = args[3], only = args.count > 4 ? args[4] : "all"
guard let target = NSRunningApplication(processIdentifier: pid_t(args[1])!) else { print("대상 앱 없음"); exit(2) }
let pid = target.processIdentifier

func sid(_ s: TISInputSource) -> String { unsafeBitCast(TISGetInputSourceProperty(s, kTISPropertyInputSourceID), to: CFString.self) as String }
func source(_ id: String) -> TISInputSource? {
    (TISCreateInputSourceList([kTISPropertyInputSourceID as String: id] as CFDictionary, false).takeRetainedValue() as! [TISInputSource]).first
}
let src = CGEventSource(stateID: .hidSystemState)
let original = TISCopyCurrentKeyboardInputSource().takeRetainedValue()
let korean = source("com.apple.inputmethod.Korean.2SetKorean")!
let abc = source("com.apple.keylayout.ABC")!
func restore() { if isKorean, NSWorkspace.shared.frontmostApplication?.processIdentifier == pid { toggle() }; TISSelectInputSource(original); print("입력기 되돌림:", sid(TISCopyCurrentKeyboardInputSource().takeRetainedValue())) }
// 사람처럼 입력기 전환 단축키(⌃⌥Space)를 한 번 눌러 ABC ↔ 두벌식을 바꾼다 (입력 메뉴에 둘만 켜져 있음)
var isKorean = false
func toggle() {
    guardFocus("입력기 전환")
    let f: CGEventFlags = [.maskControl, .maskAlternate]
    for (c, down) in [(CGKeyCode(59), true), (58, true), (49, true), (49, false), (58, false), (59, false)] {
        let e = CGEvent(keyboardEventSource: src, virtualKey: c, keyDown: down)!
        e.flags = (c == 49 || down) ? f : []
        e.post(tap: .cghidEventTap)
        Thread.sleep(forTimeInterval: 0.03)
    }
    Thread.sleep(forTimeInterval: 0.6)
    isKorean.toggle()
}
func select(_ s: TISInputSource) { if (sid(s) == sid(korean)) != isKorean { toggle() } }

func focusedTitle() -> String {
    let ax = AXUIElementCreateApplication(pid)
    var w: CFTypeRef?, t: CFTypeRef?
    guard AXUIElementCopyAttributeValue(ax, kAXFocusedWindowAttribute as CFString, &w) == .success else { return "" }
    AXUIElementCopyAttributeValue(w as! AXUIElement, kAXTitleAttribute as CFString, &t)
    return t as? String ?? ""
}
func guardFocus(_ where_: String) {
    let front = NSWorkspace.shared.frontmostApplication?.processIdentifier
    let title = focusedTitle()
    if front != pid || !title.contains(marker) {
        print("멈춤(\(where_)): 맨 앞 pid=\(front ?? -1) 대상=\(pid) 제목=\(title)")
        restore()
        exit(3)
    }
}

func key(_ code: CGKeyCode, _ flags: CGEventFlags = [], gap: Double = 0.09) {
    guardFocus("키 \(code)")
    let d = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: true)!
    let u = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: false)!
    d.flags = flags; u.flags = flags
    d.post(tap: .cghidEventTap); u.post(tap: .cghidEventTap)
    Thread.sleep(forTimeInterval: gap)
}
func mod(_ modCode: CGKeyCode, _ flag: CGEventFlags, _ code: CGKeyCode) {
    guardFocus("수정키 \(code)")
    let md = CGEvent(keyboardEventSource: src, virtualKey: modCode, keyDown: true)!; md.flags = flag
    md.post(tap: .cghidEventTap); Thread.sleep(forTimeInterval: 0.05)
    key(code, flag, gap: 0.05)
    let mu = CGEvent(keyboardEventSource: src, virtualKey: modCode, keyDown: false)!; mu.flags = []
    mu.post(tap: .cghidEventTap); Thread.sleep(forTimeInterval: 0.09)
}
// ANSI 자판 키 번호 — 두벌식: ㅎ=g ㅏ=k ㄴ=s ㄱ=r ㅡ=m ㄹ=f ㄷ=e
let K: [Character: CGKeyCode] = ["a": 0, "s": 1, "d": 2, "f": 3, "g": 5, "b": 11, "e": 14, "r": 15, "k": 40, "m": 46,
                                 "1": 18, "2": 19, "3": 20, "4": 21, "5": 23, "6": 22, "7": 26, "8": 28, "9": 25]
let RET: CGKeyCode = 36, SPACE: CGKeyCode = 49, BS: CGKeyCode = 51, ESC: CGKeyCode = 53
func typ(_ s: String, gap: Double = 0.09) { for c in s { key(K[c]!, gap: gap) } }
func wait(_ s: Double = 0.7) { Thread.sleep(forTimeInterval: s) }

func windowNumber() -> Int? {
    let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] ?? []
    return list.first { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && (($0[kCGWindowName as String] as? String) ?? "").contains(marker) }?[kCGWindowNumber as String] as? Int
}

target.activate()
wait(1.0)
guardFocus("시작")
select(korean)
print("입력기:", sid(TISCopyCurrentKeyboardInputSource().takeRetainedValue()))

// 1: "한글" + 스페이스 — 조합 중 화면을 찍는다
typ("1"); typ("gks"); wait(0.5)
if let n = windowNumber() {
    let p = Process(); p.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture"); p.arguments = ["-x", "-o", "-l\(n)", shot]
    try? p.run(); p.waitUntilExit()
}
typ("rmf"); key(SPACE); wait()
if only == "one" { restore(); print("끝(1)"); exit(0) }
// 2: "한" 다음 바로 Return
typ("2"); typ("gks"); key(RET); wait()
// 3: 조합 중 Backspace → "하" 가 남아 스페이스로 확정
typ("3"); typ("gks"); key(BS); key(SPACE); wait()
// 4: 조합 중 Esc
typ("4"); typ("gks"); key(ESC); wait()
// 5: 조합 중 Shift+Enter
typ("5"); typ("gks"); mod(56, .maskShift, RET); wait()
// 6: 조합 중 Ctrl+C
typ("6"); typ("gks"); mod(59, .maskControl, 8); wait()
// 7: 영문으로 바꿔 "ab" 친 뒤 다시 한글로 바꾸자마자 "한" + 스페이스
typ("7"); select(abc); typ("ab"); select(korean); typ("gks"); key(SPACE); wait()
// 8: "한글" 다음 바로 Return
typ("8"); typ("gksrmf"); key(RET); wait()
// 9: 빠르게 "가나다라" + Return (키 사이 15ms)
typ("9"); typ("rkskekfk", gap: 0.015); key(RET, gap: 0.3); wait()
restore()
print("끝")
