// 자율주행 규칙(AutopilotRules) — 출발 준비·지난 실행 이어 가기에서 보드가 git 을 읽기 전의 git 자리 확인(다듬기 ⑬-1 ④, ⑦ 보드 git 받침의 남은 것).
// 합치기·승격 시트는 실행 기록(run.json branches.repos 의 git 폴더 자리·inode)으로 바꿔치기를 보지만(BoardGit.swift), 출발 준비(주제로 시작·가져오기의
// 작업 경로, 이어 가기의 단위 작업 폴더)는 기록이 없거나 오래되어 공통 보호(-c core.fsmonitor=false·safe.bareRepository·diff.ignoreSubmodules·플래그)로만 돌았다.
// 공통 보호는 저장소 설정의 fsmonitor·hook 은 덮지만, 고른 경로가 모델이 쓸 수 있던 작업 트리 안의 중첩 git(⑫-8 보안 검토 꼴 B `app/.git`, 꼴 A gitlink +
// `sub/.git` 이 작업 트리 안 git 폴더를 가리킴)이면 그 git 폴더 설정의 clean filter 를 보드의 git status 가 샌드박스 밖에서 돌린다(다듬기 ⑬-1 재현, 표지 1개).
// 그래서 엔진과 같은 확인을 git 을 부르기 전에 한다:
//  ① 파일 시스템으로만 저장소 뿌리와 git 폴더를 찾는다(git 은 rev-parse 도 그 .git 의 설정을 읽으므로 바꿔치기된 자리에서는 git 을 아예 돌리지 않는다 — 엔진 strayGitDir 과 같은 까닭).
//     .git 이 바로 가기·FIFO 면 막고, .git 파일(따로 작업 트리·하위 모듈·떨어진 git 폴더)은 가리키는 자리가 저장소 .git 꼴이고 되짚는 자리(gitdir·commondir)가 맞을 때만.
//  ② 그 git 폴더가 다른 저장소의 작업 트리 안에 있으면(중첩 git) 막는다 — 모델이 쓸 수 있던 자리다. 홈 폴더와 그 위의 .git(홈 dotfiles 저장소)은 보지 않는다.
//  ③ 지난 실행 기록이 있으면(이어 가기) 그때 git 폴더와 같은지(apBoardGitChanged, 공통 git 폴더만 — 끝난 실행의 따로 작업 트리는 지워졌을 수 있다).
//  ④ git 설정 파일(공통 config·config.worktree)을 git 이 해석한 대로(git config --file --no-includes, 저장소를 찾지 않음) 읽어, 명령인 core.fsmonitor 와
//     저장소·임시 폴더 안을 가리키는 include 를 막는다(엔진 configIncludes 와 같은 범위 꼴, 10단 상한).
//  ⑤ 하위 모듈(색인 gitlink·.gitmodules)이 있으면 막는다(엔진 start 가 출발하지 않는 것과 같음). 색인을 못 읽으면 닫힌 쪽.
// 보안 검토 고침(같은 날): 고른 폴더 위의 다른 저장소 작업 트리, .git 파일이 가리키는 git 폴더가 임시·작업 트리 안·자율주행 작업 트리 자리이거나
// 떨어진 git 폴더 꼴, 저장소 설정과 include 로 끌어온 설정의 filter 명령(git-lfs 표준 글 밖), 따로 작업 트리가 아닌 git 폴더의 commondir 도 막는다
// (재현: 보드의 git status 가 그 자리 설정의 clean filter 를 돌림).
// 막으면 그 저장소에서 git 을 더 돌리지 않고(보드가 저장소 줄에 까닭) 출발 준비를 멈춘다. 통과하면 읽기에 박을 env(GIT_DIR·GIT_COMMON_DIR·GIT_WORK_TREE)를 준다.
// 파일 시스템·git 은 포트(APPrepGitWorld)로만 본다 — 진짜 어댑터는 보드, 가짜는 Tests/AutopilotRulesTests/PrepGitTests
import Foundation

