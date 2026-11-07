// 자율주행 규칙(AutopilotRules) — 보드에 그리는 멈출 선 자리 표(다듬기 ⑬-1 ③, 6단계 ⑫-6 남은 것).
// 엔진들이 기록 저장소에 함께 쓰는 자리 표 둘(stopline_slots·path_claims, 판 3, Multirun.swift)을 보드가 읽기만 해서, 자율주행 중 화면에
// "실행 · 단위 · 도는 중/기다림 · 잡은 %p · 시작한 때" 를 줄마다 보인다. 15분 넘게 다시 적지 않은 자리(apSlotLive 가 아님)는 죽은 엔진의 자리로 따로
// (엔진은 다음 판정·다시 적기에서 그 줄을 지운다). 표는 실행이 둘 이상이거나 기다리는 실행·죽은 엔진의 자리가 있을 때만 보인다(혼자 도는 실행에는 늘 같은 한 줄이라).
// 기다림의 차례는 엔진 apSlotDecide 와 같은 (기다리기 시작한 때, 실행 id) 차례다. 시각 글자는 보드가 그린다(값만 돌려준다)
import Foundation

public enum APSlotRowState: String, Equatable, Sendable {
    case running     // 자리를 잡고 단위가 도는 중(hold, 살아 있음)
    case waiting     // 자리가 없어 기다림(wait, 살아 있음) — 차례가 있다
    case dead        // 죽은 엔진의 자리(15분 넘게 다시 적지 않음, 또는 시각이 앞서 있음)
}

public struct APSlotRow: Equatable, Sendable, Identifiable {
    public var id: String { run }
    public var run: String
    public var unit: Int
    public var state: APSlotRowState
    public var turn: Int?          // 기다림이면 몇 번째(1 부터)
    public var pp: Double          // 잡은 %p(기다림이면 잡으려는 %p)
    public var used: Double        // 그 단위가 쓴 %p
    public var since: Double       // 잡은 때(기다림이면 기다리기 시작한 때)
    public var beat: Double        // 엔진이 마지막으로 적은 때
    public var paths: [String]     // 같은 실행이 잡은 같은 경로 자리(실제 경로)
    public var mine: Bool          // 이 화면이 보는 실행
}

// 멈출 선 자리 없이 같은 경로 자리만 쥔 실행(일시정지·이어받음·출발 중 등)
public struct APClaimRow: Equatable, Sendable, Identifiable {
    public var id: String { run }
    public var run: String
    public var paths: [String]
    public init(run: String, paths: [String]) { self.run = run; self.paths = paths }
}

public struct APSlotTable: Equatable, Sendable {
    public var rows: [APSlotRow] = []
    public var claimsOnly: [APClaimRow] = []
    public var show = false
    public var summary = ""
    public init() {}
}

// 표 한 번 — slots·claims 는 기록 저장소의 줄 그대로, activeRuns 는 보드가 보는 끝나지 않은 실행 수, viewing 은 이 화면의 실행
public func apSlotTable(slots: [APSlot], claims: [APClaim], activeRuns: Int, viewing: String?, now t: Double) -> APSlotTable {
    var paths: [String: [String]] = [:]
    for c in claims.sorted(by: { ($0.run, $0.path) < ($1.run, $1.path) }) where !(paths[c.run] ?? []).contains(c.path) { paths[c.run, default: []].append(c.path) }
    let live = slots.filter { apSlotLive($0, now: t) }
    let holds = live.filter { $0.kind == .hold }.sorted { ($0.since, $0.run) < ($1.since, $1.run) }
    let waits = live.filter { $0.kind == .wait }.sorted { ($0.since, $0.run) < ($1.since, $1.run) }
    let dead = slots.filter { !apSlotLive($0, now: t) }.sorted { ($0.beat, $0.run) < ($1.beat, $1.run) }
    func row(_ s: APSlot, _ st: APSlotRowState, _ turn: Int?) -> APSlotRow {
        APSlotRow(run: s.run, unit: s.unit, state: st, turn: turn, pp: s.pp, used: st == .waiting ? 0 : s.used, since: s.since, beat: s.beat,
                  paths: paths[s.run] ?? [], mine: s.run == viewing)
    }
    var out = APSlotTable()
    out.rows = holds.map { row($0, .running, nil) } + waits.enumerated().map { row($1, .waiting, $0 + 1) } + dead.map { row($0, .dead, nil) }
    let inSlots = Set(slots.map(\.run))
    out.claimsOnly = paths.keys.sorted().filter { !inSlots.contains($0) }.map { APClaimRow(run: $0, paths: paths[$0] ?? []) }
    out.show = activeRuns >= 2 || !waits.isEmpty || !dead.isEmpty
    var parts: [String] = []
    if !holds.isEmpty { parts.append("도는 중 \(holds.count)") }
    if !waits.isEmpty { parts.append("기다림 \(waits.count)") }
    if !dead.isEmpty { parts.append("죽은 엔진의 자리 \(dead.count)") }
    let left = holds.reduce(0) { $0 + apSlotLeft($1) }
    if !holds.isEmpty { parts.append("잡아 두고 아직 쓰지 않은 몫 \(apPPText(left))%p") }
    out.summary = parts.isEmpty ? "잡은 자리 없음" : parts.joined(separator: " · ")
    return out
}

// 줄의 상태 글자(보드 꼬리표)
public func apSlotStateText(_ r: APSlotRow) -> String {
    switch r.state {
    case .running: "도는 중"
    case .waiting: "기다림" + (r.turn.map { " \($0)번째" } ?? "")
    case .dead: "죽은 엔진의 자리"
    }
}

// 줄의 %p 글자 — 도는 중은 잡은 %p 와 쓴 %p, 기다림은 잡으려는 %p, 죽은 엔진의 자리는 남은 몫(엔진이 지울 때까지 다른 실행의 판정에 들지 않음)
public func apSlotPPText(_ r: APSlotRow) -> String {
    switch r.state {
    case .running: "\(apPPText(r.pp))%p 잡음(쓴 \(apPPText(r.used))%p)"
    case .waiting: "\(apPPText(r.pp))%p 기다림"
    case .dead: "\(apPPText(r.pp))%p(쓴 \(apPPText(r.used))%p) · 판정에 들지 않음, 다음 판정이 지움"
    }
}
