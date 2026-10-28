// 사용 한도 자동 전환 판단 시험 — 바깥이 없는 순수 함수라 가짜 어댑터 없이 세션·한도·계정 값만 넣는다.
// 기대값은 AutoSwitcher.tick·switchAway 에 있던 지금 동작 그대로다. 계정·이메일은 가짜다
import Foundation
import Testing
import BoardRules

private let n0 = Date(timeIntervalSince1970: 3_000_000)   // 시험의 지금
private let longAgo: Double = -1000                         // 마지막 전환 — settle(40초)이 한참 지났다

// MARK: - 시험 자료

// 한도에 걸린 세션 — limitAt 은 지금에서 초, nil 이면 걸리지 않음
struct SessionSpec: Sendable, CustomStringConvertible {
    let id: String
    var state = "idle"
    var limitAt: Double? = nil
    var cwd: String? = nil
    var message = "한도 메시지"
    var description: String { id }
}

private func session(_ s: SessionSpec) -> Session {
    Session(id: s.id, cwd: s.cwd ?? "/p/\(s.id)", projectDir: "/p", state: s.state, since: n0, updated: n0, headless: false,
            tty: "", topic: "", message: s.message, resumeCmd: "", file: "", ctx: nil, brain: nil, pid: 0, subagents: [],
            shells: 0, paused: false, muted: false, limitAt: s.limitAt.map { n0 + $0 }, name: nil)
}

// 한도 — resets 는 지금에서 초, nil 이면 모름
struct LimitSpec: Sendable {
    let id: String
    let pct: Double
    var resets: Double? = nil
    init(_ id: String, _ pct: Double, _ resets: Double? = nil) { self.id = id; self.pct = pct; self.resets = resets }
}
private let labels = ["5h": "5시간", "7d": "주간 · 전체", "7d-Fable": "주간 · Fable"]
private func limit(_ l: LimitSpec) -> Limit { Limit(id: l.id, label: labels[l.id] ?? l.id, pct: l.pct, resets: l.resets.map { n0 + $0 }) }

// 계정 — 이름 하나("A")가 id "A|org". expired 면 refreshExpires 가 1초 전
private func account(_ name: String, expired: Bool = false) -> Account {
    Account(oauth: ["accountUuid": name, "organizationUuid": "org", "emailAddress": "\(name.lowercased())@example.com"],
            refreshExpires: expired ? n0 - 1 : n0 + 86400)!
}
private func id(_ name: String) -> String { "\(name)|org" }
private func epoch(_ offset: Double) -> Double { (n0 + offset).timeIntervalSince1970 }

// 판단 결과를 견주기 쉬운 글자로
private func describe(_ move: AutoSwitchMove) -> String {
    switch move {
    case .stay: return "stay"
    case let .switchTo(next, from, reason, stopped):
        return "switch \(from.id.prefix(1))→\(next.id.prefix(1)) [\(reason)]" + (stopped ? " stopped" : "")
    case .noCandidate(let reason): return "none [\(reason)]"
    }
}

// MARK: - 판단 표

struct DecideRow: Row {
    let note: String
    var sessions: [SessionSpec] = []
    var limits: [LimitSpec] = []
    var current: String? = "A"
    var saved = ["A", "B", "C"]
    var expired: [String] = []          // 다시 로그인해야 하는 저장 계정
    var full: [String: Double] = [:]    // 계정 이름 → 표시가 풀리는 시각(지금에서 초)
    var resumed: [String] = []          // 이미 이어 간 "세션 id@한도 시각" — 여기는 세션 id 로 적는다
    var lastSwitch = longAgo            // 마지막 전환(지금에서 초)
    var pending = false
    var switching = false
    let move: String                    // describe 결과
    var marks: [String: Double]? = nil  // 새 한도 표시(계정 이름 → 지금에서 초), nil 이면 그대로
    var resume: [String] = []           // 이어 갈 세션 id(차례대로)
}

private let hitReason = "s1 세션이 한도에 걸림(한도 메시지)"

