// 출발 준비 git 자리 확인(Sources/AutopilotRules/PrepGit.swift) 시험 — 다듬기 ⑬-1 ④.
// 가짜 파일 시스템(폴더·파일·바로 가기·FIFO)과 가짜 git(설정 줄·색인)으로 꼴마다 뿌리·git 폴더·막는 까닭을 본다.
// 꼴은 ⑫-8 보안 검토 재현(꼴 A: gitlink + sub/.git 이 작업 트리 안 git 폴더를 가리킴, 꼴 B: 하위 폴더 app/.git)과 저장소 자체 .git/config 의 core.fsmonitor,
// 그리고 막지 말아야 할 보통 꼴(따로 작업 트리·작업 트리 안 따로 작업 트리·하위 모듈 자리·떨어진 git 폴더·홈 dotfiles 저장소)
import Foundation
import Testing
import AutopilotRules

final class FakeGitFS: APPrepGitWorld, @unchecked Sendable {
    var dirs: Set<String> = ["/"]
    var files: [String: String] = [:]
    var links: [String: String] = [:]
    var fifos: Set<String> = []
    var configs: [String: [APConfigEntry]] = [:]
    var index: [String: (gitlinks: [String], gitmodules: Bool)] = [:]
    var indexFail: Set<String> = []
    var stamps: [String: String] = [:]

    func mkdir(_ p: String) {
        var cur = ""
        for c in p.split(separator: "/") { cur += "/" + c; dirs.insert(cur) }
    }
    func put(_ p: String, _ text: String = "") { mkdir((p as NSString).deletingLastPathComponent); files[p] = text }
    func repo(_ r: String) { mkdir(r + "/.git"); put(r + "/.git/HEAD", "ref: refs/heads/main\n"); put(r + "/.git/config") }
    func cfg(_ file: String, _ kv: [(String, String?)]) { if files[file] == nil { put(file) }; configs[file] = kv.map { APConfigEntry(key: $0.0, value: $0.1) } }

    private func lexical(_ p: String) -> String { URL(fileURLWithPath: p).standardizedFileURL.path }
    func resolve(_ p: String, _ hops: Int = 0) -> String? {
        var cur = ""
        let parts = lexical(p).split(separator: "/").map(String.init)
        for (i, c) in parts.enumerated() {
            cur += "/" + c
            if let t = links[cur] {
                guard hops < 16 else { return nil }
                let rest = parts[(i + 1)...].joined(separator: "/")
                return resolve(t + (rest.isEmpty ? "" : "/" + rest), hops + 1)
            }
        }
        if cur.isEmpty { cur = "/" }
        return dirs.contains(cur) || files[cur] != nil || fifos.contains(cur) ? cur : nil
    }
    func entry(_ dir: String) -> APGitEntry? {
        let p = (dir == "/" ? "" : dir) + "/.git"
        if links[p] != nil { return .link }
        if fifos.contains(p) { return .other }
        if dirs.contains(p) { return .dir }
        if let f = files[p] { return .file(f.hasPrefix("gitdir:") ? String(f.dropFirst(7)) : nil) }
        return nil
    }
    func real(_ path: String) -> String? { resolve(path) }
    func text(_ path: String) -> String? { links[path] == nil ? files[path] : nil }
    func isFile(_ path: String) -> Bool { links[path] == nil && files[path] != nil }
    func isDir(_ path: String) -> Bool { links[path] == nil && dirs.contains(path) }
    func stamp(_ path: String) -> String? { stamps[path] ?? (resolve(path) != nil ? "1:" + path : nil) }
    func config(_ file: String) -> [APConfigEntry]? {
        if links[file] != nil || fifos.contains(file) { return nil }
        guard files[file] != nil else { return [] }
        return configs[file] ?? []
    }
    func submodules(top: String, gitDir: String) -> (gitlinks: [String], gitmodules: Bool)? {
        indexFail.contains(top) ? nil : index[top] ?? ([], false)
    }
}

struct PrepGitRow: Row {
    let note: String
    let path: String
    var home = "/h"
    var records: [String: APGitRecord] = [:]
    let build: @Sendable (FakeGitFS) -> Void
    let top: String?
    var gitDir: String? = nil
    var block: String? = nil       // 막는 까닭(앞부분이 같으면 됨)
}

