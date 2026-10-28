// 보드 규칙의 포트 — 세션 상태 판정(scanSessions)이 바깥(세션 기록 파일·프로세스·Claude Code 상태·대화 기록·컨텍스트·시계)을 부르는 자리,
// 사용량 규칙(Usage.swift)이 바깥(사용량 캐시·로그인 정보·사용량 API·시계)을 부르는 자리.
// 진짜 어댑터는 보드 앱(ClaudeBoard.swift)에, 가짜 어댑터는 Tests/BoardRulesTests 에만 둔다(이슈 #9 헥사고날 첫 단계).
// 규칙에 새 OS 호출(파일·프로세스·시각·환경 변수·홈 폴더)이 필요하면 여기 포트를 거친다 — 코어에서 FileManager·kill·Date() 를 직접 부르지 않는다.
// 포트는 역할로 나눈다(범용 FileSystem 포트를 두면 시험이 경로 문자열 시험이 된다).
//
// 포트는 Sendable 이다. 세션 상태 판정의 진짜 어댑터는 mtime 캐시를 쥔 클래스라 @unchecked Sendable 로 적는다 — Store.reload 가 메인 스레드에서만 부르고
// (1초 타이머·알림 관찰자), scanSessions 는 그 안에서 동기로 돈다. 다른 스레드에서 부르려면 어댑터에 잠금이 먼저 있어야 한다
import Foundation

// 세션 기록 한 개 — sessions/<파일 이름> 을 푼 것.
// json 이 [String: Any](JSONSerialization 결과)라 Sendable 이 아니다 — scanSessions 안에서만 쓰고 메인 스레드 밖으로 넘기지 않는다
public struct SessionRecord {
    public let name: String            // 파일 이름(<sid>.json) — 기록에 session_id 가 없으면 세션 id 를 여기서 뗀다
    public let file: String            // 그 파일 자리 — Session.file 로 넘어가 카드 지우기 등이 쓴다
    public let json: [String: Any]
    public init(name: String, file: String, json: [String: Any]) { self.name = name; self.file = file; self.json = json }
}

// 세션 기록(~/.claude/notifier/sessions/) — state.py 가 쓰는 <sid>.json 과 보드가 남기는 표지.
// 파일이 바뀔 때만 다시 푸는 mtime 캐시는 어댑터가 가진다 — 코어는 mtime 을 모른다
public protocol SessionRecords: Sendable {
    /// 읽을 수 있는 세션 기록 모두(파일 목록 차례). 풀 수 없는 파일은 뺀다
    func load() -> [SessionRecord]
    /// 낡은 기록을 지운다(scanSessions 가 판단한다)
    func remove(_ record: SessionRecord)
    /// <sid>.paused — 보드가 일시중지할 때 남기고, 새 요청이 오면 state.py 가 지운다
    func isPaused(_ sid: String) -> Bool
    /// <sid>.mute — 그 세션의 알림 끄기
    func isMuted(_ sid: String) -> Bool
    /// mute-all — 전체 알림 끄기. 상태 판정에는 쓰지 않고 Store 가 읽는다
    func allMuted() -> Bool
}

// 프로세스 — 세션 기록의 pid 가 아직 있는지, 그 claude 에 붙어 도는 백그라운드 셸 수
public protocol ProcessProbe: Sendable {
    /// 확실히 없는 pid 만 true(kill(pid, 0) 이 ESRCH). 권한이 없어 못 보는 pid(EPERM)는 false — 살아 있다고 본다
    func isGone(_ pid: Int32) -> Bool
    /// 그 claude 의 자식으로 도는 백그라운드 셸 수. 진짜 어댑터는 Store 가 3초마다 뒤에서 센 ps 결과를 그대로 준다
    func backgroundShells(_ pid: Int32) -> Int
}

// Claude Code 가 도는 세션마다 쓰는 ~/.claude/sessions/<pid>.json — status 가 바뀐 시각과 세션 이름
public protocol ClaudeLive: Sendable {
    /// status(idle·busy·waiting)와 그것이 바뀐 시각. 그 파일의 sessionId 가 이 세션일 때만
    func statusChange(_ pid: Int32, session: String) -> (status: String, at: Date)?
    /// 세션 이름(자동 이름·--name·/rename). 없으면 nil
    func name(_ pid: Int32) -> String?
    /// sessionId 가 이 세션인 파일의 pid — 세션 기록에 pid 가 없을 때(state.py 가 claude 를 못 찾음) 생사를 보려고 찾는다.
    /// 그런 파일이 없으면 nil(claude -p 처럼 이 파일을 쓰지 않는 세션이 있을 수 있다)
    func pid(forSession session: String) -> Int32?
    /// 목록에 남은 세션의 pid(기록의 pid, 없으면 위에서 찾은 pid)만 캐시에 남긴다
    func prune(keep: Set<Int32>)
}