// <폴더>/.git 의 꼴(따라가지 않고 본 것)
public enum APGitEntry: Equatable, Sendable {
    case dir               // .git 폴더
    case file(String?)     // .git 파일 — "gitdir:" 뒤 글(그 꼴이 아니거나 못 읽으면 nil)
    case link              // 바로 가기
    case other             // FIFO·장치 등
}

// git 설정 한 줄(git config --null --list 꼴: 키는 git 이 소문자로 준 것, 값 없는 키는 nil)
public struct APConfigEntry: Equatable, Sendable {
    public let key: String
    public let value: String?
    public init(key: String, value: String?) { self.key = key; self.value = value }
}

public protocol APPrepGitWorld {
    /// <dir>/.git 의 꼴(lstat). 없으면 nil
    func entry(_ dir: String) -> APGitEntry?
    /// 실제 경로(realpath). 없으면 nil
    func real(_ path: String) -> String?
    /// 작은 보통 파일의 글(바로 가기·FIFO·큰 파일은 nil)
    func text(_ path: String) -> String?
    /// 보통 파일인가 · 폴더인가(lstat, 바로 가기는 아님)
    func isFile(_ path: String) -> Bool
    func isDir(_ path: String) -> Bool
    /// "장치:inode"(lstat)
    func stamp(_ path: String) -> String?
    /// git config --file <파일> --no-includes --null --list(저장소를 찾지 않는 자리에서). 파일이 없으면 [], 보통 파일이 아니거나 읽지 못하면 nil
    func config(_ file: String) -> [APConfigEntry]?
    /// 박는 env(pin: GIT_DIR·GIT_COMMON_DIR·GIT_WORK_TREE 와 apPrepGitReadEnv)로 색인의 gitlink(ls-files -s -z)와 .gitmodules(작업 트리·색인·HEAD). 색인을 읽지 못하면 nil
    func submodules(top: String, pin: [String]) -> (gitlinks: [String], gitmodules: Bool)?
}

public struct APPrepGit: Equatable, Sendable {
    public var top: String?          // 저장소 뿌리(파일 시스템으로 찾은 실제 경로). nil 이면 git 저장소가 아님(위로 / 까지 .git 없음)
    public var gitDir: String?       // 그 자리의 git 폴더(따로 작업 트리면 .git/worktrees/<이름>)
    public var commonDir: String?    // 공통 git 폴더(가지·객체·config)
    public var block: String?        // 막은 까닭 — 있으면 이 저장소에서 git 을 더 돌리지 않는다
    public init(top: String? = nil, gitDir: String? = nil, commonDir: String? = nil, block: String? = nil) {
        self.top = top; self.gitDir = gitDir; self.commonDir = commonDir; self.block = block
    }
    // 통과한 저장소의 읽기(지금 가지·가지 목록·status·rev-list)에 박는 env — git 이 위로 저장소를 스스로 찾지 않게, 원격으로 객체를 받지 않게(apPrepGitReadEnv).
    // 보안 검토 고침 8: 공통 git 폴더도 확인한 자리로 박는다(GIT_COMMON_DIR 가 있으면 git 은 git 폴더의 commondir 파일을 읽지 않는다 — 확인 뒤에 commondir 를 심어도 따르지 않게)
    public var pin: [String] {
        guard block == nil, let t = top, let g = gitDir else { return [] }
        return ["GIT_DIR=" + g, "GIT_COMMON_DIR=" + (commonDir ?? g), "GIT_WORK_TREE=" + t] + apPrepGitReadEnv
    }
}

private func parent(_ p: String) -> String {
    let d = (p as NSString).deletingLastPathComponent
    return d.isEmpty ? "/" : d
}
private func lexical(_ p: String) -> String { URL(fileURLWithPath: p).standardizedFileURL.path }
public func apPrepShortPath(_ p: String, home: String) -> String { p.hasPrefix(home + "/") ? "~/" + p.dropFirst(home.count + 1) : p }

// 명령이 아닌 core.fsmonitor 값(끔·켬, git 의 불리언 꼴). 켬(true)은 git 이 든 내장 감시이고, 그 밖의 글은 돌릴 명령(hook 경로)이다
private func fsmonitorIsBool(_ v: String?) -> Bool {
    guard let v else { return true }
    return ["", "true", "false", "yes", "no", "on", "off", "1", "0"].contains(v.trimmingCharacters(in: .whitespaces).lowercased())
}

