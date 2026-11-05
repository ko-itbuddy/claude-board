// 자율주행 규칙(AutopilotRules) — 보고서 창의 콘텐츠 규칙(바깥 요청 막기) 준비 차례(다듬기 ⑬-1 ②, 3단계 남은 것).
// 보드는 보고서 창의 웹 보기에 http(s)·ws(s)·ftp(s) 요청을 막는 콘텐츠 규칙을 붙인다(보고서 글의 CSP·이동 막기와 겹친 받침). 규칙은 WebKit 이
// 뒤에서 컴파일하는데, 전에는 첫 창이 컴파일을 시작하자마자 "해 봤음" 으로 적어 컴파일이 끝나기 전에 연 둘째 창은 규칙 없이 글을 실었고,
// 컴파일이 실패하면 다시 해 보지 않았다. 이 규칙은 창마다 "글을 실어도 되는가" 를 정한다:
//  - 규칙이 준비됐으면 바로 싣는다.
//  - 컴파일 중이면 기다린다(규칙 없이 싣지 않음). 컴파일이 끝나면 기다린 창을 모두 싣는다.
//  - 실패하면 기다린 창에 까닭을 보이고 싣지 않는다(닫힌 쪽, 설계 판단). 다음 창을 열거나 "다시 해 보기" 를 누르면 다시 컴파일한다.
// WebKit 호출·창은 보드(AutopilotReport.swift)가 쥐고, 여기는 값만 다룬다
import Foundation

public enum APRuleGatePhase: Equatable, Sendable {
    case notYet       // 아직 컴파일하지 않음
    case compiling    // 컴파일 중(한 번에 하나)
    case ready        // 규칙이 있음 — 창마다 붙인 뒤 싣는다
    case failed       // 마지막 컴파일이 실패함 — 다음 요청이 다시 컴파일한다
}

// 보드가 할 일
public enum APRuleGateAction: Equatable, Sendable {
    case compile                     // 컴파일을 시작한다(끝나면 compiled 를 부른다)
    case wait(String)                // 이 창은 규칙을 기다린다(싣지 않고 기다림 글)
    case load(String)                // 이 창에 규칙을 붙이고 글을 싣는다
    case refuse(String, String)      // 이 창은 싣지 않는다(창, 까닭) — 까닭 글과 "다시 해 보기"
}

public struct APRuleGate: Equatable, Sendable {
    public private(set) var phase: APRuleGatePhase = .notYet
    public private(set) var waiting: [String] = []   // 규칙을 기다리는 창(연 차례)
    public private(set) var lastError: String?
    public private(set) var tries = 0                  // 컴파일을 시작한 횟수(실패 뒤 다시 한 것 포함)
    public init() {}

    // 창 하나가 글을 싣고 싶다(창을 열 때·같은 실행을 다시 눌러 다시 그릴 때·"다시 해 보기")
    public mutating func want(_ window: String) -> [APRuleGateAction] {
        switch phase {
        case .ready:
            return [.load(window)]
        case .compiling:
            if !waiting.contains(window) { waiting.append(window) }
            return [.wait(window)]
        case .notYet, .failed:
            phase = .compiling
            tries += 1
            if !waiting.contains(window) { waiting.append(window) }
            return [.compile, .wait(window)]
        }
    }

    // 컴파일이 끝났다. 되면 기다린 창을 모두 싣고, 안 되면 기다린 창에 까닭(다음 요청이 다시 컴파일). 컴파일 중이 아닐 때 온 끝(늦게 온 콜백)은 무시
    public mutating func compiled(ok: Bool, error: String?) -> [APRuleGateAction] {
        guard phase == .compiling else { return [] }
        let w = waiting
        waiting = []
        if ok {
            phase = .ready
            lastError = nil
            return w.map { .load($0) }
        }
        phase = .failed
        let why = apRuleGateWhy(error)
        lastError = why
        return w.map { .refuse($0, why) }
    }

    // 창이 닫혔다 — 기다리던 창이면 뺀다(컴파일이 끝나도 싣지 않게)
    public mutating func closed(_ window: String) {
        waiting.removeAll { $0 == window }
    }
}

// 실패 까닭 글(창에 한 줄) — WebKit 오류 글은 상한까지
public func apRuleGateWhy(_ error: String?) -> String {
    let e = (error ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
    let short = e.count > 160 ? String(e.prefix(160)) + "…" : e
    return "바깥 요청 막기 규칙을 만들지 못해 보고서를 싣지 않았습니다" + (short.isEmpty ? "." : "(\(short)).") + " 다시 해 보기를 누르거나 창을 다시 열면 규칙을 다시 만듭니다."
}

// 규칙을 기다리는 동안 창에 보이는 글
public let apRuleGateWaitText = "바깥 요청 막기 규칙을 준비하는 중입니다. 규칙이 붙은 뒤에 보고서를 싣습니다."
