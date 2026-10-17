// 검증된 한글 조합 고침 (2026-10-17, 시험 앱 ime-fix 모드에서 실측) — 보드에 옮길 때 참고
import AppKit
import GhosttyTerminal

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