// 보안 검토 고침 6(받침, 재현하지 않음): 출발 준비의 git 읽기는 부분 복제(promisor) 저장소에서 빠진 객체를 받으려고 원격 명령
// (remote.*.uploadpack·core.sshCommand)을 부르지 않게 늦은 받기를 끄고 모든 전송 프로토콜을 막는다(GIT_CONFIG_COUNT 로 명령줄 설정과 같은 무게)
public let apPrepGitReadEnv = ["GIT_NO_LAZY_FETCH=1", "GIT_CONFIG_COUNT=1", "GIT_CONFIG_KEY_0=protocol.allow", "GIT_CONFIG_VALUE_0=never"]

// 보안 검토 고침 7: 출발 준비 읽기의 환경 — 보드가 물려받은 GIT_*(GIT_CONFIG_PARAMETERS·GIT_CONFIG_COUNT/KEY/VALUE·GIT_SSH*·GIT_ALLOW_PROTOCOL·
// GIT_DIR·GIT_INDEX_FILE 등)는 모두 빼고(재현: 물려받은 GIT_CONFIG_PARAMETERS 의 clean filter 를 박은 읽기의 status 가 돌림, 표지 1),
// 시스템·전역 설정·attributes·replace 를 읽지 않는 보호(보드 apGitEnv 와 같은 넷)와 박는 값(extra: "이름=값")만 넣는다
public func apPrepGitEnv(inherited: [String: String], extra: [String]) -> [String: String] {
    var env = inherited.filter { !$0.key.hasPrefix("GIT_") }
    for (k, v) in [("GIT_CONFIG_NOSYSTEM", "1"), ("GIT_CONFIG_GLOBAL", "/dev/null"), ("GIT_ATTR_NOSYSTEM", "1"), ("GIT_NO_REPLACE_OBJECTS", "1")] { env[k] = v }
    for s in extra { if let eq = s.firstIndex(of: "=") { env[String(s[..<eq])] = String(s[s.index(after: eq)...]) } }
    return env
}

// git-lfs 가 저장소에 쓰는 표준 filter 글(git lfs install --local) — 이 글 그대로일 때만 filter 명령을 받는다
private let apLFSFilter: [String: String] = ["filter.lfs.clean": "git-lfs clean -- %f", "filter.lfs.smudge": "git-lfs smudge -- %f", "filter.lfs.process": "git-lfs filter-process"]

