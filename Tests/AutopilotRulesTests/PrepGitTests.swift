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
    var pins: [String: [String]] = [:]   // 색인 읽기(submodules)에 받은 박는 env

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
    func submodules(top: String, pin: [String]) -> (gitlinks: [String], gitmodules: Bool)? {
        pins[top] = pin
        return indexFail.contains(top) ? nil : index[top] ?? ([], false)
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
    // 보안 검토 고침(다듬기 ⑬-2, MEDIUM 찾기가 git 과 다름): git 은 HEAD 가 refs/ 를 가리키는 바로 가기여도 저장소로 받는다 — 지나치지 않고 막는다
    .init(note: "HEAD 가 바로 가기인 .git(혼자 선 저장소)은 막음", path: "/w/hs", build: { f in
        f.repo("/w/hs"); f.files["/w/hs/.git/HEAD"] = nil; f.put("/w/hs/.git/refs/heads/main", "0"); f.links["/w/hs/.git/HEAD"] = "/w/hs/.git/refs/heads/main"
    }, top: "/w/hs", block: ".git/HEAD 가 보통 파일이 아니라(바로 가기 등)"),
    .init(note: "바깥 저장소 안 중첩 저장소의 HEAD 가 바로 가기면 바깥으로 지나치지 않고 막음", path: "/w/a/app/src", build: { f in
        f.repo("/w/a"); f.repo("/w/a/app"); f.mkdir("/w/a/app/src"); f.files["/w/a/app/.git/HEAD"] = nil
        f.put("/w/a/app/.git/refs/heads/main", "0"); f.links["/w/a/app/.git/HEAD"] = "/w/a/app/.git/refs/heads/main"
    }, top: "/w/a/app", block: ".git/HEAD 가 보통 파일이 아니라(바로 가기 등)"),
    .init(note: "HEAD 가 FIFO 인 .git 도 막음", path: "/w/hf", build: { f in
        f.repo("/w/hf"); f.files["/w/hf/.git/HEAD"] = nil; f.fifos.insert("/w/hf/.git/HEAD")
    }, top: "/w/hf", block: ".git/HEAD 가 보통 파일이 아니라(바로 가기 등)"),
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
    // 보안 검토 고침 8(재현: .git/commondir 가 저장소 밖에 심은 공통 git 폴더를 가리키고 그 config 에 clean filter — 옛 확인은 깨끗한 .git/config 만 보고
    // 지나갔고 보드 꼴 git status 가 filter 를 돌림, 표지 2). git 은 commondir 가 가리키는 자리의 config 를 읽는다
    .init(note: "보안 검토 고침 8: .git/commondir 가 다른 폴더(심은 config 에 filter)를 가리키면 막음", path: "/w/a", build: { f in
        f.repo("/w/a"); f.put("/w/a/.git/commondir", "/w/planted\n"); f.mkdir("/w/planted/objects"); f.mkdir("/w/planted/refs")
        f.cfg("/w/planted/config", [("filter.x.clean", "/w/plant.sh")])
    }, top: "/w/a", block: "git 폴더(/w/a/.git)에 commondir 가 있어 공통 git 폴더를 다른 자리로 돌려"),
    .init(note: "보안 검토 고침 8: commondir 가 그 git 폴더 자신을 가리켜도 막음(따로 작업 트리 밖의 commondir 는 받지 않음)", path: "/w/a", build: { f in
        f.repo("/w/a"); f.put("/w/a/.git/commondir", ".\n")
    }, top: "/w/a", block: "git 폴더(/w/a/.git)에 commondir 가 있어 공통 git 폴더를 다른 자리로 돌려"),
    .init(note: "보안 검토 고침 8: commondir 가 바로 가기여도 있음으로 보고 막음", path: "/w/a", build: { f in
        f.repo("/w/a"); f.mkdir("/w/planted"); f.links["/w/a/.git/commondir"] = "/w/planted"
    }, top: "/w/a", block: "git 폴더(/w/a/.git)에 commondir 가 있어 공통 git 폴더를 다른 자리로 돌려"),
    .init(note: "보안 검토 고침 8: 하위 모듈 git 폴더에 commondir 가 있으면 막음", path: "/w/s/sub", build: { f in
        f.repo("/w/s"); f.mkdir("/w/s/.git/modules/sub"); f.put("/w/s/.git/modules/sub/HEAD"); f.put("/w/s/sub/.git", "gitdir: ../.git/modules/sub")
        f.mkdir("/w/planted"); f.put("/w/s/.git/modules/sub/commondir", "/w/planted")
    }, top: "/w/s/sub", block: "하위 모듈 git 폴더(/w/s/.git/modules/sub)에 commondir 가 있어 공통 git 폴더를 다른 자리로 돌려"),
    .init(note: "하위 모듈 git 폴더가 그 저장소 밖이면 막음", path: "/w/t/sub", build: { f in
        f.repo("/w/s"); f.mkdir("/w/s/.git/modules/sub"); f.mkdir("/w/t"); f.put("/w/t/sub/.git", "gitdir: /w/s/.git/modules/sub")
    }, top: "/w/t/sub", block: "하위 모듈 git 폴더가 그 저장소 밖을 가리켜"),
    .init(note: "보안 검토 고침 2: 떨어진 git 폴더(--separate-git-dir 꼴)는 자리를 믿을 수 없어 막음(전에는 통과)", path: "/w/sep",
          build: { f in f.mkdir("/w/store/sep.git"); f.put("/w/store/sep.git/HEAD"); f.put("/w/sep/.git", "gitdir: /w/store/sep.git") },
          top: "/w/sep", block: "git 파일이 떨어진 git 폴더(/w/store/sep.git)를 가리켜 자리를 믿을 수 없어"),
    .init(note: "보안 검토 고침 1·2: .git 파일이 임시 자리 git 폴더를 가리키면 막음(전에는 통과, 그 설정의 clean filter 를 보드 status 가 돌림)", path: "/w/top", build: { f in
        f.repo("/tmp/evil"); f.cfg("/tmp/evil/.git/config", [("filter.x.clean", "/w/plant.sh")]); f.put("/w/top/.git", "gitdir: /tmp/evil/.git")
    }, top: "/w/top", block: "git 폴더(/tmp/evil/.git)가 임시 폴더·자율주행 작업 트리 자리에 있어"),
    .init(note: "보안 검토 고침 2: 따로 작업 트리의 git 폴더가 임시 자리면 막음", path: "/w/wt/x", build: { f in
        f.repo("/private/var/folders/q/m"); f.put("/w/wt/x/.git", "gitdir: /private/var/folders/q/m/.git/worktrees/x")
        f.put("/private/var/folders/q/m/.git/worktrees/x/gitdir", "/w/wt/x/.git"); f.put("/private/var/folders/q/m/.git/worktrees/x/commondir", "../..")
    }, top: "/w/wt/x", block: "git 폴더(/private/var/folders/q/m/.git/worktrees/x)가 임시 폴더·자율주행 작업 트리 자리에 있어"),
    .init(note: "보안 검토 고침 2: 자율주행 작업 트리 자리(~/.claude-board/worktrees) 안 git 폴더면 막음", path: "/w/q", build: { f in
        f.repo("/h/.claude-board/worktrees/r1/x"); f.put("/w/q/.git", "gitdir: /h/.claude-board/worktrees/r1/x/.git")
    }, top: "/w/q", block: "git 폴더(~/.claude-board/worktrees/r1/x/.git)가 임시 폴더·자율주행 작업 트리 자리에 있어"),
    .init(note: "보안 검토 고침 2: .git 파일이 그 작업 트리 안 git 폴더를 가리키면 막음", path: "/w/t", build: { f in f.repo("/w/t/.hide"); f.put("/w/t/.git", "gitdir: .hide/.git") },
          top: "/w/t", block: "git 폴더(/w/t/.hide/.git)가 그 작업 트리 안에 있어"),
    .init(note: "보안 검토 고침 1: .git 파일이 다른 곳의 저장소를 가리켜도 고른 폴더가 다른 저장소 작업 트리 안이면 막음(전에는 통과)", path: "/w/x/sub", build: { f in
        f.repo("/w/x"); f.repo("/w/other"); f.put("/w/x/sub/.git", "gitdir: /w/other/.git")
    }, top: "/w/x/sub", block: "다른 저장소(/w/x)의 작업 트리 안에 든 git 자리라"),
    .init(note: "자율주행 작업 트리 자리의 따로 작업 트리(git 폴더는 원래 저장소)는 통과", path: "/h/.claude-board/worktrees/r1/a", build: { f in
        f.repo("/w/a"); f.put("/h/.claude-board/worktrees/r1/a/.git", "gitdir: /w/a/.git/worktrees/r1")
        f.put("/w/a/.git/worktrees/r1/gitdir", "/h/.claude-board/worktrees/r1/a/.git"); f.put("/w/a/.git/worktrees/r1/commondir", "../..")
    }, top: "/h/.claude-board/worktrees/r1/a", gitDir: "/w/a/.git/worktrees/r1"),
    .init(note: "보안 검토 고침 3: 저장소 설정의 filter 명령(clean)은 막음(보드 status 가 돌림, 전에는 통과)", path: "/w/own", build: { f in
        f.repo("/w/own"); f.cfg("/w/own/.git/config", [("filter.x.clean", "/w/plant.sh flt")])
    }, top: "/w/own", block: "저장소 설정(/w/own/.git/config)에 filter 명령(filter.x.clean)이 있어"),
    .init(note: "보안 검토 고침 3: smudge·process 도 막음(대소문자 접음)", path: "/w/own", build: { f in
        f.repo("/w/own"); f.cfg("/w/own/.git/config", [("filter.Y.Process", "evil")])
    }, top: "/w/own", block: "저장소 설정(/w/own/.git/config)에 filter 명령(filter.y.process)이 있어"),
    .init(note: "git-lfs 표준 filter 는 통과", path: "/w/lfs", build: { f in
        f.repo("/w/lfs"); f.cfg("/w/lfs/.git/config", [("filter.lfs.clean", "git-lfs clean -- %f"), ("filter.lfs.smudge", "git-lfs smudge -- %f"),
                                                       ("filter.lfs.process", "git-lfs filter-process"), ("filter.lfs.required", "true")])
    }, top: "/w/lfs", gitDir: "/w/lfs/.git"),
    .init(note: "git-lfs 를 흉내 낸 다른 글은 막음", path: "/w/lfs", build: { f in f.repo("/w/lfs"); f.cfg("/w/lfs/.git/config", [("filter.lfs.clean", "git-lfs clean -- %f; touch x")]) },
          top: "/w/lfs", block: "저장소 설정(/w/lfs/.git/config)에 filter 명령(filter.lfs.clean)이 있어"),
    .init(note: "보드가 돌리는 git(ls-files·cat-file·symbolic-ref·for-each-ref·status·rev-list)이 부르지 않는 명령 키는 막지 않음(재현 안 됨)", path: "/w/own", build: { f in
        f.repo("/w/own"); f.cfg("/w/own/.git/config", [("diff.external", "x"), ("diff.y.textconv", "x"), ("core.pager", "x"), ("core.sshcommand", "x")])
    }, top: "/w/own", gitDir: "/w/own/.git"),
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
    .init(note: "보안 검토 고침 4: include 로 끌어온 범위 밖 파일의 filter 명령도 막음(보드 status 가 돌림, 전에는 뿌리 파일만 봄)", path: "/w/a", build: { f in
        f.repo("/w/a"); f.cfg("/w/a/.git/config", [("include.path", "~/team.cfg")]); f.cfg("/h/team.cfg", [("filter.z.clean", "/w/plant.sh")])
    }, top: "/w/a", block: "git 설정(~/team.cfg)에 filter 명령(filter.z.clean)이 있어"),
    .init(note: "보안 검토 고침 5: ~<사용자>/ 꼴 include 는 git 이 그 사용자 홈으로 풀어 우리 풀이와 달라 막음(전에는 상대 경로로 읽어 통과)", path: "/w/a", build: { f in
        f.repo("/w/a"); f.cfg("/w/a/.git/config", [("include.path", "~toy/team.cfg")]); f.cfg("/h/team.cfg", [("filter.z.clean", "/w/plant.sh")])
    }, top: "/w/a", block: "git 설정 include 경로(~toy/team.cfg)를 git 과 같게 풀 수 없어"),
    .init(note: "보안 검토 고침 5: %(prefix) 꼴 include 도 막음", path: "/w/a", build: { f in f.repo("/w/a"); f.cfg("/w/a/.git/config", [("includeif.gitdir:/w/.path", "%(prefix)/etc/x.cfg")]) },
          top: "/w/a", block: "git 설정 include 경로(%(prefix)/etc/x.cfg)를 git 과 같게 풀 수 없어"),
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
        if r.block == nil, let t = r.top, let g = r.gitDir { #expect(got.pin == ["GIT_DIR=" + g, "GIT_COMMON_DIR=" + (got.commonDir ?? g), "GIT_WORK_TREE=" + t] + apPrepGitReadEnv) }
        if r.block != nil { #expect(got.pin.isEmpty, "막은 저장소는 박을 env 도 없다") }
    }

    @Test("보안 검토 고침 8: 박는 env 에 확인한 공통 git 폴더(따로 작업 트리면 그 저장소 git 폴더), 색인 읽기도 같은 env")
    func 공통폴더박기() {
        let f = FakeGitFS()
        f.repo("/w/a"); f.put("/w/wt/x/.git", "gitdir: /w/a/.git/worktrees/x\n")
        f.put("/w/a/.git/worktrees/x/gitdir", "/w/wt/x/.git\n"); f.put("/w/a/.git/worktrees/x/commondir", "../..\n"); f.put("/w/a/.git/worktrees/x/HEAD")
        let wt = apPrepGitCheck("/w/wt/x", world: f, home: "/h")
        #expect(wt.block == nil)
        #expect(wt.pin == ["GIT_DIR=/w/a/.git/worktrees/x", "GIT_COMMON_DIR=/w/a/.git", "GIT_WORK_TREE=/w/wt/x"] + apPrepGitReadEnv)
        #expect(f.pins["/w/wt/x"] == wt.pin)
        let plain = apPrepGitCheck("/w/a", world: f, home: "/h")
        #expect(plain.pin == ["GIT_DIR=/w/a/.git", "GIT_COMMON_DIR=/w/a/.git", "GIT_WORK_TREE=/w/a"] + apPrepGitReadEnv)
        #expect(f.pins["/w/a"] == plain.pin)
    }

    @Test("보안 검토 고침 6: 읽기 env 는 늦은 받기를 끄고 전송 프로토콜을 막는다")
    func 읽기env() {
        #expect(apPrepGitReadEnv == ["GIT_NO_LAZY_FETCH=1", "GIT_CONFIG_COUNT=1", "GIT_CONFIG_KEY_0=protocol.allow", "GIT_CONFIG_VALUE_0=never"])
    }

    @Test("보안 검토 고침 7: 출발 준비 읽기 환경은 물려받은 GIT_* 를 모두 빼고 보호·박기만 넣는다")
    func 물려받은env() {
        let inherited = ["PATH": "/usr/bin:/bin", "HOME": "/h", "GIT_CONFIG_PARAMETERS": "'filter.q.clean'='/w/plant.sh'", "GIT_CONFIG_COUNT": "2",
                         "GIT_CONFIG_KEY_0": "core.fsmonitor", "GIT_CONFIG_VALUE_0": "/w/plant.sh", "GIT_CONFIG_KEY_1": "x", "GIT_CONFIG_VALUE_1": "y",
                         "GIT_SSH_COMMAND": "/w/plant.sh", "GIT_ALLOW_PROTOCOL": "file", "GIT_INDEX_FILE": "/tmp/i", "GIT_DIR": "/elsewhere"]
        let pin = APPrepGit(top: "/w/a", gitDir: "/w/a/.git", commonDir: "/w/a/.git").pin
        let env = apPrepGitEnv(inherited: inherited, extra: pin)
        #expect(env == ["PATH": "/usr/bin:/bin", "HOME": "/h", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_ATTR_NOSYSTEM": "1",
                        "GIT_NO_REPLACE_OBJECTS": "1", "GIT_DIR": "/w/a/.git", "GIT_COMMON_DIR": "/w/a/.git", "GIT_WORK_TREE": "/w/a", "GIT_NO_LAZY_FETCH": "1",
                        "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "protocol.allow", "GIT_CONFIG_VALUE_0": "never"])
    }

    @Test("막을 때 줄 글")
    func 글() {
        #expect(apPrepGitBlockText(top: "/h/w/nest/app", why: "다른 저장소(~/w/nest)의 작업 트리 안에 든 git 자리라", home: "/h")
                == "보안: ~/w/nest/app 의 다른 저장소(~/w/nest)의 작업 트리 안에 든 git 자리라 그 저장소에서 git 을 돌리지 않았습니다. 사람이 확인한 뒤 다시 읽습니다.")
        #expect(apPrepShortPath("/opt/x", home: "/h") == "/opt/x")
    }
}
