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
// 막으면 그 저장소에서 git 을 더 돌리지 않고(보드가 저장소 줄에 까닭) 출발 준비를 멈춘다. 통과하면 읽기에 박을 env(GIT_DIR·GIT_WORK_TREE)를 준다.
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
    /// 박은 GIT_DIR·GIT_WORK_TREE 로 색인의 gitlink(ls-files -s -z)와 .gitmodules(작업 트리·색인·HEAD). 색인을 읽지 못하면 nil
    func submodules(top: String, gitDir: String) -> (gitlinks: [String], gitmodules: Bool)?
}

public struct APPrepGit: Equatable, Sendable {
    public var top: String?          // 저장소 뿌리(파일 시스템으로 찾은 실제 경로). nil 이면 git 저장소가 아님(위로 / 까지 .git 없음)
    public var gitDir: String?       // 그 자리의 git 폴더(따로 작업 트리면 .git/worktrees/<이름>)
    public var commonDir: String?    // 공통 git 폴더(가지·객체·config)
    public var block: String?        // 막은 까닭 — 있으면 이 저장소에서 git 을 더 돌리지 않는다
    public init(top: String? = nil, gitDir: String? = nil, commonDir: String? = nil, block: String? = nil) {
        self.top = top; self.gitDir = gitDir; self.commonDir = commonDir; self.block = block
    }
    // 통과한 저장소의 읽기(지금 가지·가지 목록·status·rev-list)에 박는 env — git 이 위로 저장소를 스스로 찾지 않게
    public var pin: [String] {
        guard block == nil, let t = top, let g = gitDir else { return [] }
        return ["GIT_DIR=" + g, "GIT_WORK_TREE=" + t]
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

// 유스케이스: 경로 하나(고른 작업 경로·단위 작업 폴더)의 저장소와 막을 까닭. records 는 이어 가는 지난 실행의 저장소별 git 자리 기록(뿌리 → 기록)
public func apPrepGitCheck(_ path: String, world: APPrepGitWorld, records: [String: APGitRecord] = [:], home: String) -> APPrepGit {
    guard let start = world.real(path) else { return APPrepGit() }
    // ① 위로 올라가며 처음 만난 .git — git 이 저장소로 보지 않는 .git 폴더(HEAD 없음)는 git 처럼 지나친다
    var d = start, found: (top: String, entry: APGitEntry)?
    while true {
        if let e = world.entry(d) {
            if case .dir = e, !world.isFile(d + "/.git/HEAD") {} else { found = (d, e); break }
        }
        if d == "/" { break }
        d = parent(d)
    }
    guard let (top, entry) = found else { return APPrepGit() }
    var r = APPrepGit(top: top)
    func stop(_ why: String) -> APPrepGit { r.block = why; return r }
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
    var common = g
    if rest.isEmpty {
        // 그 저장소의 git 폴더 그대로(.git 파일이 가리키는 떨어진 git 폴더 --separate-git-dir 꼴도) — 자리는 아래 ② 로 본다
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
            guard k == "include.path" || (k.hasPrefix("includeif.") && k.hasSuffix(".path")), let v = e.value, !v.isEmpty else { continue }
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
    guard let sm = world.submodules(top: top, gitDir: g) else { return stop("색인을 읽지 못해(git ls-files 실패)") }
    if apHasSubmodule(gitlinks: sm.gitlinks, gitmodules: sm.gitmodules) { return stop(apSubmoduleReason(gitlinks: sm.gitlinks, gitmodules: sm.gitmodules)) }
    return r
}

// 저장소 줄·준비 줄에 보이는 글(엔진의 "보안: <저장소> 의 <까닭> git 을 돌리지 않습니다" 와 같은 꼴)
public func apPrepGitBlockText(top: String, why: String, home: String) -> String {
    "보안: \(apPrepShortPath(top, home: home)) 의 \(why) 그 저장소에서 git 을 돌리지 않았습니다. 사람이 확인한 뒤 다시 읽습니다."
}
