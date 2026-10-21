// 새 세션 띄우기 — 명령을 고쳐(--session-id, --name) 그 폴더의 셸 탭에 쳐 넣는다.
// macOS 보드의 freshSessionCommand(ClaudeBoard.swift:6270-6277)와 named(5481-5500)에 맞춘 것이다.
import { setTerminalExpanded } from "$lib/backend";
import { board } from "$lib/board/store.svelte";
import { folderName, hub, openShell, shownName } from "./hub.svelte";

/** 새 세션 기본 명령 — 승인 대기가 거의 없게 권한 확인을 건너뛴다(ClaudeBoard.swift defaultSessionCommand) */
export const DEFAULT_SESSION_COMMAND = "claude --dangerously-skip-permissions";

// claude 로 시작하는 명령 — 앞의 경로(\ 또는 /)와 .exe 는 있어도 된다
const CLAUDE_START = /^\s*(\S*[\\/])?claude(\.exe)?(?=\s|$)/i;
const CONTINUES = /(^|\s)(--resume|-r|--continue|-c)(=|\s|$)/;
const HAS_NAME = /(^|\s)(-n|--name)(=|\s|$)/;
const SESSION_ID_FLAG = /\s--session-id(=|\s+)\S+/g;

/**
 * 새로 띄우는 claude 에 세션 id 를 붙인다(--session-id) — 보드가 그 세션을 미리 안다.
 * claude 가 아니거나 이어 열기·이어 하기면 그대로(id 는 null). 이미 --session-id 가 있으면 새 id 로 바꾼다.
 */
export function freshSessionCommand(command: string): { command: string; sessionId: string | null } {
  if (!CLAUDE_START.test(command) || CONTINUES.test(command)) return { command, sessionId: null };
  const sessionId = crypto.randomUUID();
  return { command: command.replace(SESSION_ID_FLAG, "") + " --session-id " + sessionId, sessionId };
}

// 셸에 치는 이름 — pwsh·powershell·cmd 가 모두 같은 뜻으로 읽는 큰따옴표로 묶는다. 글자·숫자·_.- 뿐이면 그대로 둔다.
// 큰따옴표 안에서도 풀리는 " ` $ % 는 뺀다
function shellQuote(name: string): string {
  if (/^[\p{L}\p{N}_.-]+$/u.test(name)) return name;
  return `"${name.replace(/["`$%]/g, "")}"`;
}

/** 다른 탭이 쓰는 이름과 겹치지 않는 이름 — 겹치면 -2, -3 …(대소문자는 가리지 않는다) */
export function uniqueTabName(base: string): string {
  const used = new Set(hub.tabs.map((tab) => shownName(tab).toLowerCase()));
  let name = base;
  for (let n = 2; used.has(name.toLowerCase()); n++) name = `${base}-${n}`;
  return name;
}

/** claude 명령이면 `--name <이름>` 을 claude 바로 뒤에 붙인다. claude 가 아니거나 사용자가 -n·--name 을 적었으면 그대로 */
export function namedCommand(command: string, name: string): string {
  const start = CLAUDE_START.exec(command);
  if (!start || HAS_NAME.test(command)) return command;
  const at = start[0].length;
  return `${command.slice(0, at)} --name ${shellQuote(name)}${command.slice(at)}`;
}

/** 그 폴더에서 새 세션을 새 탭으로 띄운다. 터미널이 접혀 있으면 펼친다. 답변 꾸미기(md-render)는 Windows 에 아직 없다 */
export function startSession(command: string, dir: string) {
  const trimmed = command.trim();
  if (!trimmed) return;
  if (!board.snap?.settings.terminalExpanded) setTerminalExpanded(true);
  const fresh = freshSessionCommand(trimmed);
  const finalCommand = namedCommand(fresh.command, uniqueTabName(folderName(dir)));
  openShell(dir, { command: finalCommand, sessionId: fresh.sessionId ?? undefined });
}
