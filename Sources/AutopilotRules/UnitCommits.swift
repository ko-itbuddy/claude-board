// 자율주행 규칙(AutopilotRules) — 단위 커밋 기록과 승격 뒤 되돌림 신호(다듬기 ⑬-2, 2026-11-05, 설계 2장 "③ 판정 놓침 되먹임"·4장 "다듬기 ⑬-2 판단"·
// 5장 7절 unit_commits).
// 단위 커밋: 엔진이 단위를 시작할 때 저장소마다 작업 가지 머리를 적고(heads_start), 단위가 끝날 때 그 머리 → 끝 머리 사이 커밋을 단위 기록(commits)에 남긴다.
// 스토리로 합친 병합 커밋(단위 기록 merge), finish merge·promote 가 만든 병합 커밋(run.json branches.promoted 의 to)도 같은 표에 둔다(기록 저장소 판 5).
// 되돌림 신호(judge_misses signal revert, 판정 쪽): 승격한 가지(promoted 의 into)를 그 승격 머리(to) 뒤로만 훑어 "This reverts commit <sha>" 줄의 sha 가
// 기록된 단위 커밋·병합 커밋과 같을 때만 짝짓는다(시각으로 어림하지 않음). 병합 커밋 되돌림은 그 병합이 들여온 커밋에 든 단위 모두.
// 되돌림을 다시 되돌린 것(다시 살림)은 신호를 거둔다(열린 줄을 지운다, 쓰인 줄은 둔다). 같은 되돌림은 id 가 같아 한 번만 남는다.
// 커밋 메시지는 믿지 않는 자료다 — sha 는 16진수 7~40자 꼴만 받고, 짝지은 결과만 남기며, 메시지 원문은 첫 줄을 짧게 잘라 표시 괄호 ⟦ ⟧ 로 감싼다.
// git 은 포트(APRevertWorld)로만 — 진짜 어댑터는 엔진(EngineRevertWorld, 출발 준비와 같은 git 자리 확인을 지난 저장소만 박은 env 로), 가짜는 시험에만
import Foundation

public enum APCommitKind: String, Equatable, Sendable, CaseIterable {
    case unit                          // 단위가 만든 커밋(단위 시작 머리 → 끝 머리)
    case storyMerge = "story_merge"    // 판정을 통과한 작업 가지를 스토리 가지로 합친 병합 커밋(엔진, 단위 하나)
    case finishMerge = "finish_merge"  // finish merge 가 합칠 쪽(자율주행 가지·스토리 가지)을 받는 가지로 합친 병합 커밋(실행 또는 스토리)
    case promote                       // finish promote 가 기존 가지를 다음 가지로 올린 병합 커밋
}

// unit_commits 한 줄
public struct APUnitCommit: Equatable, Sendable {
    public var run: String
    public var unitN: Int?             // finish_merge·promote 는 nil(실행·스토리 단위의 병합)
    public var repo: String
    public var sha: String             // 소문자 16진수 40자(SHA-256 저장소는 64자)
    public var kind: APCommitKind
    public var ref: String             // 그 커밋이 든 가지: 단위 → 작업 가지, story_merge → 스토리 가지, finish_merge·promote → 받는 가지
    public var source: String?         // story_merge → story:<키>, finish_merge·promote → 합칠 쪽(autopilot·story:<키>·branch:<가지>)
    public var at: Double
    public init(run: String, unitN: Int?, repo: String, sha: String, kind: APCommitKind, ref: String, source: String? = nil, at: Double) {
        self.run = run; self.unitN = unitN; self.repo = repo; self.sha = sha; self.kind = kind; self.ref = ref; self.source = source; self.at = at
    }
}

// 기록할 커밋 이름 꼴(엔진이 git 에서 받은 전체 이름)
public func apFullSHA(_ s: String) -> String? {
    let t = s.lowercased()
    return apMatch(t, "^[0-9a-f]{40}([0-9a-f]{24})?$") ? t : nil
}
// 되돌림 줄의 커밋 이름 꼴(사람이 고쳐 쓸 수 있는 자리): 16진수 7~40자만
public func apRevertSHA(_ s: String) -> String? {
    let t = s.lowercased()
    return apMatch(t, "^[0-9a-f]{7,40}$") ? t : nil
}
// 가지 이름 꼴(기록에서 읽은 받는 가지) — git 명령줄에 넘기므로 좁게
public func apRefNameOK(_ b: String) -> Bool {
    apMatch(b, "^[A-Za-z0-9][A-Za-z0-9._/-]{0,199}$") && !b.contains("..") && !b.contains("//") && !b.hasSuffix("/") && !b.hasSuffix(".lock") && !b.hasSuffix(".")
}

