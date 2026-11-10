// 셸 명령 글자를 낱말 단위로 읽고 고치는 규칙 — 보드가 탭 셸에 치는 claude 명령(보드가 만든 명령·state.py 의 resume_cmd·사용자가 친 명령·
// 바깥에서 탭 열기)에 답변 꾸미기(--plugin-dir …/plugins/md-render)를 빼고 붙이고, 세션 이름 --name 을 끼우고, --session-id 를 바꾼다.
// 3.3.2 까지는 따옴표로 묶인 글자 위에서 정규식으로 지우고 끼워, 첫 요청 글 같은 묶인 낱말 안을 지우거나 따옴표 경계를 지워 셸이 다르게 읽을 수 있었다
// (보안 검토 [높음] — 되살리기의 --session-id 지우기가 첫 요청 글 끝의 닫는 ' 를 함께 지워 다음 인자가 명령이 됨). 그래서 낱말을 원문 자리와 함께 읽고
// 낱말 범위만 지우거나 그 사이에 끼운 뒤(남긴 낱말은 원문 글자 그대로라 따옴표 꼴이 바뀌지 않는다), 고친 글자를 다시 읽어 뜻한 낱말과 같은지 확인한다.
// 셸은 바이트로 읽으므로 글자(Character, 낱자 묶음)가 아니라 유니코드 스칼라로 읽는다 — ' 바로 뒤에 결합 문자가 오면 Character 로는 ' 가 보이지 않는다
// (3.3.2 의 shlexQuote 가 이 때문에 ' 를 바꾸지 못해 바깥 열기 첫 요청 글로 명령을 끼워 넣을 수 있었다)
import Foundation

// MARK: - 따옴표

/// state.py 가 resume_cmd 를 만들 때 쓰는 shlex.quote 와 같은 인용(보드 EmbeddedTerminal.swift 에서 옮김) — resume_cmd 안의 토큰을 그대로 찾고,
/// 보드가 셸에 치는 인자를 안전하게 넘긴다. ' 는 글자 그대로(.literal) 찾는다 — 기본 비교는 ' + 결합 문자를 한 글자로 보아 지나친다
public func shlexQuote(_ s: String) -> String {
    if !s.isEmpty, s.range(of: "[^A-Za-z0-9_@%+=:,./-]", options: .regularExpression) == nil { return s }
    return "'" + s.replacingOccurrences(of: "'", with: "'\"'\"'", options: .literal) + "'"
}

/// 보드가 셸에 새로 치는 인자 하나 — shlexQuote 꼴에, 따옴표 없이 = 로 시작하면 묶는다(zsh 는 =이름 을 그 명령의 경로로 펼친다)
public func shellArg(_ s: String) -> String {
    let q = shlexQuote(s)
    return q.hasPrefix("=") ? "'" + q + "'" : q
}

// MARK: - 낱말 읽기

/// 셸 낱말 하나 — 따옴표·\ 를 푼 값과 원문 자리(유니코드 스칼라 경계, s.unicodeScalars[range] 로 읽는다)
public struct ShellWord: Equatable, Sendable {
    public let text: String
    public let range: Range<String.Index>
    /// 셸이 이 값을 글자 그대로 넘긴다 — 따옴표 밖이 shlexQuote 의 안전한 글자뿐이고(맨 앞 = 는 아님), 큰따옴표 안에 $ ` ! 가 없다
    public let literal: Bool
    public init(text: String, range: Range<String.Index>, literal: Bool) { self.text = text; self.range = range; self.literal = literal }
    public func raw(in s: String) -> String { String(s.unicodeScalars[range]) }
}

/// 셸 명령 한 줄을 읽은 것
public struct ShellLine: Equatable, Sendable {
    public let words: [ShellWord]
    /// 셸이 이 낱말들을 한 명령의 인자로 글자 그대로 넘긴다 — 모든 낱말이 literal 이고 낱말 사이가 공백·탭뿐(따옴표 밖 줄바꿈 없음)
    public let plain: Bool
    public var texts: [String] { words.map(\.text) }
}

private let shellSafe = Set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_@%+=:,./-".unicodeScalars)
private func isBlank(_ c: Unicode.Scalar) -> Bool { c == " " || c == "\t" || c == "\n" }

