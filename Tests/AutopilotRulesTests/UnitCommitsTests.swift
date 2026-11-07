// 단위 커밋 기록과 승격 뒤 되돌림 신호(Sources/AutopilotRules/UnitCommits.swift) 시험 — 다듬기 ⑬-2 ①③.
// 가짜 git(저장소별 승격 뒤 커밋 목록·병합이 들여온 커밋)으로 되돌림 줄 꼴(보통·병합·되돌림의 되돌림·짧은 이름·다른 저장소의 같은 이름·사람이 고쳐 쓴 메시지)을 본다.
// 기록 저장소 판 5(unit_commits) 올리기, tickets 표 줄, 빠진 단위 보고서도 여기. 값은 장난감(zz-toy·TOY-1·/w/…)
import Foundation
import SQLite3
import Testing
import AutopilotRules

private func sha(_ c: Character, _ tail: String = "") -> String { String(repeating: String(c), count: 40 - tail.count) + tail }

final class FakeRevertGit: APRevertWorld, @unchecked Sendable {
    var logs: [String: [APScannedCommit]] = [:]        // "<repo>|<into>|<from>" → 새것부터
    var merges: [String: [String]] = [:]               // "<repo>|<merge>" → 들여온 커밋
    var calls: [String] = []
    func commits(repo: String, into: String, from: String, max: Int) -> [APScannedCommit]? {
        calls.append("log \(repo) \(into) \(from.prefix(4))")
        return logs["\(repo)|\(into)|\(from)"].map { Array($0.prefix(max)) }
    }
    func introduced(repo: String, merge: String, max: Int) -> [String]? {
        calls.append("introduced \(repo) \(merge.prefix(4))")
        return merges["\(repo)|\(merge)"]
    }
}

@Suite("자율주행 단위 커밋 기록") struct UnitCommitRowsTests {
    @Test("단위 기록의 commits·merge 와 run.json promoted 에서 줄로(앞으로 감기·그대로는 새 커밋이 없어 남기지 않음)")
    func 줄() {
        let e: [String: Any] = ["ended": 200.0, "commits": [["repo": "/w/shop", "branch": "features/TOY-2", "shas": [sha("a"), sha("b"), "zz", sha("a")]]],
                                "merge": ["state": "merged", "how": "merge", "head": sha("c"), "branch": "features/parent/TOY-1", "story": "TOY-1", "repo": "/w/shop", "at": 210.0]]
        let rows = apUnitCommitRows(run: "r1", n: 2, engine: e)
        #expect(rows.map { "\($0.kind.rawValue):\($0.sha.prefix(1)):\($0.ref)" } == ["unit:a:features/TOY-2", "unit:b:features/TOY-2", "story_merge:c:features/parent/TOY-1"])
        #expect(rows[2].source == "story:TOY-1" && rows[2].at == 210 && rows[0].at == 200 && rows.allSatisfy { $0.unitN == 2 })
        // 앞으로 감기로 합침·옛 기록(merge.repo 없음)은 단위 커밋의 저장소가 하나일 때 그것
        var ff = e; ff["merge"] = ["state": "merged", "how": "ff", "head": sha("b")]
        #expect(apUnitCommitRows(run: "r1", n: 2, engine: ff).filter { $0.kind == .storyMerge }.isEmpty)
        var old = e; old["merge"] = ["state": "merged", "how": "merge", "head": sha("c"), "branch": "s", "story": "TOY-1"]
        #expect(apUnitCommitRows(run: "r1", n: 2, engine: old).last?.repo == "/w/shop")
        let promoted: [[String: Any]] = [
            ["source": "autopilot", "repo": "/w/shop", "branch": "autopilot/r1", "into": "dev", "how": "merge", "from": sha("1"), "to": sha("d"), "at": 300.0],
            ["source": "branch:dev", "repo": "/w/shop", "branch": "dev", "into": "live", "how": "merge", "from": sha("2"), "to": sha("e"), "at": 310.0],
            ["source": "autopilot", "repo": "/w/web", "branch": "autopilot/r1", "into": "dev", "how": "ff", "from": sha("3"), "to": sha("a")],
            ["source": "story:TOY-1", "repo": "/w/shop", "into": "dev", "how": "same", "from": sha("4"), "to": sha("4")],
        ]
        #expect(apFinishCommitRows(run: "r1", promoted: promoted).map { "\($0.kind.rawValue):\($0.ref):\($0.source ?? "")" } == ["finish_merge:dev:autopilot", "promote:live:branch:dev"])
        #expect(apFinishCommitRows(run: "r1", promoted: promoted).allSatisfy { $0.unitN == nil })
    }