// 단위 기록 하나(엔진 비공개 사본 engine/unit-<n>.json 꼴)에서 줄로: commits(저장소·가지·커밋들)와 merge(스토리로 합친 병합 커밋)
public func apUnitCommitRows(run: String, n: Int, engine e: [String: Any]) -> [APUnitCommit] {
    var out: [APUnitCommit] = [], seen = Set<String>()
    let at = (e["ended"] as? NSNumber)?.doubleValue ?? (e["started"] as? NSNumber)?.doubleValue ?? 0
    let commits = (e["commits"] as? [Any] ?? []).compactMap { $0 as? [String: Any] }
    for c in commits {
        guard let repo = c["repo"] as? String, repo.hasPrefix("/") else { continue }
        let ref = c["branch"] as? String ?? ""
        for s in (c["shas"] as? [Any] ?? []).prefix(apUnitCommitMax) {
            guard let sha = (s as? String).flatMap(apFullSHA), seen.insert(repo + "\u{0}" + sha).inserted else { continue }
            out.append(APUnitCommit(run: run, unitN: n, repo: repo, sha: sha, kind: .unit, ref: ref, at: at))
        }
    }
    // 스토리로 합친 병합 커밋(앞으로 감기는 새 커밋이 없어 남기지 않는다). 저장소는 merge.repo, 옛 기록(⑬-2 전)은 단위 커밋의 저장소가 하나일 때 그것
    if let m = e["merge"] as? [String: Any], m["state"] as? String == "merged", m["how"] as? String == "merge", let sha = (m["head"] as? String).flatMap(apFullSHA) {
        let repos = Set(commits.compactMap { $0["repo"] as? String })
        if let repo = (m["repo"] as? String) ?? (repos.count == 1 ? repos.first : nil), repo.hasPrefix("/") {
            out.append(APUnitCommit(run: run, unitN: n, repo: repo, sha: sha, kind: .storyMerge, ref: m["branch"] as? String ?? "",
                                    source: (m["story"] as? String).map { "story:" + $0 }, at: (m["at"] as? NSNumber)?.doubleValue ?? at))
        }
    }
    return out
}
public let apUnitCommitMax = 500   // 단위 하나에 남길 커밋 수 상한(넘으면 엔진이 more 를 적음)

// run.json branches.promoted 에서 finish 가 만든 병합 커밋 줄(how merge 의 to). 앞으로 감기·그대로는 새 커밋이 없다
public func apFinishCommitRows(run: String, promoted: [[String: Any]]) -> [APUnitCommit] {
    var out: [APUnitCommit] = []
    for p in promoted where p["how"] as? String == "merge" {
        guard let repo = p["repo"] as? String, repo.hasPrefix("/"), let sha = (p["to"] as? String).flatMap(apFullSHA), let into = p["into"] as? String else { continue }
        let src = p["source"] as? String ?? ""
        out.append(APUnitCommit(run: run, unitN: nil, repo: repo, sha: sha, kind: src.hasPrefix("branch:") ? .promote : .finishMerge, ref: into, source: src,
                                at: (p["at"] as? NSNumber)?.doubleValue ?? 0))
    }
    return out
}

// MARK: - 되돌림 줄

// 커밋 메시지에서 되돌린 커밋 이름들 — 줄 앞(빈칸 뺌)이 "This reverts commit " 이고 바로 뒤가 16진수 7~40자, 그 뒤는 끝·마침표·쉼표·빈칸·괄호일 때만.
// git revert 는 "This reverts commit <sha>."(병합은 "<sha>, reversing" 뒤 줄바꿈), revert.reference 는 "<짧은 이름> (<제목>, <날짜>)." 꼴로 쓴다.
// 메시지는 앞 64 KB·200줄만 본다(믿지 않는 자료)
public func apRevertTargets(_ message: String) -> [String] {
    var out: [String] = []
    let head = message.utf8.count > 65_536 ? String(decoding: Array(message.utf8.prefix(65_536)), as: UTF8.self) : message
    for raw in head.split(separator: "\n", omittingEmptySubsequences: false).prefix(200) {
        let line = raw.drop { $0 == " " || $0 == "\t" }
        let lead = "this reverts commit "
        guard line.count > lead.count, line.prefix(lead.count).lowercased() == lead else { continue }
        let rest = line.dropFirst(lead.count)
        let hex = rest.prefix { $0.isASCII && $0.isHexDigit }
        let after = rest.dropFirst(hex.count).first
        guard let sha = apRevertSHA(String(hex)), after == nil || [".", ",", " ", "\t", "(", "\r"].contains(after!) else { continue }
        if !out.contains(sha) { out.append(sha) }
    }
    return out
}

