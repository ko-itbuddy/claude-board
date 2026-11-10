// 셸 명령 낱말 단위 고치기(ShellCommand.swift) 시험 — 3.3.3 보안 고침.
// 보통 줄의 기대값은 고치기 전(3.3.2) 보드 EmbeddedTerminal.swift 의 mdRenderCommand·freshSessionCommand·named 를 그대로 옮겨 돌린 덤프다
// (md-render 경로 /Users/u/.claude/notifier/plugins/md-render, 새 id 1111…, 이름 "역할 1"). 일부러 달라진 줄은 note 에 "3.3.3" 과 까닭.
// 공격 줄은 첫 요청 글(인자 하나)에 셸이 다르게 읽을 글자를 넣은 것 — 고친 뒤에는 결과를 다시 읽은 낱말이 뜻한 인자 그대로여야 한다
import Foundation
import Testing
import BoardRules

struct ShellCommandTests {
    static let dir = "/Users/u/.claude/notifier/plugins/md-render"
    static let newID = "11111111-2222-3333-4444-555555555555"
    static let sid = "0f0e1d2c-3b4a-5968-7766-554433221100"
    static let md = " --plugin-dir \(dir)"
    static let fresh = " --session-id \(newID)"

    // MARK: 따옴표

    @Test("shlexQuote 는 ' 뒤에 결합 문자가 와도 ' 를 바꾼다(3.3.3 — 3.3.2 는 Character 비교로 지나쳐 따옴표가 깨졌다)")
    func quoteCombining() {
        let s = "a'\u{301}; touch /tmp/x; '\u{301}b"
        let q = shlexQuote(s)
        #expect(q.unicodeScalars.elementsEqual("'a'\"'\"'\u{301}; touch /tmp/x; '\"'\"'\u{301}b'".unicodeScalars))
        #expect(shellLine(q)?.texts == [s])
        #expect(shellLine(q)?.plain == true)
    }

    @Test("shellArg 는 맨 앞 = 를 묶는다(zsh 의 =이름 펼침), 나머지는 shlexQuote 그대로")
    func arg() {
        #expect(shellArg("=x") == "'=x'")
        #expect(shellArg("a=x") == "a=x")
        #expect(shellArg("it's") == shlexQuote("it's"))
        #expect(shellArg("=a b") == "'=a b'")
    }

    // MARK: 낱말 읽기

    @Test("shellLine 은 낱말 값과 원문 자리를 함께 읽는다")
    func spans() throws {
        let s = #"  claude --name 'it'"'"'s' "a \"b\" \$c" d\ e '' x"#
        let line = try #require(shellLine(s))
        #expect(line.texts == ["claude", "--name", "it's", #"a "b" $c"#, "d e", "", "x"])
        #expect(line.words.map { $0.raw(in: s) } == ["claude", "--name", #"'it'"'"'s'"#, #""a \"b\" \$c""#, #"d\ e"#, "''", "x"])
        #expect(line.plain)
    }

    struct LineRow: Sendable, CustomTestStringConvertible {
        let note: String
        let line: String
        let texts: [String]?
        let plain: Bool
        var testDescription: String { note }
    }