/// 셸 명령 글자를 낱말로 — 보드가 남긴 명령(shlex.quote 꼴)과 사람이 적은 명령의 흔한 꼴: 공백·탭·줄바꿈으로 나누고 '…'(그대로)·"…"(\ 는 " \ $ ` 줄바꿈 앞에서만)·
/// 따옴표 밖 \ 를 푼다(\줄바꿈 은 이어 쓰기라 둘 다 없어진다). 따옴표가 닫히지 않았거나 끝이 \ 이면 nil(뒤에 붙인 글자를 삼키므로).
/// 변수·명령 치환·연산자(; | & 등)는 풀지 않고 글자 그대로 낱말에 두며 그 낱말은 literal 이 아니다
public func shellLine(_ s: String) -> ShellLine? {
    let u = s.unicodeScalars
    var words: [ShellWord] = []
    var plain = true
    var cur = String.UnicodeScalarView(), start: String.Index?, literal = true
    var i = u.startIndex
    func begin(_ at: String.Index) { if start == nil { start = at } }
    while i < u.endIndex {
        let c = u[i]
        switch c {
        case " ", "\t", "\n":
            if let st = start {
                words.append(ShellWord(text: String(cur), range: st..<i, literal: literal))
                cur = String.UnicodeScalarView(); start = nil; literal = true
            }
            if c == "\n" { plain = false }
            i = u.index(after: i)
        case "'":
            begin(i)
            var j = u.index(after: i)
            while j < u.endIndex, u[j] != "'" { cur.append(u[j]); j = u.index(after: j) }
            guard j < u.endIndex else { return nil }
            i = u.index(after: j)
        case "\"":
            begin(i)
            var j = u.index(after: i)
            while true {
                guard j < u.endIndex else { return nil }
                let d = u[j]
                if d == "\"" { break }
                if d == "\\" {
                    let k = u.index(after: j)
                    if k < u.endIndex, u[k] == "\"" || u[k] == "\\" || u[k] == "$" || u[k] == "`" || u[k] == "\n" {
                        if u[k] != "\n" { cur.append(u[k]) }
                        j = u.index(after: k)
                        continue
                    }
                }
                if d == "$" || d == "`" || d == "!" { literal = false }
                cur.append(d)
                j = u.index(after: j)
            }
            i = u.index(after: j)
        case "\\":
            let k = u.index(after: i)
            guard k < u.endIndex else { return nil }
            if u[k] == "\n" { i = u.index(after: k); continue }   // 이어 쓰기
            begin(i)
            cur.append(u[k])
            i = u.index(after: k)
        default:
            if start == nil {
                start = i
                if c == "=" { literal = false }
            }
            if !shellSafe.contains(c) { literal = false }
            cur.append(c)
            i = u.index(after: i)
        }
    }
    if let st = start { words.append(ShellWord(text: String(cur), range: st..<u.endIndex, literal: literal)) }
    return ShellLine(words: words, plain: plain && words.allSatisfy(\.literal))
}

/// 셸 명령 글자를 낱말 값으로(shellLine 의 값만). 읽지 못하면 nil
public func shellWords(_ s: String) -> [String]? { shellLine(s)?.texts }

/// 인자 목록을 셸 명령 글자로 — 인자마다 shellArg 로 묶어 공백으로 잇고, 다시 읽어 셸이 그 인자 그대로 넘기는지(plain) 확인한다. 아니면 nil
public func shellCommand(_ argv: [String]) -> String? {
    let s = argv.map(shellArg).joined(separator: " ")
    guard let line = shellLine(s), line.plain, line.texts == argv else { return nil }
    return s
}

// MARK: - 낱말 단위로 고치기

/// 낱말 단위 고치기 — remove 의 낱말(차례 번호)을 앞 공백과 함께 지우고(이어진 차례는 한 덩어리로, oneSpace 면 덩어리 앞 공백은 한 글자만 —
/// 3.3.2 의 --session-id 지우기와 같은 꼴), afterFirst 를 첫 낱말 바로 뒤에, append 를 끝에 끼운다(값마다 shellArg 로 묶고 앞에 공백 하나).
/// 남긴 낱말과 그 사이 공백은 원문 글자 그대로다. 고친 글자를 다시 읽어 (남긴 낱말 + 끼운 값)과 같지 않으면 nil
public func shellEdit(_ s: String, _ line: ShellLine, remove: Set<Int> = [], oneSpace: Bool = false,
                      afterFirst: [String] = [], append: [String] = []) -> String? {
    let u = s.unicodeScalars
    let words = line.words
    guard !words.isEmpty || afterFirst.isEmpty else { return nil }
    // 지울 덩어리 [앞 공백 시작, 마지막 낱말 끝)
    var cuts: [Range<String.Index>] = []
    var k = 0
    while k < words.count {
        guard remove.contains(k) else { k += 1; continue }
        var last = k
        while remove.contains(last + 1), last + 1 < words.count { last += 1 }
        var lead = words[k].range.lowerBound
        let floor = k > 0 ? words[k - 1].range.upperBound : u.startIndex
        if oneSpace {
            if lead > floor, isBlank(u[u.index(before: lead)]) { lead = u.index(before: lead) }
        } else {
            while lead > floor, isBlank(u[u.index(before: lead)]) { lead = u.index(before: lead) }
        }
        cuts.append(lead..<words[last].range.upperBound)
        k = last + 1
    }
    let insert = afterFirst.isEmpty ? "" : " " + afterFirst.map(shellArg).joined(separator: " ")
    var out = String.UnicodeScalarView()
    var at = u.startIndex
    if !insert.isEmpty {
        let firstEnd = words[0].range.upperBound
        out.append(contentsOf: u[at..<firstEnd])
        out.append(contentsOf: insert.unicodeScalars)
        at = firstEnd
    }
    for cut in cuts {
        if cut.lowerBound > at { out.append(contentsOf: u[at..<cut.lowerBound]) }
        at = max(at, cut.upperBound)
    }
    out.append(contentsOf: u[at...])
    if !append.isEmpty { out.append(contentsOf: (" " + append.map(shellArg).joined(separator: " ")).unicodeScalars) }
    let result = String(out)
    var want = words.enumerated().filter { !remove.contains($0.offset) }.map(\.element.text)
    if !afterFirst.isEmpty { want.insert(contentsOf: afterFirst, at: 1) }
    want += append
    guard shellLine(result)?.texts == want else { return nil }
    return result
}