// 대화 기록(~/.claude/projects/…/<sid>.jsonl) — 어디 있는지, 있는지, 끝에 적힌 Esc 중단·넘김·마지막 응답의 모델
public protocol Transcripts: Sendable {
    /// 세션의 대화 기록 자리 — 세션 기록의 transcript_path, 없으면 홈 폴더 아래 Claude Code 규칙대로
    func locate(given: String?, cwd: String, session: String) -> String
    func exists(_ path: String) -> Bool
    /// 마지막 대화 줄이 Esc 중단 표시면 그 시각
    func interruptedAt(_ path: String) -> Date?
    /// 대화를 백그라운드로 넘겼으면 (이어 받은 세션 id, 넘긴 시각) — 넘긴 뒤 대화 줄이 없을 때만
    func handoff(_ path: String) -> (id: String, at: Date)?
    /// 마지막 응답의 모델·effort. 없으면 fallbackModel(빈 글자면 nil). 세션·서브에이전트 대화 기록 모두
    func brain(_ path: String, fallbackModel: String, now: Date) -> Brain?
    /// keep: 모델·중단 캐시에 남길 것(끝나지 않은 세션과 그 서브에이전트의 대화 기록), keepHandoffs: 넘김 캐시에 남길 것(목록에 남은 모든 세션의 대화 기록)
    func prune(keep: Set<String>, keepHandoffs: Set<String>)
}

// 컨텍스트 사용량과 지금 effort — 상태줄 스크립트가 남긴 context/<sid>.json, 없으면 대화 기록 끝의 usage.
// 상태 판정에는 쓰지 않지만 Session 의 ctx·brain(effort)이 여기서 나오고 Session 은 고칠 수 없는 값이라, 판정 중에 함께 부른다
public protocol ContextUsage: Sendable {
    func read(_ sid: String, transcript: String, model: String, now: Date) -> Ctx?
    /// /effort 로 바꾼 지금 effort — 상태줄 기록이 대화 기록만큼 신선할 때만
    func liveEffort(_ sid: String, transcript: String) -> String?
    /// 목록에 남은 세션 몫만 캐시에 남긴다 — keep: 세션 id(상태줄 기록), keepTranscripts: 그 세션들의 대화 기록 자리(대화 기록에서 센 컨텍스트)
    func prune(keep: Set<String>, keepTranscripts: Set<String>)
}

// 시계 — scanSessions 는 한 번만 읽고 그 시각을 결과(SessionScan.now)로 돌려준다
public protocol Clock: Sendable {
    var now: Date { get }
}

// MARK: - 사용량 (Usage.swift)
// 진짜 어댑터 셋은 상태가 없는 struct 라 그대로 Sendable 이다. 받기(fetchUsage)는 백그라운드 큐에서, 캐시 확인·결과 처리는 메인에서 부른다

// 사용량 캐시 /tmp/.claude-usage-cache.json — 상태줄 스크립트와 함께 쓰는 약속이라 자리·형식(사용량 API 응답 그대로)을 바꾸지 않는다.
// 수정 시각과 내용을 따로 읽는다 — 내용은 믿을 수 있는 캐시일 때만 읽는다
public protocol UsageCache: Sendable {
    /// 캐시 파일 수정 시각. 없으면 nil
    func modifiedAt() -> Date?
    /// 캐시 파일 내용. 없거나 못 읽으면 nil
    func read() -> Data?
    /// 받은 응답 그대로 쓴다(통째로 바꿔 쓰기)
    func write(_ data: Data)
    /// 계정을 바꾼 뒤 앞 계정 값이 든 캐시를 지운다
    func remove()
}

// Claude Code 로그인 정보 — 키체인 "Claude Code-credentials" 의 JSON 바이트. 토큰이 들어 있으니 어댑터·규칙 모두 기록하지 않는다
public protocol LoginCredentials: Sendable {
    /// 로그인 정보 JSON. 항목이 없으면 nil
    func read() -> Data?
}

// 사용량 API(api.anthropic.com/api/oauth/usage) — 토큰은 메모리에만 두고 기록하지 않는다
public protocol UsageAPI: Sendable {
    /// 그 토큰으로 사용량을 받는다. 200 일 때만 본문, 그 밖(다른 상태·연결 실패·시간 초과)은 nil. 끝날 때까지 기다린다
    func fetch(token: String) -> Data?
}