// 훑은 커밋 하나(포트가 준 것)
public struct APScannedCommit: Equatable, Sendable {
    public var sha: String
    public var message: String
    public init(sha: String, message: String) { self.sha = sha; self.message = message }
    public var subject: String { String(message.split(separator: "\n", maxSplits: 1, omittingEmptySubsequences: false).first ?? "") }
}

// 훑을 자리: 저장소 repo 의 받는 가지 into 를 승격 머리 from 뒤로만(from..refs/heads/into)
public struct APRevertRange: Equatable, Sendable, Hashable {
    public var repo: String
    public var into: String
    public var from: String
    public init(repo: String, into: String, from: String) { self.repo = repo; self.into = into; self.from = from }
}

// 실행들의 승격 기록(branches.promoted)에서 훑을 자리 — 꼴이 맞는 줄만(저장소 절대 경로·가지 이름 꼴·머리 전체 이름), 같은 자리는 한 번
public func apRevertRanges(promoted: [[String: Any]]) -> [APRevertRange] {
    var out: [APRevertRange] = []
    for p in promoted {
        guard let repo = p["repo"] as? String, repo.hasPrefix("/"), !repo.contains("\n"), let into = p["into"] as? String, apRefNameOK(into),
              let from = (p["to"] as? String).flatMap(apFullSHA) else { continue }
        let r = APRevertRange(repo: repo, into: into, from: from)
        if !out.contains(r) { out.append(r) }
    }
    return out
}

public protocol APRevertWorld {
    /// repo 의 from..refs/heads/<into> 커밋(새것부터 max 개까지, 이름은 전체 이름). 읽지 못하면 nil
    func commits(repo: String, into: String, from: String, max: Int) -> [APScannedCommit]?
    /// 병합 커밋 merge 가 들여온 커밋(merge^1..merge, max 개까지, merge 자신 포함). 읽지 못하면 nil
    func introduced(repo: String, merge: String, max: Int) -> [String]?
}
public let apRevertScanMax = 2000      // 훑을 자리 하나에서 읽는 커밋 수 상한(새것부터)
public let apRevertIntroducedMax = 5000

// 짝지은 되돌림 — 단위 하나가 되돌림 커밋 하나에 걸린 것
public struct APRevertHit: Equatable, Sendable {
    public var run: String
    public var unitN: Int
    public var repo: String
    public var target: String          // 되돌린 기록 커밋(전체 이름) — 단위 커밋이거나 그 단위를 담은 병합 커밋
    public var targetKind: APCommitKind
    public var revert: String          // 되돌림 커밋(전체 이름)
    public var subject: String         // 되돌림 커밋 첫 줄(다듬기 전 글, 줄 글은 apRevertMisses 가 다듬음)
    public init(run: String, unitN: Int, repo: String, target: String, targetKind: APCommitKind, revert: String, subject: String) {
        self.run = run; self.unitN = unitN; self.repo = repo; self.target = target; self.targetKind = targetKind; self.revert = revert; self.subject = subject
    }
    public var key: String { "\(run)/\(unitN)/\(target)" }
}
public struct APRevertScan: Equatable, Sendable {
    public var hits: [APRevertHit] = []
    public var withdrawn: [(run: String, unitN: Int, target: String)] = []   // 되돌림이 다시 되돌려져 거둘 것(그 단위·대상에 산 되돌림이 없음)
    public var notes: [String] = []
    public var scanned = 0
    public init() {}
    public static func == (a: APRevertScan, b: APRevertScan) -> Bool {
        a.hits == b.hits && a.notes == b.notes && a.scanned == b.scanned
            && a.withdrawn.map { "\($0.run)/\($0.unitN)/\($0.target)" } == b.withdrawn.map { "\($0.run)/\($0.unitN)/\($0.target)" }
    }
}