/// claude 명령이면 그 낱말 — 첫 낱말이 따옴표 없는 claude(경로 꼴 …/claude 도)이고 읽을 수 있을 때만. 아니면 nil
public func claudeLine(_ command: String) -> ShellLine? {
    guard let line = shellLine(command), let first = line.words.first,
          first.raw(in: command).range(of: #"^(\S*/)?claude$"#, options: .regularExpression) != nil else { return nil }
    return line
}

/// 답변 꾸미기(md-render)의 --plugin-dir 낱말 차례 — "--plugin-dir <…/plugins/md-render>" 두 낱말과 "--plugin-dir=…/plugins/md-render" 한 낱말.
/// 첫 낱말(claude)은 보지 않는다. 사용자의 다른 --plugin-dir 은 두고, 첫 요청 글처럼 묶인 낱말 안의 글자는 낱말 값 전체가 아니면 건드리지 않는다
public func mdRenderArgIndices(_ args: [String]) -> Set<Int> {
    var out = Set<Int>()
    var i = 1
    while i < args.count {
        if args[i] == "--plugin-dir", i + 1 < args.count, args[i + 1].hasSuffix("/plugins/md-render") {
            out.formUnion([i, i + 1]); i += 2; continue
        }
        if args[i].hasPrefix("--plugin-dir="), args[i].hasSuffix("/plugins/md-render") { out.insert(i) }
        i += 1
    }
    return out
}

/// 보드 탭 claude 의 답변 꾸미기(사용자 결정 2026-10-17) — 명령에 든 md-render 의 --plugin-dir(옛 경로·옛 설치본·사용자가 적은 것)을 빼고,
/// dir 이 있으면(켬) 그 경로를 끝에 한 번 붙인다. claude 가 아니거나 읽지 못하는 명령은 그대로
public func withMdRender(_ command: String, dir: String?) -> String {
    guard let line = claudeLine(command) else { return command }
    return shellEdit(command, line, remove: mdRenderArgIndices(line.texts), append: dir.map { ["--plugin-dir", $0] } ?? []) ?? command
}

/// 새로 띄우는 claude 에 세션 id 를 붙인다(--session-id <newID>, 끝에) — 보드가 그 세션을 미리 안다. 이미 --session-id 가 있으면 빼고 새 id 로
/// (되살리기가 대화 없이 끝난 세션을 다시 띄울 때). claude 가 아니거나 이어 열기·이어 하기(--resume·-r·--continue·-c)거나 읽지 못하면 그대로·nil
public func withFreshSession(_ command: String, newID: String) -> (String, String?) {
    guard let line = claudeLine(command) else { return (command, nil) }
    let texts = line.texts
    let resumes = texts.dropFirst().contains { t in ["--resume", "-r", "--continue", "-c"].contains { t == $0 || t.hasPrefix($0 + "=") } }
    guard !resumes else { return (command, nil) }
    var remove = Set<Int>()
    var i = 1
    while i < texts.count {
        if texts[i] == "--session-id", i + 1 < texts.count { remove.formUnion([i, i + 1]); i += 2; continue }
        if texts[i].hasPrefix("--session-id=") { remove.insert(i) }
        i += 1
    }
    guard let out = shellEdit(command, line, remove: remove, oneSpace: true, append: ["--session-id", newID]) else { return (command, nil) }
    return (out, newID)
}

/// claude 명령에 세션 이름 --name <이름> 을 claude 바로 뒤에 끼운다(--resume 과 함께 주면 --name 이 이긴다 — 실측). claude 가 아니거나
/// 이미 -n·--name 이 있거나 읽지 못하면 그대로이고 name 은 부르지 않는다(보드가 이름을 고르며 잡아 두므로)
public func withSessionName(_ command: String, name: () -> String) -> String {
    guard let line = claudeLine(command) else { return command }
    guard !line.texts.dropFirst().contains(where: { t in ["-n", "--name"].contains { t == $0 || t.hasPrefix($0 + "=") } }) else { return command }
    return shellEdit(command, line, afterFirst: ["--name", name()]) ?? command
}