    @Test("훑을 자리는 승격 기록의 받는 가지를 승격 머리 뒤로만, 꼴이 틀린 줄은 뺀다")
    func 자리() {
        let p: [[String: Any]] = [["repo": "/w/shop", "into": "dev", "to": sha("d")], ["repo": "/w/shop", "into": "dev", "to": sha("d")],
                                  ["repo": "/w/shop", "into": "dev;rm -rf", "to": sha("d")], ["repo": "/w/shop", "into": "--output=/tmp/x", "to": sha("d")],
                                  ["repo": "/w/shop", "into": "a/../b", "to": sha("d")], ["repo": "rel", "into": "dev", "to": sha("d")],
                                  ["repo": "/w/shop", "into": "live", "to": "HEAD"], ["repo": "/w/shop", "into": "live", "to": sha("e")]]
        #expect(apRevertRanges(promoted: p) == [APRevertRange(repo: "/w/shop", into: "dev", from: sha("d")), APRevertRange(repo: "/w/shop", into: "live", from: sha("e"))])
    }
}

@Suite("자율주행 승격 뒤 되돌림 줄 읽기") struct RevertLineTests {
    @Test("되돌림 줄 꼴", arguments: [
        ("Revert \"할 일\"\n\nThis reverts commit \(sha("a")).", [sha("a")]),
        ("Revert \"Merge branch 'autopilot/r1' into dev\"\n\nThis reverts commit \(sha("d")), reversing\nchanges made to \(sha("1")).", [sha("d")]),
        ("되돌림\n\nThis reverts commit abc1234 (할 일 고침, 2026-11-05).", ["abc1234"]),
        ("되돌림\n\n  This reverts commit ABC1234DEF because it broke login", ["abc1234def"]),
        ("this reverts commit abc1234.", ["abc1234"]),
        ("This reverts commit abc123.", []),                                    // 6자는 짧음
        ("This reverts commit \(sha("a"))0.", []),                              // 41자
        ("This reverts commit abc1234x.", []),                                  // 뒤가 이름 글자
        ("This reverts commit ghijklm.", []),
        ("사람이 다시 쓴 메시지: 로그인 고침을 뺌", []),
        ("> This reverts commit abc1234.", []),                                  // 인용 줄은 줄 앞이 아님
        ("Revert two\n\nThis reverts commit abc1234.\nThis reverts commit def5678.\nThis reverts commit abc1234.", ["abc1234", "def5678"]),
    ])
    func 꼴(_ m: String, _ want: [String]) { #expect(apRevertTargets(m) == want) }

    @Test("메시지는 앞 200줄만 본다(믿지 않는 자료)")
    func 상한() {
        let m = String(repeating: "x\n", count: 250) + "This reverts commit abc1234."
        #expect(apRevertTargets(m).isEmpty)
    }
}

@Suite("자율주행 승격 뒤 되돌림 짝짓기") struct RevertScanTests {
    let repo = "/w/shop"
    var recorded: [APUnitCommit] {
        [APUnitCommit(run: "r1", unitN: 1, repo: repo, sha: sha("a"), kind: .unit, ref: "autopilot/r1", at: 1),
         APUnitCommit(run: "r1", unitN: 2, repo: repo, sha: sha("b"), kind: .unit, ref: "autopilot/r1", at: 2),
         APUnitCommit(run: "r1", unitN: 3, repo: repo, sha: sha("f"), kind: .unit, ref: "autopilot/r1", at: 3),
         APUnitCommit(run: "r1", unitN: nil, repo: repo, sha: sha("d"), kind: .finishMerge, ref: "dev", source: "autopilot", at: 4),
         APUnitCommit(run: "r1", unitN: nil, repo: "/w/web", sha: sha("a"), kind: .unit, ref: "autopilot/r1", at: 5)]
    }
    var range: APRevertRange { APRevertRange(repo: repo, into: "dev", from: sha("d")) }

