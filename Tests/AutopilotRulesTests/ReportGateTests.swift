// 보고서 창 콘텐츠 규칙 준비 차례(Sources/AutopilotRules/ReportGate.swift) 시험 — 다듬기 ⑬-1 ②.
// 줄마다 사건(창이 싣고 싶음 want·컴파일 끝 ok/fail·창 닫힘)을 차례로 넣고, 보드가 할 일을 글자로 이어 견준다.
// 글자 꼴: compile · wait:<창> · load:<창> · refuse:<창>. 고치기 전 동작(첫 창이 시작하자마자 "해 봤음" → 둘째 창은 규칙 없이 load, 실패 뒤 다시 하지 않음)은
// note 에 "전에는" 으로 적었다
import Testing
import AutopilotRules

struct GateRow: Row {
    let note: String
    let events: [String]     // "want:A" · "ok" · "fail" · "close:A"
    let actions: String      // 사건마다 할 일을 " | " 로, 한 사건 안은 "," 로
    let phase: APRuleGatePhase
    var tries = 1
}

private func act(_ a: APRuleGateAction) -> String {
    switch a {
    case .compile: "compile"
    case .wait(let w): "wait:" + w
    case .load(let w): "load:" + w
    case .refuse(let w, _): "refuse:" + w
    }
}

private let gateRows: [GateRow] = [
    .init(note: "첫 창: 컴파일하고 기다린 뒤 싣는다", events: ["want:A", "ok"], actions: "compile,wait:A | load:A", phase: .ready),
    .init(note: "컴파일이 끝나기 전에 연 둘째 창도 기다렸다가 함께 싣는다(전에는 규칙 없이 바로 load:B)",
          events: ["want:A", "want:B", "ok"], actions: "compile,wait:A | wait:B | load:A,load:B", phase: .ready),
    .init(note: "준비된 뒤 연 창은 바로 싣는다(다시 컴파일하지 않음)", events: ["want:A", "ok", "want:B"], actions: "compile,wait:A | load:A | load:B", phase: .ready),
    .init(note: "같은 창을 기다리는 동안 다시 눌러도 한 번만 싣는다", events: ["want:A", "want:A", "ok"], actions: "compile,wait:A | wait:A | load:A", phase: .ready),
    .init(note: "실패하면 기다린 창 모두 싣지 않고 까닭(닫힌 쪽)", events: ["want:A", "want:B", "fail"], actions: "compile,wait:A | wait:B | refuse:A,refuse:B", phase: .failed),
    .init(note: "실패 뒤 다음 창을 열면 다시 컴파일한다(전에는 다시 하지 않고 규칙 없이 load)",
          events: ["want:A", "fail", "want:B", "ok"], actions: "compile,wait:A | refuse:A | compile,wait:B | load:B", phase: .ready, tries: 2),
    .init(note: "실패한 창의 다시 해 보기도 같은 길", events: ["want:A", "fail", "want:A", "ok"], actions: "compile,wait:A | refuse:A | compile,wait:A | load:A", phase: .ready, tries: 2),
    .init(note: "두 번 실패해도 셋째 요청이 또 컴파일", events: ["want:A", "fail", "want:A", "fail", "want:A"],
          actions: "compile,wait:A | refuse:A | compile,wait:A | refuse:A | compile,wait:A", phase: .compiling, tries: 3),
    .init(note: "기다리던 창이 닫히면 컴파일이 끝나도 싣지 않는다", events: ["want:A", "want:B", "close:A", "ok"], actions: "compile,wait:A | wait:B | - | load:B", phase: .ready),
    .init(note: "컴파일 중이 아닐 때 온 끝(늦은 콜백)은 무시", events: ["want:A", "ok", "fail"], actions: "compile,wait:A | load:A | -", phase: .ready),
    .init(note: "아무도 기다리지 않을 때 끝나도 준비됨으로", events: ["want:A", "close:A", "ok", "want:B"], actions: "compile,wait:A | - | - | load:B", phase: .ready),
]

@Suite("자율주행 보고서 창 콘텐츠 규칙") struct ReportGateTests {
    @Test("규칙이 붙기 전에는 싣지 않고, 실패하면 다음 요청이 다시 만든다", arguments: gateRows)
    func 차례(_ r: GateRow) {
        var g = APRuleGate()
        var out: [String] = []
        for e in r.events {
            if e.hasPrefix("want:") { out.append(g.want(String(e.dropFirst(5))).map(act).joined(separator: ",")) }
            else if e.hasPrefix("close:") { g.closed(String(e.dropFirst(6))); out.append("-") }
            else {
                let a = g.compiled(ok: e == "ok", error: e == "ok" ? nil : "toy compile error").map(act).joined(separator: ",")
                out.append(a.isEmpty ? "-" : a)
            }
        }
        #expect(out.joined(separator: " | ") == r.actions)
        #expect(g.phase == r.phase)
        #expect(g.tries == r.tries)
        #expect(g.waiting.isEmpty || r.phase == .compiling)
    }

    @Test("실패 까닭 글")
    func 까닭() {
        var g = APRuleGate()
        _ = g.want("A")
        let a = g.compiled(ok: false, error: "  Rule list compilation failed  ")
        #expect(a == [.refuse("A", "바깥 요청 막기 규칙을 만들지 못해 보고서를 싣지 않았습니다(Rule list compilation failed). 다시 해 보기를 누르거나 창을 다시 열면 규칙을 다시 만듭니다.")])
        #expect(g.lastError?.hasPrefix("바깥 요청 막기 규칙을") == true)
        #expect(apRuleGateWhy(nil) == "바깥 요청 막기 규칙을 만들지 못해 보고서를 싣지 않았습니다. 다시 해 보기를 누르거나 창을 다시 열면 규칙을 다시 만듭니다.")
        #expect(apRuleGateWhy(String(repeating: "x", count: 300)).contains(String(repeating: "x", count: 160) + "…"))
        _ = g.want("A")
        _ = g.compiled(ok: true, error: nil)
        #expect(g.lastError == nil, "다시 된 뒤에는 까닭을 지운다")
    }
}