// 유스케이스: 경로 하나(고른 작업 경로·단위 작업 폴더)의 저장소와 막을 까닭. records 는 이어 가는 지난 실행의 저장소별 git 자리 기록(뿌리 → 기록)
public func apPrepGitCheck(_ path: String, world: APPrepGitWorld, records: [String: APGitRecord] = [:], home: String) -> APPrepGit {
    guard let start = world.real(path) else { return APPrepGit() }
    // ① 위로 올라가며 처음 만난 .git — git 이 저장소로 보지 않는 .git 폴더(HEAD 없음)는 git 처럼 지나친다.
    // 보안 검토 고침(다듬기 ⑬-2, MEDIUM 찾기가 git 과 다름): git 은 HEAD 가 refs/ 를 가리키는 바로 가기여도 저장소로 받는다(재현: HEAD 를 바로 가기로 바꾼
    // 저장소를 이 규칙은 지나쳐 위 폴더를 고르거나 저장소가 아니라고 봤고, 엔진 prerun 의 탐침이 git 이 찾은 그 저장소에서 filter 를 돌림, 표지 4·8).
    // HEAD 가 있는데 보통 파일이 아니면(바로 가기·FIFO 등) 그 .git 을 찾은 것으로 두고 막는다(닫힌 쪽)
    var d = start, found: (top: String, entry: APGitEntry)?, oddHead = false
    while true {
        if let e = world.entry(d) {
            if case .dir = e, !world.isFile(d + "/.git/HEAD") {
                if world.stamp(d + "/.git/HEAD") != nil { found = (d, e); oddHead = true; break }
            } else { found = (d, e); break }
        }
        if d == "/" { break }
        d = parent(d)
    }
    guard let (top, entry) = found else { return APPrepGit() }
    var r = APPrepGit(top: top)
    func stop(_ why: String) -> APPrepGit { r.block = why; return r }
    if oddHead { return stop(".git/HEAD 가 보통 파일이 아니라(바로 가기 등)") }
    let g: String
    switch entry {
    case .link: return stop(".git 이 바로 가기라")
    case .other: return stop(".git 이 폴더도 파일도 아니라")
    case .file(nil): return stop(".git 파일을 읽지 못해")
    case .file(let raw?):
        let t = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !t.isEmpty, let gg = world.real(t.hasPrefix("/") ? t : top + "/" + t), world.isDir(gg) else { return stop(".git 파일이 가리키는 git 폴더(\(t))가 없어") }
        g = gg
    case .dir:
        g = top + "/.git"
    }
    r.gitDir = g
    // git 폴더 꼴: <뿌리>/.git(또는 bare 꼴 <이름>.git) 그 자체, 그 아래 worktrees/<이름>(따로 작업 트리), modules/…(하위 모듈)
    let comps = g.split(separator: "/", omittingEmptySubsequences: true).map(String.init)
    guard let i = comps.lastIndex(of: ".git") ?? comps.lastIndex(where: { $0.hasSuffix(".git") }) else { return stop("git 폴더(\(apPrepShortPath(g, home: home)))가 저장소 .git 꼴 자리가 아니라") }
    let owner = "/" + comps[...i].joined(separator: "/"), rest = Array(comps[(i + 1)...])
    let base = comps[i] == ".git" ? parent(owner) : owner   // 그 git 폴더를 품은 저장소 뿌리(bare 꼴이면 그 폴더)
    // 보안 검토 고침 1: 고른 폴더(뿌리) 위로도 본다 — 다른 저장소의 작업 트리 안이면 막는다. 그 저장소 자신의 따로 작업 트리·하위 모듈
    // (git 폴더가 그 저장소 .git 의 worktrees·modules 아래)만 예외. 홈 폴더와 그 위는 보지 않는다(홈 dotfiles 저장소)
    var up = parent(top)
    while up != "/" && up != home && !(home + "/").hasPrefix(up + "/") {
        if world.entry(up) != nil, !(up + "/.git" == owner && !rest.isEmpty) {
            return stop("다른 저장소(\(apPrepShortPath(up, home: home)))의 작업 트리 안에 든 git 자리라")
        }
        up = parent(up)
    }
    // 보안 검토 고침 2: .git 파일이 가리키는 git 폴더는 모델이 쓸 수 있던 자리면 막는다 — 그 작업 트리 안, 임시 폴더, 자율주행 작업 트리 자리.
    // 떨어진 git 폴더(--separate-git-dir 꼴)는 되짚는 자리가 없어 누가 만들었는지 가릴 수 없으므로 막는다(닫힌 쪽)
    if case .file = entry {
        if g.hasPrefix(top + "/") { return stop("git 폴더(\(apPrepShortPath(g, home: home)))가 그 작업 트리 안에 있어") }
        let writable = ["/tmp", "/private/tmp", "/var/tmp", "/private/var/tmp", "/var/folders", "/private/var/folders", home + "/.claude-board/worktrees"].map { $0.lowercased() }
        for x in [g, owner] where writable.contains(where: { x.lowercased() == $0 || x.lowercased().hasPrefix($0 + "/") }) {
            return stop("git 폴더(\(apPrepShortPath(g, home: home)))가 임시 폴더·자율주행 작업 트리 자리에 있어")
        }
        if rest.isEmpty { return stop("git 파일이 떨어진 git 폴더(\(apPrepShortPath(g, home: home)))를 가리켜 자리를 믿을 수 없어") }
    }
    // 보안 검토 고침 8: git 은 git 폴더에 commondir 파일이 있으면 그 자리를 공통 git 폴더로 써서 거기 config 를 읽는다(재현: .git/commondir 가 다른 폴더를
    // 가리키고 그 config 에 clean filter 를 심으면 이 확인은 깨끗한 .git/config 만 보고 지나갔고 보드의 git status 가 filter 를 돌림, 표지 2).
    // commondir 는 따로 작업 트리(worktrees/<이름>, 아래에서 그 저장소 git 폴더로 풀리는지 봄)에만 받고, 그 밖의 git 폴더(.git 그대로·modules/…)에 있으면 막는다.
    // 있는지는 lstat(바로 가기·FIFO 도 있음으로)
    let hasCommondir = world.stamp(g + "/commondir") != nil
    var common = g
    if rest.isEmpty {
        // 그 저장소의 .git 폴더 그대로 — 자리는 아래 ② 로 본다
        if hasCommondir { return stop("git 폴더(\(apPrepShortPath(g, home: home)))에 commondir 가 있어 공통 git 폴더를 다른 자리로 돌려") }
    } else if rest.count == 2, rest[0] == "worktrees" {
        // 따로 작업 트리: 되짚는 자리(gitdir)가 이 .git 파일이고 commondir 가 그 git 폴더여야 한다
        let back = world.text(g + "/gitdir")?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        let backReal = back.isEmpty ? nil : world.real(back.hasPrefix("/") ? back : g + "/" + back)
        guard backReal == top + "/.git" else { return stop("따로 작업 트리의 되짚는 자리(gitdir)가 이 폴더가 아니라") }
        let cd = world.text(g + "/commondir")?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        guard !cd.isEmpty, world.real(cd.hasPrefix("/") ? cd : g + "/" + cd) == owner else { return stop("따로 작업 트리의 commondir 가 그 저장소 git 폴더가 아니라") }
        common = owner
    } else if rest.count >= 2, rest[0] == "modules" {
        guard top.hasPrefix(base + "/") else { return stop("하위 모듈 git 폴더가 그 저장소 밖을 가리켜") }
        if hasCommondir { return stop("하위 모듈 git 폴더(\(apPrepShortPath(g, home: home)))에 commondir 가 있어 공통 git 폴더를 다른 자리로 돌려") }
    } else {
        return stop("git 폴더(\(apPrepShortPath(g, home: home)))가 저장소 .git 꼴 자리가 아니라")
    }
    r.commonDir = common
    // ② 다른 저장소의 작업 트리 안에 든 git 폴더(중첩 git)인가 — 홈 폴더와 그 위는 보지 않는다(홈 dotfiles 저장소)
    var a = parent(base)
    while a != "/" && a != home && !(home + "/").hasPrefix(a + "/") {
        if world.entry(a) != nil { return stop("다른 저장소(\(apPrepShortPath(a, home: home)))의 작업 트리 안에 든 git 자리라") }
        a = parent(a)
    }
    // ③ 이어 가는 지난 실행의 기록과 같은 git 폴더인가(공통 git 폴더만)
    if let rec = records[top], let why = apBoardGitChanged(APGitRecord(commonDir: rec.commonDir, commonIno: rec.commonIno, workGitDir: nil, workGitIno: nil),
                                                         liveCommon: common, liveCommonIno: world.stamp(common), liveWorkIno: nil) {
        return stop(why.replacingOccurrences(of: "출발 때", with: "지난 실행이 출발할 때"))
    }
    // ④ git 설정: 명령인 core.fsmonitor, 저장소·임시 폴더 안을 가리키는 include
    let scope = [top, base, common, g, home + "/.claude-board/worktrees", "/tmp", "/private/tmp", "/var/tmp", "/private/var/tmp", "/var/folders", "/private/var/folders"]
    let sc = Set(scope.flatMap { [lexical($0), world.real($0) ?? lexical($0)] }.map { $0.lowercased() })
    func inScope(_ p: String) -> Bool {
        [lexical(p), world.real(p) ?? lexical(p)].map { $0.lowercased() }.contains { f in sc.contains { f == $0 || f.hasPrefix($0 + "/") } }
    }
    var seen = Set<String>()
    func walk(_ file: String, _ depth: Int, root: Bool) -> String? {
        guard seen.insert(world.real(file) ?? lexical(file)).inserted else { return nil }
        guard let entries = world.config(file) else { return "git 설정(\(apPrepShortPath(file, home: home)))을 읽지 못해" }
        let dir = parent(world.real(file) ?? lexical(file))
        for e in entries {
            let k = e.key.lowercased()
            if root, k == "core.fsmonitor", !fsmonitorIsBool(e.value) {
                let v = e.value ?? ""
                return "저장소 설정(\(apPrepShortPath(file, home: home)))에 core.fsmonitor 명령(\(v.count > 60 ? String(v.prefix(60)) + "…" : v))이 있어"
            }
            // 보안 검토 고침 3: filter 드라이버의 명령(clean·smudge·process)은 보드의 git status 가 바뀐 듯한 파일을 견줄 때 돌린다(재현) — git-lfs 표준 글만 받는다.
            // 보안 검토 고침 4: include 로 끌어온 파일(범위 밖이어도)의 filter 도 같다(재현) — 뿌리 파일만이 아니라 지나는 모든 파일에서 본다.
            // core.fsmonitor 는 보드 git 의 -c core.fsmonitor=false 가 끌어온 파일 것까지 덮어 돌지 않으므로(재현 안 됨) 저장소 자체 파일에서만 막는다.
            // diff.external·diff.*.textconv·core.pager·core.sshCommand 는 보드가 돌리는 git(ls-files·cat-file·symbolic-ref·for-each-ref·status·rev-list)이 부르지 않는다(재현 안 됨)
            if k.hasPrefix("filter."), k.hasSuffix(".clean") || k.hasSuffix(".smudge") || k.hasSuffix(".process"), !(apLFSFilter[k] == e.value) {
                return "\(root ? "저장소 설정" : "git 설정")(\(apPrepShortPath(file, home: home)))에 filter 명령(\(k))이 있어"
            }
            guard k == "include.path" || (k.hasPrefix("includeif.") && k.hasSuffix(".path")), let v = e.value, !v.isEmpty else { continue }
            // 보안 검토 고침 5: git 은 ~<사용자>/ 를 그 사용자의 홈으로, %(prefix)/ 를 git 설치 자리로 푼다(재현: ~<이 사용자>/ 로 끌어온 파일의 filter 를 status 가 돌림).
            // 우리 풀이와 달라 엉뚱한 파일을 보게 되므로 닫힌 쪽으로 막는다
            if (v.hasPrefix("~") && v != "~" && !v.hasPrefix("~/")) || v.contains("%(prefix)") { return "git 설정 include 경로(\(v.count > 80 ? String(v.prefix(80)) + "…" : v))를 git 과 같게 풀 수 없어" }
            let p = v == "~" ? home : v.hasPrefix("~/") ? home + v.dropFirst(1) : v.hasPrefix("/") ? v : dir + "/" + v
            if inScope(p) { return "git 설정 include 가 저장소·임시 폴더 안(\(apPrepShortPath(p, home: home)))을 가리켜" }
            if depth >= 10 { return "git 설정 include 가 10단 넘게 겹쳐" }
            if let why = walk(p, depth + 1, root: false) { return why }
        }
        return nil
    }
    for f in [common + "/config", common + "/config.worktree"] + (g != common ? [g + "/config.worktree"] : []) {
        if let why = walk(f, 0, root: true) { return stop(why) }
    }
    // ⑤ 하위 모듈(색인 gitlink·.gitmodules) — 엔진이 출발하지 않는 저장소. 색인을 못 읽으면 닫힌 쪽
    guard let sm = world.submodules(top: top, pin: r.pin) else { return stop("색인을 읽지 못해(git ls-files 실패)") }
    if apHasSubmodule(gitlinks: sm.gitlinks, gitmodules: sm.gitmodules) { return stop(apSubmoduleReason(gitlinks: sm.gitlinks, gitmodules: sm.gitmodules)) }
    return r
}

// 저장소 줄·준비 줄에 보이는 글(엔진의 "보안: <저장소> 의 <까닭> git 을 돌리지 않습니다" 와 같은 꼴)
public func apPrepGitBlockText(top: String, why: String, home: String) -> String {
    "보안: \(apPrepShortPath(top, home: home)) 의 \(why) 그 저장소에서 git 을 돌리지 않았습니다. 사람이 확인한 뒤 다시 읽습니다."
}