    func run(_ cs: [APScannedCommit], merges: [String: [String]] = [:], recorded rec: [APUnitCommit]? = nil, blocked: [String: String] = [:]) -> (APRevertScan, FakeRevertGit) {
        let g = FakeRevertGit()
        g.logs["\(repo)|dev|\(sha("d"))"] = cs
        for (k, v) in merges { g.merges["\(repo)|\(k)"] = v }
        return (apScanReverts(ranges: [range], recorded: rec ?? recorded, world: g, blocked: blocked), g)
    }

    @Test("보통 되돌림: 단위 커밋 하나를 되돌린 커밋 → 그 단위")
    func 보통() {
        let (s, _) = run([APScannedCommit(sha: sha("9"), message: "Revert \"할 일\"\n\nThis reverts commit \(sha("a")).")])
        #expect(s.hits == [APRevertHit(run: "r1", unitN: 1, repo: repo, target: sha("a"), targetKind: .unit, revert: sha("9"), subject: "Revert \"할 일\"")])
        #expect(s.withdrawn.isEmpty && s.scanned == 1)
    }

    @Test("병합 커밋 되돌림: 그 병합이 들여온 커밋에 든 단위 모두(기록에 없는 커밋은 넘김)")
    func 병합() {
        let (s, g) = run([APScannedCommit(sha: sha("9"), message: "Revert merge\n\nThis reverts commit \(sha("d")), reversing\nchanges made to \(sha("1")).")],
                         merges: [sha("d"): [sha("d"), sha("a"), sha("b"), sha("7")]])
        #expect(s.hits.map { "\($0.unitN):\($0.targetKind.rawValue):\($0.target.prefix(1))" } == ["1:finish_merge:d", "2:finish_merge:d"])
        #expect(g.calls.contains("introduced \(repo) dddd"))
    }

    @Test("되돌림의 되돌림(다시 살림)은 신호를 거두고, 세 번째로 다시 되돌리면 다시 신호")
    func 다시살림() {
        let r1 = APScannedCommit(sha: sha("9"), message: "Revert \"할 일\"\n\nThis reverts commit \(sha("a")).")
        let r2 = APScannedCommit(sha: sha("8"), message: "Reapply \"할 일\"\n\nThis reverts commit \(sha("9")).")
        let (s, _) = run([r2, r1])
        #expect(s.hits.isEmpty && s.withdrawn.map { "\($0.run)/\($0.unitN)/\($0.target.prefix(1))" } == ["r1/1/a"])
        let r3 = APScannedCommit(sha: sha("6"), message: "Revert \"Reapply\"\n\nThis reverts commit \(sha("8")).")
        let (s3, _) = run([r3, r2, r1])
        #expect(s3.hits.map(\.unitN) == [1] && s3.hits[0].revert == sha("9") && s3.withdrawn.isEmpty)
        // 다시 살린 뒤 같은 단위 커밋을 새로 되돌림 — 산 되돌림이 있으면 거두지 않는다
        let r4 = APScannedCommit(sha: sha("5"), message: "Revert again\n\nThis reverts commit \(sha("a")).")
        let (s4, _) = run([r4, r2, r1])
        #expect(s4.hits.map(\.revert) == [sha("5")] && s4.withdrawn.isEmpty)
    }

