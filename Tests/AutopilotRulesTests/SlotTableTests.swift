// 보드 멈출 선 자리 표(Sources/AutopilotRules/SlotTable.swift) 시험 — 다듬기 ⑬-1 ③.
// 자리 줄은 엔진이 쓰는 꼴(APSlot·APClaim) 그대로 넣고, 줄 차례·상태·기다림 차례·보일지·요약을 본다. 시각은 장난감 값(now 10_000)
import Testing
import AutopilotRules

private let T = 10_000.0
private func slot(_ run: String, _ kind: APSlotKind, unit: Int = 1, pp: Double = 5, used: Double = 0, since: Double, beat: Double = T - 30) -> APSlot {
    APSlot(run: run, project: "zz-toy", unit: unit, kind: kind, pp: pp, used: used, pid: 1, since: since, beat: beat)
}
private func claim(_ run: String, _ path: String) -> APClaim {
    APClaim(key: path.lowercased(), path: path, run: run, runDir: "/w/runs/" + run, label: "com.claude.board.dev.autopilot." + run, pid: 1, claimed: T - 100)
}

struct SlotTableRow: Row {
    let note: String
    let slots: [APSlot]
    var claims: [APClaim] = []
    var active = 1
    var viewing: String? = "a"
    let rows: String          // "<실행>:<상태>[#차례]" 를 차례로
    let show: Bool
    let summary: String
}

private let tableRows: [SlotTableRow] = [
    .init(note: "실행 하나·자리 하나는 보이지 않는다(늘 같은 한 줄)", slots: [slot("a", .hold, since: T - 300)], rows: "a:running", show: false,
          summary: "도는 중 1 · 잡아 두고 아직 쓰지 않은 몫 5%p"),
    .init(note: "실행이 둘이면 보인다(자리가 하나여도)", slots: [slot("a", .hold, since: T - 300)], active: 2, rows: "a:running", show: true,
          summary: "도는 중 1 · 잡아 두고 아직 쓰지 않은 몫 5%p"),
    .init(note: "기다리는 실행이 있으면 보인다, 도는 중이 먼저·기다림은 기다린 차례대로",
          slots: [slot("c", .wait, since: T - 50), slot("b", .wait, since: T - 200), slot("a", .hold, used: 1.5, since: T - 300)], active: 3,
          rows: "a:running b:waiting#1 c:waiting#2", show: true, summary: "도는 중 1 · 기다림 2 · 잡아 두고 아직 쓰지 않은 몫 3.5%p"),
    .init(note: "같은 때 기다리기 시작했으면 실행 id 차례(엔진 apSlotDecide 와 같음)",
          slots: [slot("z", .wait, since: T - 60), slot("m", .wait, since: T - 60)], active: 1, viewing: "z",
          rows: "m:waiting#1 z:waiting#2", show: true, summary: "기다림 2"),
    .init(note: "15분 넘게 다시 적지 않은 자리는 죽은 엔진의 자리로 따로(실행이 하나여도 보임)",
          slots: [slot("a", .hold, since: T - 2000, beat: T - 901), slot("b", .hold, since: T - 300)], active: 1, viewing: "b",
          rows: "b:running a:dead", show: true, summary: "도는 중 1 · 죽은 엔진의 자리 1 · 잡아 두고 아직 쓰지 않은 몫 5%p"),
    .init(note: "15분 바로 아래는 아직 산 자리", slots: [slot("a", .hold, since: T - 2000, beat: T - 899)], rows: "a:running", show: false,
          summary: "도는 중 1 · 잡아 두고 아직 쓰지 않은 몫 5%p"),
    .init(note: "시각이 1분 넘게 앞선 자리도 산 것으로 보지 않는다(엔진과 같음)", slots: [slot("a", .wait, since: T, beat: T + 61)], rows: "a:dead", show: true,
          summary: "죽은 엔진의 자리 1"),
    .init(note: "죽은 엔진의 기다림 줄은 차례에 들지 않는다",
          slots: [slot("x", .wait, since: T - 3000, beat: T - 1000), slot("y", .wait, since: T - 100)], active: 1, viewing: "y",
          rows: "y:waiting#1 x:dead", show: true, summary: "기다림 1 · 죽은 엔진의 자리 1"),
    .init(note: "자리 표가 비면 잡은 자리 없음", slots: [], active: 2, rows: "", show: true, summary: "잡은 자리 없음"),
]

@Suite("자율주행 보드 멈출 선 자리 표") struct SlotTableTests {
    @Test("줄 차례·상태·보일지", arguments: tableRows)
    func 표(_ r: SlotTableRow) {
        let t = apSlotTable(slots: r.slots, claims: r.claims, activeRuns: r.active, viewing: r.viewing, now: T)
        let got = t.rows.map { "\($0.run):\($0.state.rawValue)" + ($0.turn.map { "#\($0)" } ?? "") }.joined(separator: " ")
        #expect(got == r.rows)
        #expect(t.show == r.show)
        #expect(t.summary == r.summary)
    }

    @Test("같은 경로 자리를 그 실행 줄에 붙이고, 멈출 선 자리 없는 실행은 따로")
    func 경로() {
        let t = apSlotTable(slots: [slot("a", .hold, since: T - 300)],
                            claims: [claim("a", "/w/toy/beta"), claim("a", "/w/toy/alpha"), claim("p", "/w/toy/gamma"), claim("a", "/w/toy/alpha")],
                            activeRuns: 2, viewing: "a", now: T)
        #expect(t.rows.first?.paths == ["/w/toy/alpha", "/w/toy/beta"])
        #expect(t.rows.first?.mine == true)
        #expect(t.claimsOnly == [APClaimRow(run: "p", paths: ["/w/toy/gamma"])])
    }

    @Test("줄 글자")
    func 글자() {
        let t = apSlotTable(slots: [slot("a", .hold, pp: 5, used: 1.25, since: T - 300), slot("b", .wait, pp: 2.5, since: T - 10),
                                    slot("c", .hold, pp: 4, used: 4, since: T - 3000, beat: T - 2000)], claims: [], activeRuns: 2, viewing: nil, now: T)
        #expect(t.rows.map(apSlotStateText) == ["도는 중", "기다림 1번째", "죽은 엔진의 자리"])
        #expect(t.rows.map(apSlotPPText) == ["5%p 잡음(쓴 1.3%p)", "2.5%p 기다림", "4%p(쓴 4%p) · 판정에 들지 않음, 다음 판정이 지움"])
        #expect(t.rows[1].used == 0, "기다림 줄은 쓴 몫이 없다")
    }
}