private let decideRows: [DecideRow] = [
    // 전환할지
    .init(note: "아무 일 없음 → 그대로", sessions: [SessionSpec(id: "s1")], limits: [LimitSpec("5h", 50, 1800)], move: "stay"),
    .init(note: "settle 뒤에 새로 걸린 세션 → 다음 계정으로, 풀림을 몰라 지금 계정은 1시간 표시",
          sessions: [SessionSpec(id: "s1", limitAt: -5)], move: "switch A→B [\(hitReason)] stopped", marks: ["A": 3600]),
    .init(note: "settle 안(전환 10초 뒤)이면 새로 걸린 세션이 있어도 전환하지 않고 이어 가지도 않는다",
          sessions: [SessionSpec(id: "s1", limitAt: -5)], limits: [LimitSpec("5h", 100, 1800)], lastSwitch: -10, move: "stay"),
    .init(note: "settle 이 꼭 끝난 때(지금 = 전환 + 40초)는 아직 안(지나야 한다)",
          sessions: [SessionSpec(id: "s1", limitAt: -1)], lastSwitch: -40, move: "stay"),
    .init(note: "5시간 100%·풀림이 미래 → 전환, 지금 계정은 풀림까지 표시",
          limits: [LimitSpec("5h", 100, 1800)], move: "switch A→B [5시간 한도 100%]", marks: ["A": 1800]),
    .init(note: "5시간 100%·풀림이 지났다 → 그대로", limits: [LimitSpec("5h", 100, -1)], move: "stay"),
    .init(note: "5시간 100%·풀림이 꼭 지금 → 그대로(미래여야 한다)", limits: [LimitSpec("5h", 100, 0)], move: "stay"),
    .init(note: "5시간 100%·풀림 모름 → 미래로 보고 전환, 표시는 1시간",
          limits: [LimitSpec("5h", 100)], move: "switch A→B [5시간 한도 100%]", marks: ["A": 3600]),
    .init(note: "5시간 99.9% → 그대로", limits: [LimitSpec("5h", 99.9, 1800)], move: "stay"),
    .init(note: "주간 120% → 전환, 퍼센트는 정수로", limits: [LimitSpec("5h", 10, 1800), LimitSpec("7d", 120.7, 7200)],
          move: "switch A→B [주간 · 전체 한도 120%]", marks: ["A": 7200]),
    .init(note: "모델별 주간만 100% → 전환하지 않는다(5시간·주간 전체만 본다)", limits: [LimitSpec("7d-Fable", 100, 7200)], move: "stay"),
    .init(note: "표시 풀림 시각은 모델별 주간까지 100% 이상 한도 가운데 가장 늦은 미래 것",
          sessions: [SessionSpec(id: "s1", limitAt: -5)], limits: [LimitSpec("5h", 100, 1800), LimitSpec("7d-Fable", 100, 7200), LimitSpec("7d", 100, -10), LimitSpec("7d", 50, 99999)],
          move: "switch A→B [\(hitReason)] stopped", marks: ["A": 7200]),
    .init(note: "세션과 한도가 함께면 세션 까닭, 세션이 여럿이면 첫 것",
          sessions: [SessionSpec(id: "s1", limitAt: -5), SessionSpec(id: "s2", limitAt: -3)], limits: [LimitSpec("5h", 100, 1800)],
          move: "switch A→B [\(hitReason)] stopped", marks: ["A": 1800]),
    .init(note: "까닭의 폴더 이름은 cwd 의 마지막 조각",
          sessions: [SessionSpec(id: "s1", limitAt: -5, cwd: "/Users/me/일/claude-board/", message: "rate limit")],
          move: "switch A→B [claude-board 세션이 한도에 걸림(rate limit)] stopped", marks: ["A": 3600]),
    .init(note: "전환 대기 중이면 전환하지 않는다", sessions: [SessionSpec(id: "s1", limitAt: -5)], pending: true, move: "stay"),
    .init(note: "전환 중이면 전환하지 않는다", limits: [LimitSpec("5h", 100, 1800)], switching: true, move: "stay"),
    .init(note: "지금 계정이 없으면 전환하지 않는다", sessions: [SessionSpec(id: "s1", limitAt: -5)], current: nil, move: "stay"),
    .init(note: "끝남·일시중지·승인 대기 세션의 한도는 보지 않는다",
          sessions: [SessionSpec(id: "s1", state: "ended", limitAt: -5), SessionSpec(id: "s2", state: "paused", limitAt: -5), SessionSpec(id: "s3", state: "waiting", limitAt: -5)],
          move: "stay"),
    .init(note: "작업 중 세션의 한도는 본다", sessions: [SessionSpec(id: "s1", state: "working", limitAt: -5)],
          move: "switch A→B [\(hitReason)] stopped", marks: ["A": 3600]),
    .init(note: "6시간 넘은 한도는 보지 않는다(꼭 6시간도 넘은 것)",
          sessions: [SessionSpec(id: "s1", limitAt: -6 * 3600), SessionSpec(id: "s2", limitAt: -7 * 3600)], lastSwitch: -8 * 3600, move: "stay"),
    .init(note: "6시간 안이면 본다", sessions: [SessionSpec(id: "s1", limitAt: -6 * 3600 + 1)], lastSwitch: -8 * 3600,
          move: "switch A→B [\(hitReason)] stopped", marks: ["A": 3600]),
    .init(note: "stopped 는 끝나지 않은 한도 세션 모두를 본다 — 승인 대기 세션만 있어도 붙는다",
          sessions: [SessionSpec(id: "s1", state: "waiting", limitAt: -5)], limits: [LimitSpec("5h", 100, 1800)],
          move: "switch A→B [5시간 한도 100%] stopped", marks: ["A": 1800]),
    .init(note: "stopped — 한도 세션이 끝난 것뿐이면 붙지 않는다",
          sessions: [SessionSpec(id: "s1", state: "ended", limitAt: -5)], limits: [LimitSpec("5h", 100, 1800)],
          move: "switch A→B [5시간 한도 100%]", marks: ["A": 1800]),
    .init(note: "stopped — 6시간 넘은 한도 세션도 끝나지 않았으면 붙는다",
          sessions: [SessionSpec(id: "s1", limitAt: -7 * 3600)], limits: [LimitSpec("5h", 100, 1800)], lastSwitch: -8 * 3600,
          move: "switch A→B [5시간 한도 100%] stopped", marks: ["A": 1800]),

    // 다음 계정
    .init(note: "다음 계정 — 지금 계정 다음부터", limits: [LimitSpec("5h", 100, 1800)], current: "B", move: "switch B→C [5시간 한도 100%]", marks: ["B": 1800]),
    .init(note: "다음 계정 — 끝에서 처음으로 돈다", limits: [LimitSpec("5h", 100, 1800)], current: "C", move: "switch C→A [5시간 한도 100%]", marks: ["C": 1800]),
    .init(note: "다음 계정 — 지금 계정이 저장 목록에 없으면 처음부터", limits: [LimitSpec("5h", 100, 1800)], current: "Z",
          move: "switch Z→A [5시간 한도 100%]", marks: ["Z": 1800]),
    .init(note: "다음 계정 — 다시 로그인해야 하는 계정은 뺀다", limits: [LimitSpec("5h", 100, 1800)], expired: ["B"],
          move: "switch A→C [5시간 한도 100%]", marks: ["A": 1800]),
    .init(note: "다음 계정 — 표시가 남은 계정은 뺀다", limits: [LimitSpec("5h", 100, 1800)], full: ["B": 600],
          move: "switch A→C [5시간 한도 100%]", marks: ["A": 1800, "B": 600]),
    .init(note: "다음 계정 — 지난 표시는 치우고 그 계정을 고른다", limits: [LimitSpec("5h", 100, 1800)], full: ["B": -1, "C": 0],
          move: "switch A→B [5시간 한도 100%]", marks: ["A": 1800]),
    .init(note: "다음 계정 — 지금 계정의 남은 표시는 새 값으로 바꾼다", limits: [LimitSpec("5h", 100, 1800)], full: ["A": 99999],
          move: "switch A→B [5시간 한도 100%]", marks: ["A": 1800]),
    .init(note: "후보 없음 — 모두 만료·표시면 바꿀 계정이 없다(표시는 새로 바꾼다)",
          sessions: [SessionSpec(id: "s1", limitAt: -5)], expired: ["B"], full: ["C": 600, "Q": -5],
          move: "none [\(hitReason)]", marks: ["A": 3600, "C": 600]),
    .init(note: "후보 없음 — 저장 계정이 지금 계정뿐", limits: [LimitSpec("5h", 100, 1800)], saved: ["A"],
          move: "none [5시간 한도 100%]", marks: ["A": 1800]),
    .init(note: "후보 없음 — 저장 계정이 없다", limits: [LimitSpec("5h", 100, 1800)], saved: [],
          move: "none [5시간 한도 100%]", marks: ["A": 1800]),
    .init(note: "지금 계정이 만료여도 떠난다(만료는 후보에서만 본다)", limits: [LimitSpec("5h", 100, 1800)], expired: ["A"],
          move: "switch A→B [5시간 한도 100%]", marks: ["A": 1800]),

    // 이어 가기
    .init(note: "이어 가기 — settle 전에 걸린 세션(이전 계정 토큰)", sessions: [SessionSpec(id: "s1", limitAt: -980)],
          move: "stay", resume: ["s1"]),
    .init(note: "이어 가기 — settle 경계(전환 + 40초에 걸림)는 settle 전으로 친다", sessions: [SessionSpec(id: "s1", limitAt: -960)],
          move: "stay", resume: ["s1"]),
    .init(note: "이어 가기 — 이미 이어 간 것은 빼고 차례는 그대로",
          sessions: [SessionSpec(id: "s3", limitAt: -990), SessionSpec(id: "s1", limitAt: -980), SessionSpec(id: "s2", limitAt: -985)], resumed: ["s1"],
          move: "stay", resume: ["s3", "s2"]),
    .init(note: "이어 가기 — 끝남·일시중지·승인 대기는 빼고, 작업 중·대기는 넣는다",
          sessions: [SessionSpec(id: "s1", state: "ended", limitAt: -980), SessionSpec(id: "s2", state: "paused", limitAt: -980),
                     SessionSpec(id: "s3", state: "waiting", limitAt: -980), SessionSpec(id: "s4", state: "working", limitAt: -980), SessionSpec(id: "s5", limitAt: -980)],
          move: "stay", resume: ["s4", "s5"]),
    .init(note: "이어 가기 — 6시간 넘은 것은 뺀다", sessions: [SessionSpec(id: "s1", limitAt: -7 * 3600), SessionSpec(id: "s2", limitAt: -6 * 3600 + 5)],
          lastSwitch: -100, move: "stay", resume: ["s2"]),
    .init(note: "이어 가기 — settle 안이면 하지 않는다", sessions: [SessionSpec(id: "s1", limitAt: -100)], lastSwitch: -30, move: "stay", resume: []),
    .init(note: "이어 가기 — 전환 대기·전환 중·지금 계정 없음과 관계없다", sessions: [SessionSpec(id: "s1", limitAt: -980)],
          current: nil, pending: true, switching: true, move: "stay", resume: ["s1"]),
    .init(note: "이어 가기 — 같은 판단에서 전환과 함께 나온다(새로 걸린 세션은 이어 가지 않는다)",
          sessions: [SessionSpec(id: "old", limitAt: -980), SessionSpec(id: "s1", limitAt: -5)],
          move: "switch A→B [\(hitReason)] stopped", marks: ["A": 3600], resume: ["old"]),
    .init(note: "이어 가기 — 한 번도 전환하지 않았으면(마지막 전환 1970) 모든 한도 세션이 새로 걸린 것",
          sessions: [SessionSpec(id: "s1", limitAt: -500)], lastSwitch: -3_000_000, move: "switch A→B [\(hitReason)] stopped", marks: ["A": 3600]),
]