private let rows: [PrepGitRow] = [
    .init(note: "보통 저장소 뿌리", path: "/w/a", build: { $0.repo("/w/a") }, top: "/w/a", gitDir: "/w/a/.git"),
    .init(note: "하위 폴더에서 고르면 위로 찾은 뿌리", path: "/w/a/src/x", build: { $0.repo("/w/a"); $0.mkdir("/w/a/src/x") }, top: "/w/a", gitDir: "/w/a/.git"),
    .init(note: "git 저장소가 아님(위로 / 까지 .git 없음)", path: "/w/plain", build: { $0.mkdir("/w/plain") }, top: nil),
    .init(note: "없는 경로", path: "/w/none", build: { _ in }, top: nil),
    .init(note: "HEAD 없는 .git 폴더는 git 처럼 지나친다", path: "/w/a/x", build: { $0.repo("/w/a"); $0.mkdir("/w/a/x/.git") }, top: "/w/a", gitDir: "/w/a/.git"),
    .init(note: ".git 이 바로 가기면 막음", path: "/w/l", build: { $0.mkdir("/w/l"); $0.repo("/w/real"); $0.links["/w/l/.git"] = "/w/real/.git" },
          top: "/w/l", block: ".git 이 바로 가기라"),
    .init(note: ".git 이 FIFO 면 막음", path: "/w/f", build: { $0.mkdir("/w/f"); $0.fifos.insert("/w/f/.git") }, top: "/w/f", block: ".git 이 폴더도 파일도 아니라"),
    .init(note: ".git 파일이 gitdir 꼴이 아니면 막음", path: "/w/g", build: { $0.put("/w/g/.git", "hello") }, top: "/w/g", block: ".git 파일을 읽지 못해"),
    .init(note: ".git 파일이 없는 자리를 가리키면 막음", path: "/w/g", build: { $0.put("/w/g/.git", "gitdir: /w/nowhere/.git\n") }, top: "/w/g",
          block: ".git 파일이 가리키는 git 폴더(/w/nowhere/.git)가 없어"),
    .init(note: "따로 작업 트리(저장소 밖) 통과", path: "/w/wt/x", build: { f in
        f.repo("/w/a"); f.put("/w/wt/x/.git", "gitdir: /w/a/.git/worktrees/x\n")
        f.put("/w/a/.git/worktrees/x/gitdir", "/w/wt/x/.git\n"); f.put("/w/a/.git/worktrees/x/commondir", "../..\n"); f.put("/w/a/.git/worktrees/x/HEAD")
    }, top: "/w/wt/x", gitDir: "/w/a/.git/worktrees/x"),
    .init(note: "작업 트리 안 따로 작업 트리(.claude/worktrees 꼴)도 통과 — 중첩이지만 그 저장소의 git 폴더", path: "/w/a/.claude/worktrees/x", build: { f in
        f.repo("/w/a"); f.put("/w/a/.claude/worktrees/x/.git", "gitdir: /w/a/.git/worktrees/x")
        f.put("/w/a/.git/worktrees/x/gitdir", "/w/a/.claude/worktrees/x/.git"); f.put("/w/a/.git/worktrees/x/commondir", "../..")
    }, top: "/w/a/.claude/worktrees/x", gitDir: "/w/a/.git/worktrees/x"),
    .init(note: "따로 작업 트리의 되짚는 자리가 다른 폴더면 막음", path: "/w/wt/y", build: { f in
        f.repo("/w/a"); f.put("/w/wt/y/.git", "gitdir: /w/a/.git/worktrees/x"); f.put("/w/wt/x/.git", "gitdir: /w/a/.git/worktrees/x")
        f.put("/w/a/.git/worktrees/x/gitdir", "/w/wt/x/.git"); f.put("/w/a/.git/worktrees/x/commondir", "../..")
    }, top: "/w/wt/y", block: "따로 작업 트리의 되짚는 자리(gitdir)가 이 폴더가 아니라"),
    .init(note: "따로 작업 트리의 commondir 가 다른 git 폴더면 막음", path: "/w/wt/x", build: { f in
        f.repo("/w/a"); f.repo("/w/evil"); f.put("/w/wt/x/.git", "gitdir: /w/a/.git/worktrees/x")
        f.put("/w/a/.git/worktrees/x/gitdir", "/w/wt/x/.git"); f.put("/w/a/.git/worktrees/x/commondir", "/w/evil/.git")
    }, top: "/w/wt/x", block: "따로 작업 트리의 commondir 가 그 저장소 git 폴더가 아니라"),
    .init(note: "⑫-8 꼴 A: sub/.git 이 작업 트리 안 git 폴더를 가리키면 막음", path: "/w/gl/sub", build: { f in
        f.repo("/w/gl"); f.repo("/w/gl/.cache/m"); f.put("/w/gl/sub/.git", "gitdir: ../.cache/m/.git\n")
    }, top: "/w/gl/sub", gitDir: "/w/gl/.cache/m/.git", block: "다른 저장소(/w/gl)의 작업 트리 안에 든 git 자리라"),
    .init(note: "⑫-8 꼴 A 의 바깥 저장소(색인에 gitlink): 하위 모듈로 막음", path: "/w/gl", build: { f in
        f.repo("/w/gl"); f.repo("/w/gl/.cache/m"); f.put("/w/gl/sub/.git", "gitdir: ../.cache/m/.git"); f.index["/w/gl"] = (["sub"], false)
    }, top: "/w/gl", block: "작업 폴더에 하위 모듈(색인에 중첩 git 저장소(sub))이 있어"),
    .init(note: "⑫-8 꼴 B: 하위 폴더 app/.git(중첩 저장소)을 고르면 막음", path: "/w/nest/app", build: { $0.repo("/w/nest"); $0.repo("/w/nest/app") },
          top: "/w/nest/app", block: "다른 저장소(/w/nest)의 작업 트리 안에 든 git 자리라"),
    .init(note: "⑫-8 꼴 B 의 바깥 저장소는 막지 않음(보드 git 이 추적하지 않는 중첩 자리로 들어가지 않음)", path: "/w/nest", build: { $0.repo("/w/nest"); $0.repo("/w/nest/app") },
          top: "/w/nest", gitDir: "/w/nest/.git"),
    .init(note: "꼴 B 에서 하위 폴더 안쪽을 고르면 가까운 app 이 뿌리, 막음", path: "/w/nest/app/src", build: { $0.repo("/w/nest"); $0.repo("/w/nest/app"); $0.mkdir("/w/nest/app/src") },
          top: "/w/nest/app", block: "다른 저장소(/w/nest)의 작업 트리 안에 든 git 자리라"),
    .init(note: "홈 폴더 dotfiles 저장소(~/.git)는 보지 않음", path: "/h/w/a", build: { $0.repo("/h"); $0.repo("/h/w/a") }, top: "/h/w/a", gitDir: "/h/w/a/.git"),
    .init(note: "홈 밖 저장소도 그 위를 본다", path: "/opt/x/r", build: { $0.repo("/opt/x"); $0.repo("/opt/x/r") }, top: "/opt/x/r",
          block: "다른 저장소(/opt/x)의 작업 트리 안에 든 git 자리라"),
    .init(note: "하위 모듈 자리(.git/modules) 통과", path: "/w/s/sub", build: { f in
        f.repo("/w/s"); f.mkdir("/w/s/.git/modules/sub"); f.put("/w/s/.git/modules/sub/HEAD"); f.put("/w/s/sub/.git", "gitdir: ../.git/modules/sub")
    }, top: "/w/s/sub", gitDir: "/w/s/.git/modules/sub"),
    .init(note: "하위 모듈 git 폴더가 그 저장소 밖이면 막음", path: "/w/t/sub", build: { f in
        f.repo("/w/s"); f.mkdir("/w/s/.git/modules/sub"); f.mkdir("/w/t"); f.put("/w/t/sub/.git", "gitdir: /w/s/.git/modules/sub")
    }, top: "/w/t/sub", block: "하위 모듈 git 폴더가 그 저장소 밖을 가리켜"),
    .init(note: "떨어진 git 폴더(<이름>.git) 통과", path: "/w/sep", build: { f in f.mkdir("/w/store/sep.git"); f.put("/w/store/sep.git/HEAD"); f.put("/w/sep/.git", "gitdir: /w/store/sep.git") },
          top: "/w/sep", gitDir: "/w/store/sep.git"),
    .init(note: ".git 꼴이 아닌 자리를 가리키면 막음", path: "/w/odd", build: { f in f.mkdir("/w/store/plain"); f.put("/w/odd/.git", "gitdir: /w/store/plain") },
          top: "/w/odd", block: "git 폴더(/w/store/plain)가 저장소 .git 꼴 자리가 아니라"),
    .init(note: "저장소 자체 .git/config 의 core.fsmonitor 명령은 막음", path: "/w/cfg", build: { f in f.repo("/w/cfg"); f.cfg("/w/cfg/.git/config", [("core.fsmonitor", "/w/cfg/plant.sh")]) },
          top: "/w/cfg", block: "저장소 설정(/w/cfg/.git/config)에 core.fsmonitor 명령(/w/cfg/plant.sh)이 있어"),
    .init(note: "core.fsmonitor=true(git 내장 감시)·false 는 통과", path: "/w/cfg", build: { f in f.repo("/w/cfg"); f.cfg("/w/cfg/.git/config", [("core.fsmonitor", "true"), ("core.fsmonitor", "FALSE"), ("core.fsmonitor", nil)]) },
          top: "/w/cfg", gitDir: "/w/cfg/.git"),
    .init(note: "따로 작업 트리의 config.worktree 에 심은 fsmonitor 도 막음", path: "/w/wt/x", build: { f in
        f.repo("/w/a"); f.put("/w/wt/x/.git", "gitdir: /w/a/.git/worktrees/x")
        f.put("/w/a/.git/worktrees/x/gitdir", "/w/wt/x/.git"); f.put("/w/a/.git/worktrees/x/commondir", "../..")
        f.cfg("/w/a/.git/worktrees/x/config.worktree", [("core.fsmonitor", "touch x")])
    }, top: "/w/wt/x", block: "저장소 설정(/w/a/.git/worktrees/x/config.worktree)에 core.fsmonitor 명령(touch x)이 있어"),
    .init(note: "include 가 작업 트리 안을 가리키면 막음(대소문자 접음)", path: "/w/a", build: { f in f.repo("/w/a"); f.cfg("/w/a/.git/config", [("include.path", "/W/A/team.gitconfig")]) },
          top: "/w/a", block: "git 설정 include 가 저장소·임시 폴더 안(/W/A/team.gitconfig)을 가리켜"),
    .init(note: "상대 include 는 그 파일 폴더 기준(.git 안 = 범위 안)", path: "/w/a", build: { f in f.repo("/w/a"); f.cfg("/w/a/.git/config", [("includeif.onbranch:main.path", "local.cfg")]) },
          top: "/w/a", block: "git 설정 include 가 저장소·임시 폴더 안(/w/a/.git/local.cfg)을 가리켜"),
    .init(note: "include 가 임시 폴더를 가리키면 막음", path: "/w/a", build: { f in f.repo("/w/a"); f.cfg("/w/a/.git/config", [("include.path", "/tmp/x.cfg")]) },
          top: "/w/a", block: "git 설정 include 가 저장소·임시 폴더 안(/tmp/x.cfg)을 가리켜"),
    .init(note: "범위 밖 include 는 따라가 그 안의 include 도 본다", path: "/w/a", build: { f in
        f.repo("/w/a"); f.cfg("/w/a/.git/config", [("include.path", "~/team.cfg")]); f.cfg("/h/team.cfg", [("include.path", "/w/a/evil.cfg")])
    }, top: "/w/a", block: "git 설정 include 가 저장소·임시 폴더 안(/w/a/evil.cfg)을 가리켜"),
    .init(note: "범위 밖 파일의 fsmonitor 는 사람 설정이라 막지 않음(보드 git 의 -c 가 덮음)", path: "/w/a", build: { f in
        f.repo("/w/a"); f.cfg("/w/a/.git/config", [("include.path", "~/team.cfg")]); f.cfg("/h/team.cfg", [("core.fsmonitor", "watchman-hook")])
    }, top: "/w/a", gitDir: "/w/a/.git"),
    .init(note: "include 가 10단 넘게 겹치면 막음", path: "/w/a", build: { f in
        f.repo("/w/a"); f.cfg("/w/a/.git/config", [("include.path", "/o/c0")])
        for i in 0..<12 { f.cfg("/o/c\(i)", [("include.path", "/o/c\(i + 1)")]) }
    }, top: "/w/a", block: "git 설정 include 가 10단 넘게 겹쳐"),
    .init(note: "설정 파일이 FIFO 면 읽지 못한 것으로 막음", path: "/w/a", build: { f in f.repo("/w/a"); f.fifos.insert("/w/a/.git/config.worktree") },
          top: "/w/a", block: "git 설정(/w/a/.git/config.worktree)을 읽지 못해"),
    .init(note: ".gitmodules 만 있어도 막음", path: "/w/a", build: { $0.repo("/w/a"); $0.index["/w/a"] = ([], true) }, top: "/w/a", block: "작업 폴더에 하위 모듈(.gitmodules 파일)이 있어"),
    .init(note: "색인을 못 읽으면 막음(닫힌 쪽)", path: "/w/a", build: { $0.repo("/w/a"); $0.indexFail.insert("/w/a") }, top: "/w/a", block: "색인을 읽지 못해(git ls-files 실패)"),
    .init(note: "지난 실행 기록과 같은 git 폴더는 통과", path: "/w/a", records: ["/w/a": APGitRecord(commonDir: "/w/a/.git", commonIno: "1:/w/a/.git", workGitDir: "/gone", workGitIno: "9:9")],
          build: { $0.repo("/w/a") }, top: "/w/a", gitDir: "/w/a/.git"),
    .init(note: "지난 실행 기록과 inode 가 다르면 막음(바꿔치기)", path: "/w/a", records: ["/w/a": APGitRecord(commonDir: "/w/a/.git", commonIno: "1:2", workGitDir: nil, workGitIno: nil)],
          build: { $0.repo("/w/a") }, top: "/w/a", block: "git 폴더가 지난 실행이 출발할 때와 달라져"),
    .init(note: "지난 실행 기록의 git 폴더 자리가 다르면 막음", path: "/w/a", records: ["/w/a": APGitRecord(commonDir: "/w/old/.git", commonIno: nil, workGitDir: nil, workGitIno: nil)],
          build: { $0.repo("/w/a") }, top: "/w/a", block: "git 폴더가 지난 실행이 출발할 때와 달라져"),
]