    @Test("shellLine 이 읽지 못하는 꼴·셸이 펼치는 꼴", arguments: [
        LineRow(note: "닫히지 않은 작은따옴표", line: "claude '열린", texts: nil, plain: false),
        LineRow(note: "닫히지 않은 큰따옴표", line: #"claude "열린"#, texts: nil, plain: false),
        LineRow(note: "끝의 \\ 는 뒤에 붙일 글자를 삼키므로 실패", line: #"claude x\"#, texts: nil, plain: false),
        LineRow(note: "\\줄바꿈 은 이어 쓰기(둘 다 없어짐)", line: "claude a\\\nb \\\n c", texts: ["claude", "ab", "c"], plain: true),
        LineRow(note: "따옴표 밖 ; 는 낱말에 남지만 plain 이 아니다", line: "claude a;touch x", texts: ["claude", "a;touch", "x"], plain: false),
        LineRow(note: "따옴표 밖 $( ) 도", line: "claude $(id)", texts: ["claude", "$(id)"], plain: false),
        LineRow(note: "큰따옴표 안 $ 도", line: #"claude "$HOME""#, texts: ["claude", "$HOME"], plain: false),
        LineRow(note: "큰따옴표 안 ! 도(대화형 셸의 기록 펼침)", line: #"claude "a!b""#, texts: ["claude", "a!b"], plain: false),
        LineRow(note: "맨 앞 = (zsh)", line: "claude =ls", texts: ["claude", "=ls"], plain: false),
        LineRow(note: "가운데 = 는 됨", line: "claude --a=b", texts: ["claude", "--a=b"], plain: true),
        LineRow(note: "따옴표 밖 ~ 는 펼침", line: "claude ~/x", texts: ["claude", "~/x"], plain: false),
        LineRow(note: "따옴표 밖 줄바꿈은 명령을 나눈다", line: "claude a\ntouch x", texts: ["claude", "a", "touch", "x"], plain: false),
        LineRow(note: "따옴표 밖 한글은 plain 이 아니다(shlexQuote 는 묶는다)", line: "claude 한글", texts: ["claude", "한글"], plain: false),
        LineRow(note: "' 뒤 결합 문자: 스칼라로 읽어 ' 를 본다", line: "claude 'a'\u{301}; x", texts: ["claude", "a\u{301};", "x"], plain: false),
    ])
    func lines(_ r: LineRow) {
        let line = shellLine(r.line)
        #expect(line?.texts == r.texts)
        if r.texts != nil { #expect(line?.plain == r.plain) }
    }

    // MARK: 보통 명령 — 3.3.2 출력과 같다

    struct EditRow: Sendable, CustomTestStringConvertible {
        let note: String
        let input: String
        let md: String      // withMdRender(dir) — 켬
        let off: String     // withMdRender(nil) — 끔
        let fresh: String?  // withFreshSession — nil 이면 그대로·id 없음
        let named: String   // withSessionName("역할 1")
        var testDescription: String { note }
    }

    static let s = sid
    @Test("보통 명령은 3.3.2 와 같은 글자", arguments: [
        EditRow(note: "claude 만", input: "claude", md: "claude" + md, off: "claude", fresh: "claude" + fresh, named: "claude --name '역할 1'"),
        EditRow(note: "옵션", input: "claude --model opus", md: "claude --model opus" + md, off: "claude --model opus",
                fresh: "claude --model opus" + fresh, named: "claude --name '역할 1' --model opus"),
        EditRow(note: "경로 꼴 claude", input: "/opt/bin/claude --verbose", md: "/opt/bin/claude --verbose" + md, off: "/opt/bin/claude --verbose",
                fresh: "/opt/bin/claude --verbose" + fresh, named: "/opt/bin/claude --name '역할 1' --verbose"),
        EditRow(note: "공백 여럿은 그대로", input: "  claude  --model  opus", md: "  claude  --model  opus" + md, off: "  claude  --model  opus",
                fresh: "  claude  --model  opus" + fresh, named: "  claude --name '역할 1'  --model  opus"),
        EditRow(note: "claude 가 아니면 그대로", input: "bash -lc 'claude x'", md: "bash -lc 'claude x'", off: "bash -lc 'claude x'", fresh: nil, named: "bash -lc 'claude x'"),
        EditRow(note: "claude- 로 시작하는 다른 명령", input: "claude-x --y", md: "claude-x --y", off: "claude-x --y", fresh: nil, named: "claude-x --y"),
        EditRow(note: "옛 md-render 경로를 지금 경로로", input: "claude --resume \(s) --plugin-dir /old/plugins/md-render",
                md: "claude --resume \(s)" + md, off: "claude --resume \(s)", fresh: nil,
                named: "claude --name '역할 1' --resume \(s) --plugin-dir /old/plugins/md-render"),
        EditRow(note: "resume_cmd 의 묶인 앱 안 경로", input: "claude --model opus --plugin-dir '/Applications/Claude Board.app/Contents/Resources/plugins/md-render' --resume \(s)",
                md: "claude --model opus --resume \(s)" + md, off: "claude --model opus --resume \(s)", fresh: nil,
                named: "claude --name '역할 1' --model opus --plugin-dir '/Applications/Claude Board.app/Contents/Resources/plugins/md-render' --resume \(s)"),
        EditRow(note: "'--plugin-dir=…' 꼴", input: "claude '--plugin-dir=/a b/plugins/md-render' --resume \(s)",
                md: "claude --resume \(s)" + md, off: "claude --resume \(s)", fresh: nil,
                named: "claude --name '역할 1' '--plugin-dir=/a b/plugins/md-render' --resume \(s)"),
        EditRow(note: "--plugin-dir=… 꼴", input: "claude --plugin-dir=/a/plugins/md-render --resume \(s)",
                md: "claude --resume \(s)" + md, off: "claude --resume \(s)", fresh: nil,
                named: "claude --name '역할 1' --plugin-dir=/a/plugins/md-render --resume \(s)"),
        EditRow(note: "사용자의 다른 --plugin-dir 은 둔다", input: "claude --plugin-dir /my/plugin --resume \(s)",
                md: "claude --plugin-dir /my/plugin --resume \(s)" + md, off: "claude --plugin-dir /my/plugin --resume \(s)", fresh: nil,
                named: "claude --name '역할 1' --plugin-dir /my/plugin --resume \(s)"),
        EditRow(note: "새 세션 명령(이름·첫 요청)", input: "claude --session-id \(s) --name '역할 2' '첫 요청'",
                md: "claude --session-id \(s) --name '역할 2' '첫 요청'" + md, off: "claude --session-id \(s) --name '역할 2' '첫 요청'",
                fresh: "claude --name '역할 2' '첫 요청'" + fresh, named: "claude --session-id \(s) --name '역할 2' '첫 요청'"),
        EditRow(note: "이어서 진행 요청('\\'' 꼴)", input: "claude --dangerously-skip-permissions --model opus --resume \(s) '이어서 진행해 주세요. 끊긴 곳부터'\\''요'",
                md: "claude --dangerously-skip-permissions --model opus --resume \(s) '이어서 진행해 주세요. 끊긴 곳부터'\\''요'" + md,
                off: "claude --dangerously-skip-permissions --model opus --resume \(s) '이어서 진행해 주세요. 끊긴 곳부터'\\''요'", fresh: nil,
                named: "claude --name '역할 1' --dangerously-skip-permissions --model opus --resume \(s) '이어서 진행해 주세요. 끊긴 곳부터'\\''요'"),
        EditRow(note: "--session-id 바꾸기", input: "claude --model opus --session-id \(s)", md: "claude --model opus --session-id \(s)" + md,
                off: "claude --model opus --session-id \(s)", fresh: "claude --model opus" + fresh, named: "claude --name '역할 1' --model opus --session-id \(s)"),
        EditRow(note: "--session-id= 바꾸기", input: "claude --session-id=\(s) --verbose", md: "claude --session-id=\(s) --verbose" + md,
                off: "claude --session-id=\(s) --verbose", fresh: "claude --verbose" + fresh, named: "claude --name '역할 1' --session-id=\(s) --verbose"),
        EditRow(note: "-c 는 이어 하기", input: "claude -c", md: "claude -c" + md, off: "claude -c", fresh: nil, named: "claude --name '역할 1' -c"),
        EditRow(note: "--continue", input: "claude --continue --model opus", md: "claude --continue --model opus" + md, off: "claude --continue --model opus",
                fresh: nil, named: "claude --name '역할 1' --continue --model opus"),
        EditRow(note: "고르기 -r", input: "claude -r", md: "claude -r" + md, off: "claude -r", fresh: nil, named: "claude --name '역할 1' -r"),
        EditRow(note: "이미 --name", input: "claude --name 역할 --resume \(s)", md: "claude --name 역할 --resume \(s)" + md, off: "claude --name 역할 --resume \(s)",
                fresh: nil, named: "claude --name 역할 --resume \(s)"),
        EditRow(note: "-n= 꼴", input: "claude -n=x", md: "claude -n=x" + md, off: "claude -n=x", fresh: "claude -n=x" + fresh, named: "claude -n=x"),
        EditRow(note: "값 없는 --session-id 는 둔다", input: "claude --session-id", md: "claude --session-id" + md, off: "claude --session-id",
                fresh: "claude --session-id" + fresh, named: "claude --name '역할 1' --session-id"),
        EditRow(note: "묶인 --add-dir", input: "claude --add-dir 'a b' c --mcp-config m.json", md: "claude --add-dir 'a b' c --mcp-config m.json" + md,
                off: "claude --add-dir 'a b' c --mcp-config m.json", fresh: "claude --add-dir 'a b' c --mcp-config m.json" + fresh,
                named: "claude --name '역할 1' --add-dir 'a b' c --mcp-config m.json"),
        EditRow(note: "3.3.3: 큰따옴표로 적은 md-render 도 낱말이면 빼고 지금 경로(3.3.2 는 지우지 못해 그대로 두고 붙이지 않음)",
                input: #"claude --plugin-dir "/a/plugins/md-render" --resume \#(s)"#, md: "claude --resume \(s)" + md, off: "claude --resume \(s)", fresh: nil,
                named: #"claude --name '역할 1' --plugin-dir "/a/plugins/md-render" --resume \#(s)"#),
        EditRow(note: "3.3.3: 읽지 못하는 명령(닫히지 않은 따옴표)은 그대로(3.3.2 는 끝에 붙여 따옴표 안에 들어감)", input: "claude '열린",
                md: "claude '열린", off: "claude '열린", fresh: nil, named: "claude '열린"),
    ])
    func edits(_ r: EditRow) {
        #expect(withMdRender(r.input, dir: Self.dir) == r.md)
        #expect(withMdRender(r.input, dir: nil) == r.off)
        let (f, id) = withFreshSession(r.input, newID: Self.newID)
        #expect(f == (r.fresh ?? r.input))
        #expect(id == (r.fresh == nil ? nil : Self.newID))
        #expect(withSessionName(r.input) { "역할 1" } == r.named)
    }

    @Test("이름이 이미 있거나 claude 가 아니면 이름을 고르지 않는다")
    func nameNotPicked() {
        var asked = 0
        _ = withSessionName("claude -n x") { asked += 1; return "y" }
        _ = withSessionName("bash") { asked += 1; return "y" }
        #expect(asked == 0)
    }

    // MARK: 공격 줄 — 첫 요청 글에 셸이 다르게 읽을 글자

    struct AttackRow: Sendable, CustomTestStringConvertible {
        let note: String
        let argv: [String]   // 바깥 열기 요청 인자(claude 뒤에 --session-id 를 붙인 뒤)
        var testDescription: String { note }
    }

    static let attacks: [AttackRow] = [
        AttackRow(note: "첫 요청 글 안의 --plugin-dir '…/plugins/md-render'(따옴표 꼴)",
                  argv: ["claude", "--session-id", sid, "x --plugin-dir '/x/plugins/md-render' ; touch /tmp/cb-pwned #"]),
        AttackRow(note: "첫 요청 글 안의 --plugin-dir …/plugins/md-render(따옴표 없는 꼴)",
                  argv: ["claude", "--session-id", sid, "보세요 --plugin-dir /x/plugins/md-render 끝"]),
        AttackRow(note: "'\"'\"' 꼴을 글로", argv: ["claude", "--session-id", sid, #"a '"'"' --plugin-dir='/x/plugins/md-render' b"#]),
        AttackRow(note: "글 끝의 --session-id 값이 닫는 따옴표를 삼키게(3.3.2 되살리기에서 다음 인자가 명령이 됨)",
                  argv: ["claude", "--session-id", sid, "요약해 줘 --session-id x", "; touch /tmp/cb-pwned #"]),
        AttackRow(note: "--session-id= 꼴", argv: ["claude", "--session-id", sid, "a --session-id=x", "; touch /tmp/cb-pwned #"]),
        AttackRow(note: "--name 글", argv: ["claude", "--session-id", sid, "use -n flag --name 'x' ; touch /tmp/cb-pwned"]),
        AttackRow(note: "; 와 $(…)", argv: ["claude", "--session-id", sid, "$(touch /tmp/cb-pwned); `id` && echo"]),
        AttackRow(note: "' 뒤 결합 문자(3.3.2 shlexQuote 가 ' 를 못 바꿈)", argv: ["claude", "--session-id", sid, "요약 '\u{301}; touch /tmp/cb-pwned; : '\u{301}"]),
        AttackRow(note: "--resume 글은 이어 열기가 아니다", argv: ["claude", "--session-id", sid, "--resume 하지 마 -c"]),
        AttackRow(note: "줄바꿈·탭이 든 글", argv: ["claude", "--session-id", sid, "첫 줄\n; touch /tmp/cb-pwned\t#"]),
    ]

    @Test("공격 줄: 바깥 열기 명령은 요청 인자 그대로 읽히고 md-render 만 끝에 붙는다", arguments: attacks)
    func openTabAttack(_ r: AttackRow) throws {
        let c = try openTabCommand(r.argv, mdRenderDir: Self.dir).get()
        #expect(shellLine(c)?.plain == true)
        #expect(shellLine(c)?.texts == r.argv + ["--plugin-dir", Self.dir])
        let off = try openTabCommand(r.argv, mdRenderDir: nil).get()
        #expect(shellLine(off)?.texts == r.argv)
    }

    @Test("공격 줄: 되살리기·이름 끼우기·꾸미기도 낱말만 고친다", arguments: attacks)
    func restoreAttack(_ r: AttackRow) throws {
        let typed = try openTabCommand(r.argv, mdRenderDir: Self.dir).get()
        let prompt = Array(r.argv.dropFirst(3))
        // 보드가 칠 때 이름을 끼운다
        #expect(shellLine(withSessionName(typed) { "역할 1" })?.texts == ["claude", "--name", "역할 1", "--session-id", Self.sid] + prompt + ["--plugin-dir", Self.dir])
        // 다시 뜰 때(대화 기록 없음): 꾸미기 다시 고르기 → 새 세션 id
        let md = withMdRender(typed, dir: "/new/plugins/md-render")
        #expect(shellLine(md)?.texts == ["claude", "--session-id", Self.sid] + prompt + ["--plugin-dir", "/new/plugins/md-render"])
        let (f, id) = withFreshSession(md, newID: Self.newID)
        #expect(id == Self.newID)
        #expect(shellLine(f)?.texts == ["claude"] + prompt + ["--plugin-dir", "/new/plugins/md-render", "--session-id", Self.newID])
        #expect(shellLine(f)?.plain == true)
    }

    @Test("바깥 열기: 요청의 md-render --plugin-dir 은 빼고 보드 경로로")
    func openTabMdRender() throws {
        let argv = ["claude", "--plugin-dir", "/old/plugins/md-render", "--plugin-dir=/b/plugins/md-render", "--plugin-dir", "/mine", "글"]
        #expect(try openTabCommand(argv, mdRenderDir: Self.dir).get() == "claude --plugin-dir /mine '글' --plugin-dir \(Self.dir)")
        #expect(try openTabCommand(argv, mdRenderDir: nil).get() == "claude --plugin-dir /mine '글'")
        #expect(try openTabCommand(["claude", "=x"], mdRenderDir: nil).get() == "claude '=x'")
    }
}