    @Test("짧은 이름: 하나에만 맞으면 짝짓고, 둘 이상에 맞으면 짝짓지 않는다(닫힌 쪽)")
    func 짧은이름() {
        let (s, _) = run([APScannedCommit(sha: sha("9"), message: "This reverts commit bbbbbbb.")])
        #expect(s.hits.map(\.unitN) == [2] && s.hits[0].target == sha("b"))
        var rec = recorded
        rec.append(APUnitCommit(run: "r2", unitN: 1, repo: repo, sha: String(repeating: "b", count: 12) + String(repeating: "0", count: 28), kind: .unit, ref: "x", at: 9))
        let (s2, _) = run([APScannedCommit(sha: sha("9"), message: "This reverts commit bbbbbbb.")], recorded: rec)
        #expect(s2.hits.isEmpty && s2.notes.contains { $0.contains("여러 커밋") })
        let (s3, _) = run([APScannedCommit(sha: sha("9"), message: "This reverts commit bbbbbbbbbbbb0.")], recorded: rec)   // 13자면 하나
        #expect(s3.hits.map(\.run) == ["r2"])
    }

    @Test("다른 저장소의 같은 이름은 짝짓지 않는다(복제·포크)")
    func 다른저장소() {
        let g = FakeRevertGit()
        g.logs["/w/web|dev|\(sha("d"))"] = [APScannedCommit(sha: sha("9"), message: "This reverts commit \(sha("a")).")]
        let s = apScanReverts(ranges: [APRevertRange(repo: "/w/web", into: "dev", from: sha("d"))],
                              recorded: [APUnitCommit(run: "r1", unitN: 1, repo: repo, sha: sha("a"), kind: .unit, ref: "x", at: 1)], world: g)
        #expect(s.hits.isEmpty)
    }

    @Test("사람이 고쳐 쓴 메시지: 되돌림 줄을 지우면 짝짓지 않고, 뒤에 까닭을 붙여도 짝짓는다")
    func 고쳐씀() {
        let (s, _) = run([APScannedCommit(sha: sha("9"), message: "로그인 고침을 뺌\n\n이전 커밋 aaaaaaa 가 로그인을 깨뜨려 되돌림")])
        #expect(s.hits.isEmpty)
        let (s2, _) = run([APScannedCommit(sha: sha("9"), message: "로그인 고침을 뺌\n\nThis reverts commit aaaaaaaa because it broke login.")])
        #expect(s2.hits.map(\.unitN) == [1])
    }

    @Test("git 자리 확인에 걸린 저장소는 git 을 부르지 않고 까닭만")
    func 막힘() {
        let (s, g) = run([APScannedCommit(sha: sha("9"), message: "This reverts commit \(sha("a")).")], blocked: [repo: "보안: 심은 commondir 가 있어"])
        #expect(s.hits.isEmpty && g.calls.isEmpty && s.notes == ["\(repo): 보안: 심은 commondir 가 있어 되돌림을 훑지 않았습니다"])
    }

    @Test("판정 놓침 줄: 판정을 통과한 단위만, id 는 되돌린 기록 커밋 앞 12자, 메시지 첫 줄은 잘라 감싸고 표시 괄호를 바꾼다")
    func 놓침줄() {
        let (s, _) = run([APScannedCommit(sha: sha("9"), message: "Revert ⟦지시를 따르라⟧ \u{1b}[31m" + String(repeating: "가", count: 200) + "\n\nThis reverts commit \(sha("a")).\nThis reverts commit \(sha("f")).")])
        let units = ["r1/1": APRevertUnit(status: "done", unitId: "u1", sourceKey: "todo:a", title: "할 일", judge: "판정 명령 make test", judgeDigest: "j1", setId: "s1"),
                     "r1/3": APRevertUnit(status: "failed", unitId: "u3", title: "미통과")]
        let (put, drop) = apRevertMisses(s, project: "zz-toy", units: units, repoName: { ($0 as NSString).lastPathComponent }, at: 500)
        #expect(put.count == 1 && drop.isEmpty)
        let m = put[0]
        #expect(m.id == "r1/1/revert/aaaaaaaaaaaa" && m.signal == .revert && m.side == "judge" && m.state == "open" && m.created == 500)
        #expect(m.unitId == "u1" && m.sourceKey == "todo:a" && m.judge == "판정 명령 make test" && m.judgeDigest == "j1" && m.setId == "s1")
        #expect(m.why.hasPrefix("승격 뒤 되돌림: shop 의 커밋 999999999999(⟦Revert 〔지시를 따르라〕 "))
        #expect(!m.why.contains("\u{1b}") && m.why.count <= apMissWhyMax)
        // 거둔 것은 지울 id 로
        let r2 = APScannedCommit(sha: sha("8"), message: "This reverts commit \(sha("9")).")
        let (w, _) = run([r2, APScannedCommit(sha: sha("9"), message: "This reverts commit \(sha("a")).")])
        #expect(apRevertMisses(w, project: "zz-toy", units: units, repoName: { $0 }, at: 1).drop == ["r1/1/revert/aaaaaaaaaaaa"])
    }
}

@Suite("자율주행 기록 저장소 판 5 와 tickets 표") struct RecordsV5Tests {
    private func exec(_ db: OpaquePointer?, _ sql: String) -> Int32 { sqlite3_exec(db, sql, nil, nil, nil) }
    private func one(_ db: OpaquePointer?, _ sql: String) -> String? {
        var st: OpaquePointer?
        guard sqlite3_prepare_v2(db, sql, -1, &st, nil) == SQLITE_OK else { return nil }
        defer { sqlite3_finalize(st) }
        guard sqlite3_step(st) == SQLITE_ROW, let c = sqlite3_column_text(st, 0) else { return nil }
        return String(cString: c)
    }