@Suite("자율주행 출발 준비 git 자리 확인") struct PrepGitTests {
    @Test("뿌리·git 폴더·막는 까닭", arguments: rows)
    func 확인(_ r: PrepGitRow) {
        let fs = FakeGitFS()
        r.build(fs)
        let got = apPrepGitCheck(r.path, world: fs, records: r.records, home: r.home)
        #expect(got.top == r.top)
        #expect(got.block == r.block)
        if let g = r.gitDir { #expect(got.gitDir == g) }
        if r.block == nil, let t = r.top, let g = r.gitDir { #expect(got.pin == ["GIT_DIR=" + g, "GIT_WORK_TREE=" + t]) }
        if r.block != nil { #expect(got.pin.isEmpty, "막은 저장소는 박을 env 도 없다") }
    }

    @Test("막을 때 줄 글")
    func 글() {
        #expect(apPrepGitBlockText(top: "/h/w/nest/app", why: "다른 저장소(~/w/nest)의 작업 트리 안에 든 git 자리라", home: "/h")
                == "보안: ~/w/nest/app 의 다른 저장소(~/w/nest)의 작업 트리 안에 든 git 자리라 그 저장소에서 git 을 돌리지 않았습니다. 사람이 확인한 뒤 다시 읽습니다.")
        #expect(apPrepShortPath("/opt/x", home: "/h") == "/opt/x")
    }
}