// 유스케이스: 훑을 자리마다 커밋을 읽어 되돌림 줄을 기록된 커밋과 짝짓는다. blocked: git 자리 확인에 걸린 저장소(→ 까닭) — 그 저장소는 git 을 부르지 않는다.
// 저장소가 다르면 같은 이름이어도 짝짓지 않는다(복제·포크). 짧은 이름이 기록된 커밋 둘 이상에 맞으면 짝짓지 않는다(닫힌 쪽)
public func apScanReverts(ranges: [APRevertRange], recorded: [APUnitCommit], world: APRevertWorld, blocked: [String: String] = [:]) -> APRevertScan {
    var scan = APRevertScan()
    var order: [String] = []
    for r in ranges where !order.contains(r.repo) { order.append(r.repo) }
    for repo in order {
        if let why = blocked[repo] { scan.notes.append("\(repo): \(why) 되돌림을 훑지 않았습니다"); continue }
        // 이 저장소의 훑은 커밋(여러 자리에서 겹치면 한 번)
        var commits: [APScannedCommit] = [], seen = Set<String>()
        for r in ranges where r.repo == repo {
            guard let cs = world.commits(repo: repo, into: r.into, from: r.from, max: apRevertScanMax) else {
                scan.notes.append("\(repo) \(r.into): 커밋을 읽지 못해 되돌림을 훑지 않았습니다"); continue
            }
            if cs.count >= apRevertScanMax { scan.notes.append("\(repo) \(r.into): 승격 뒤 커밋이 \(apRevertScanMax)개를 넘어 새것부터 그만큼만 봤습니다") }
            for c in cs { if let s = apFullSHA(c.sha), seen.insert(s).inserted { commits.append(APScannedCommit(sha: s, message: c.message)) } }
        }
        scan.scanned += commits.count
        let mine = recorded.filter { $0.repo == repo }
        let names = Set(mine.map(\.sha)).union(commits.map(\.sha))
        func resolve(_ p: String) -> String? {
            let hits = names.filter { $0.hasPrefix(p) }
            return hits.count == 1 ? hits.first : nil
        }
        // 되돌림 커밋 → 되돌린 이름들(풀린 것만)
        var reverts: [String: [String]] = [:], subject: [String: String] = [:]
        for c in commits {
            let ts = apRevertTargets(c.message)
            guard !ts.isEmpty else { continue }
            var full: [String] = []
            for t in ts {
                if let f = resolve(t) { if f != c.sha { full.append(f) } }
                else if names.contains(where: { $0.hasPrefix(t) }) { scan.notes.append("\(repo): 되돌림 줄의 짧은 이름 \(t) 가 여러 커밋에 맞아 짝짓지 않았습니다") }
            }
            if !full.isEmpty { reverts[c.sha] = full; subject[c.sha] = c.subject }
        }
        // 산 되돌림인가: 그것을 되돌린 산 되돌림이 없으면(되돌림의 되돌림은 다시 살림). 고리·깊이 32 를 넘으면 산 것으로 본다
        var memo: [String: Bool] = [:]
        func live(_ r: String, _ depth: Int, _ path: Set<String>) -> Bool {
            if let v = memo[r] { return v }
            if depth > 32 || path.contains(r) { return true }
            let undone = reverts.contains { k, v in v.contains(r) && live(k, depth + 1, path.union([r])) }
            memo[r] = !undone
            return !undone
        }
        // 기록된 커밋 → 그 커밋에 걸린 단위들
        var introducedCache: [String: [String]?] = [:]
        func units(of sha: String) -> [(run: String, n: Int, kind: APCommitKind)] {
            var out: [(run: String, n: Int, kind: APCommitKind)] = []
            func add(_ run: String, _ n: Int, _ k: APCommitKind) { if !out.contains(where: { $0.run == run && $0.n == n }) { out.append((run, n, k)) } }
            for rec in mine where rec.sha == sha {
                switch rec.kind {
                case .unit, .storyMerge: if let n = rec.unitN { add(rec.run, n, rec.kind) }
                case .finishMerge, .promote:
                    // 병합이 들여온 커밋에 든 단위 모두(단위 커밋·스토리 병합 커밋으로)
                    if introducedCache[sha] == nil { introducedCache[sha] = world.introduced(repo: repo, merge: sha, max: apRevertIntroducedMax) }
                    guard let inn = introducedCache[sha] ?? nil else { scan.notes.append("\(repo): 병합 커밋 \(sha.prefix(12)) 이 들여온 커밋을 읽지 못했습니다"); continue }
                    let set = Set(inn.compactMap(apFullSHA))
                    for u in mine where (u.kind == .unit || u.kind == .storyMerge) && set.contains(u.sha) { if let n = u.unitN { add(u.run, n, rec.kind) } }
                }
            }
            return out
        }
        var hitKeys = Set<String>()
        var withdrawn: [(run: String, unitN: Int, target: String)] = []
        for c in commits {
            guard let ts = reverts[c.sha] else { continue }
            let alive = live(c.sha, 0, [])
            for t in ts where mine.contains(where: { $0.sha == t }) {
                for u in units(of: t) {
                    if alive {
                        let h = APRevertHit(run: u.run, unitN: u.n, repo: repo, target: t, targetKind: u.kind, revert: c.sha, subject: subject[c.sha] ?? "")
                        if hitKeys.insert(h.key).inserted { scan.hits.append(h) }
                    } else if !withdrawn.contains(where: { $0.run == u.run && $0.unitN == u.n && $0.target == t }) {
                        withdrawn.append((u.run, u.n, t))
                    }
                }
            }
        }
        scan.withdrawn += withdrawn.filter { !hitKeys.contains("\($0.run)/\($0.unitN)/\($0.target)") }
    }
    return scan
}