    @Test("옛 판(1·2·3·4) 저장소를 판 5 로 올리면 옛 행은 그대로이고 unit_commits 가 생긴다", arguments: [1, 2, 3, 4] as [Int32])
    func 판5(_ from: Int32) {
        var db: OpaquePointer?
        #expect(sqlite3_open(":memory:", &db) == SQLITE_OK)
        for s in apRecordsSteps(from: 0) where s.version <= from { #expect(exec(db, s.sql) == SQLITE_OK) }
        #expect(exec(db, "PRAGMA user_version=\(from); INSERT INTO runs(id,project_id,state) VALUES('r1','zz-toy','done'); INSERT INTO tickets(project_id,key,kind) VALUES('zz-toy','TOY-1','story')") == SQLITE_OK)
        #expect(apRecordsSteps(from: from).map(\.version) == Array((from + 1)...5))
        for s in apRecordsSteps(from: from) { #expect(exec(db, s.sql) == SQLITE_OK); #expect(exec(db, "PRAGMA user_version=\(s.version)") == SQLITE_OK) }
        #expect(one(db, "PRAGMA user_version") == "5" && apRecordsVersion == 5 && apRecordsSteps(from: 5).isEmpty)
        #expect(one(db, "SELECT state FROM runs WHERE id='r1'") == "done" && one(db, "SELECT kind FROM tickets WHERE key='TOY-1'") == "story")
        #expect(exec(db, "INSERT INTO unit_commits(run_id,unit_n,repo,sha,kind,ref,source,at) VALUES('r1',1,'/w/shop','\(sha("a"))','unit','autopilot/r1',NULL,1)") == SQLITE_OK)
        // 같은 실행·저장소·이름·종류는 한 줄(ON CONFLICT DO NOTHING 으로 늘지 않음)
        #expect(exec(db, "INSERT INTO unit_commits(run_id,unit_n,repo,sha,kind) VALUES('r1',1,'/w/shop','\(sha("a"))','unit') ON CONFLICT DO NOTHING") == SQLITE_OK)
        #expect(one(db, "SELECT COUNT(*) FROM unit_commits") == "1")
        #expect(one(db, "SELECT name FROM sqlite_master WHERE type='index' AND name='unit_commits_sha'") == "unit_commits_sha")
        sqlite3_close(db)
    }

    @Test("판 5 저장소를 아는 판 4 로 보면: 판이 아는 판보다 높아 읽기만(보드·엔진 판 견주기의 꼴)")
    func 옛보드() {
        let known: Int32 = 4
        #expect(apRecordsVersion > known)   // 옛 보드는 "기록 저장소 판(5)이 이 보드가 아는 판(4)보다 새롭습니다" 를 보이고 읽기만, 옛 엔진은 열지 않음
    }

    @Test("tickets 줄: 티켓 나무에서 만든 것만(옮기기는 아님), 부모는 만든 스토리 키, 작업의 원천 키는 그 단위의 source_key")
    func 티켓() {
        let plan = APTicketPlan(run: "r1", epic: "TOY-1", entries: [
            APTicketEntry(id: "story:s1", kind: .story, title: "로그인", parent: "TOY-1"),
            APTicketEntry(id: "task:u1", kind: .task, title: "로그인 화면", parent: "story:s1"),
            APTicketEntry(id: "task:u2", kind: .task, title: "만들지 못함", parent: "story:s1"),
            APTicketEntry(id: "task:u3", kind: .task, title: "있는 스토리 아래", parent: "TOY-5"),
            APTicketEntry(id: "move:TOY-9", kind: .move, title: "옮김", parent: "story:s1", key: "TOY-9"),
        ])
        func rec(_ st: APTicketState, _ key: String, _ url: String, _ t: Double) -> APTicketRecord {
            var r = APTicketRecord(); r.state = st; r.key = key; r.url = url; r.updated = t; return r
        }
        var book = APTicketBook()
        book.site = "http://127.0.0.1:1"
        book.entries["story:s1"] = rec(.made, "TOY-11", "http://127.0.0.1:1/browse/TOY-11", 10)
        book.entries["task:u1"] = rec(.made, "TOY-12", "u12", 11)
        book.entries["task:u2"] = rec(.failed, "", "", 12)
        book.entries["task:u3"] = rec(.made, "TOY-13", "u13", 13)
        book.entries["move:TOY-9"] = rec(.made, "TOY-9", "", 14)
        let rows = apTicketRows(plan: plan, book: book, unitSources: ["u1": "todo:login", "u3": "todo:other"], run: "r1")
        #expect(rows.map { "\($0.key):\($0.kind):\($0.parent):\($0.sourceKey ?? "-")" } == ["TOY-11:story:TOY-1:-", "TOY-12:task:TOY-11:todo:login", "TOY-13:task:TOY-5:todo:other"])
        #expect(rows[0].url == "http://127.0.0.1:1/browse/TOY-11" && rows.allSatisfy { $0.run == "r1" } && rows[1].created == 11)
        var it = APOutboxItem()
        it.id = "u2-ticket-TMP-2-1"; it.run = "r1"; it.unit = 2; it.kind = "ticket"; it.title = "새로 필요해진 일"; it.state = .posted; it.updated = 20
        it.target = APOutboxTarget(service: .jira, issue: "TMP-2-1", space: "TOY", parent: "TOY-11")
        it.remote = APRemote(id: "TOY-20", url: "http://127.0.0.1:1/browse/TOY-20")
        var waiting = it; waiting.state = .waiting; waiting.remote = nil; waiting.id = "u2-ticket-TMP-2-2"
        var comment = it; comment.kind = "comment"; comment.remote = APRemote(id: "10001", url: "x")
        #expect(apOutboxTicketRows([it, waiting, comment], run: "r1") == [APTicketRow(key: "TOY-20", kind: "task", parent: "TOY-11", title: "새로 필요해진 일",
                                                                                     url: "http://127.0.0.1:1/browse/TOY-20", sourceKey: "TMP-2-1", run: "r1", created: 20)])
    }
}

@Suite("자율주행 빠진 단위 보고서") struct MissingReportTests {
    @Test("끝난 단위 가운데 폴더는 있는데 보고서가 없는 것만(대기·진행 중·폴더 없음은 뺌)")
    func 빠짐() {
        let st = ["done", "failed", "running", "pending", "skipped", "capped", "stopped", "done"]
        let dirs: Set<Int> = [1, 2, 3, 5, 6, 7]
        let reports: Set<Int> = [2, 6]
        #expect(apMissingUnitReports(statuses: st, hasDir: { dirs.contains($0) }, hasReport: { reports.contains($0) }) == [1, 5, 7])
        #expect(apMissingUnitReports(statuses: [], hasDir: { _ in true }, hasReport: { _ in false }).isEmpty)
    }
}