@Suite("자동 전환: 판단") struct AutoSwitchDecideTests {
    @Test("전환할지·다음 계정과 한도 표시·이어 갈 세션", arguments: decideRows)
    func 표(_ row: DecideRow) {
        let sessions = row.sessions.map(session)
        let resumed = Set(row.sessions.filter { row.resumed.contains($0.id) }.compactMap { s in
            s.limitAt.map { resumeKey(id: s.id, limitAt: n0 + $0) }
        })
        let d = decideAutoSwitch(sessions: sessions, limits: row.limits.map(limit),
                                 current: row.current.map { account($0, expired: row.expired.contains($0)) },
                                 saved: row.saved.map { account($0, expired: row.expired.contains($0)) },
                                 full: Dictionary(uniqueKeysWithValues: row.full.map { (id($0.key), epoch($0.value)) }),
                                 resumed: resumed, lastSwitch: n0 + row.lastSwitch,
                                 pending: row.pending, switching: row.switching, now: n0)
        #expect(describe(d.move) == row.move)
        #expect(d.full == row.marks.map { Dictionary(uniqueKeysWithValues: $0.map { (id($0.key), epoch($0.value)) }) })
        #expect(d.resume.map(\.session.id) == row.resume)
        #expect(d.resume.allSatisfy { r in r.key == resumeKey(id: r.session.id, limitAt: r.session.limitAt!) })
    }

    @Test("이어 가기 표지는 \"세션 id@한도 시각(초)\"") func 표지() {
        #expect(resumeKey(id: "s1", limitAt: Date(timeIntervalSince1970: 2_999_995)) == "s1@2999995.0")
        #expect(resumeKey(id: "s1", limitAt: Date(timeIntervalSince1970: 1.5)) == "s1@1.5")
    }

    @Test("규칙 값 — settle 40초·최근 6시간·다시 볼 때까지 1시간") func 규칙_값() {
        #expect(AutoSwitchRule.settle == 40)
        #expect(AutoSwitchRule.recent == 6 * 3600)
        #expect(AutoSwitchRule.retryAfter == 3600)
    }
}