// 판정 놓침 줄의 id — <실행 id>/<단위 번호>/revert/<되돌린 기록 커밋 앞 12자>
public func apRevertMissId(run: String, unitN: Int, target: String) -> String { "\(run)/\(unitN)/revert/\(String(target.prefix(12)))" }

// 단위 하나의 기록(units 표와 고정 설정에서)
public struct APRevertUnit: Equatable, Sendable {
    public var status: String
    public var unitId: String?
    public var sourceKey: String?
    public var title: String
    public var judge: String?
    public var judgeDigest: String?
    public var setId: String?
    public init(status: String, unitId: String? = nil, sourceKey: String? = nil, title: String = "", judge: String? = nil, judgeDigest: String? = nil, setId: String? = nil) {
        self.status = status; self.unitId = unitId; self.sourceKey = sourceKey; self.title = title; self.judge = judge; self.judgeDigest = judgeDigest; self.setId = setId
    }
}

// 짝지은 되돌림 → 판정 놓침 줄(판정을 통과한 단위만 — 판정 쪽 신호). 지울 것: 거둔 되돌림의 id(열린 줄만 지운다, 엔진)
public func apRevertMisses(_ scan: APRevertScan, project: String, units: [String: APRevertUnit], repoName: (String) -> String, at: Double) -> (put: [APMiss], drop: [String]) {
    var put: [APMiss] = []
    for h in scan.hits {
        guard let u = units["\(h.run)/\(h.unitN)"], u.status == "done" else { continue }
        let subj = apTakeoverClean(h.subject.replacingOccurrences(of: "\r", with: " "), max: 80)
        let what: String
        switch h.targetKind {
        case .unit: what = "이 단위가 만든 커밋 \(h.target.prefix(12))"
        case .storyMerge: what = "이 단위를 스토리 가지로 합친 병합 커밋 \(h.target.prefix(12))"
        case .finishMerge: what = "이 단위가 든 합치기 병합 커밋 \(h.target.prefix(12))"
        case .promote: what = "이 단위가 든 승격 병합 커밋 \(h.target.prefix(12))"
        }
        let why = "승격 뒤 되돌림: \(apDefangMarkers(repoName(h.repo))) 의 커밋 \(h.revert.prefix(12))(⟦\(subj)⟧)가 \(what) 을 되돌렸습니다(판정은 통과시켰음)"
        put.append(APMiss(id: apRevertMissId(run: h.run, unitN: h.unitN, target: h.target), project: project, run: h.run, unitN: h.unitN, unitId: u.unitId,
                          sourceKey: u.sourceKey, signal: .revert, judge: u.judge, judgeDigest: u.judge == nil ? nil : u.judgeDigest,
                          why: apClipMissWhy(why), setId: u.setId, created: at, title: apClipMissWhy(u.title, 120)))
    }
    let drop = scan.withdrawn.map { apRevertMissId(run: $0.run, unitN: $0.unitN, target: $0.target) }.filter { id in !put.contains { $0.id == id } }
    return (put, drop)
}
