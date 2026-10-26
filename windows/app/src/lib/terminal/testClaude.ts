// 시험용 claude 바꿔치기(D5) — 환경변수 CLAUDE_BOARD_CLAUDE 가 있는 시험 실행에서만 보드가 치는 명령의 맨 앞 `claude` 를 가짜로 바꾼다. 순수 함수다.
// 바꾸는 것은 명령 맨 앞 낱말 하나뿐이다 — `claude`·`claude.exe`·`claude.cmd`, 그 앞에 경로가 붙은 것, 큰따옴표로 묶은 것. 인자는 건드리지 않는다.
// 바꿀 값(prefix)은 절대 경로 조각(프로그램 + 앞쪽 인자)이고, 조각마다 셸의 따옴표 규칙으로 묶는다.
//   pwsh·powershell  `& '<조각>' '<조각>'`   작은따옴표 안은 글자 그대로라 $·`·공백·한글이 안전하다(작은따옴표는 두 번 쓴다)
//   cmd              `"<조각>" "<조각>"`     큰따옴표로 묶는다(경로에 큰따옴표는 못 든다)
import type { TermInfo } from "../types.ts";

// 맨 앞 claude 낱말: (공백)(큰따옴표로 묶은 것 | 묶지 않은 것) 바로 뒤가 공백이나 끝이어야 한다(claude2·claude-board 는 아니다)
const LEADING = /^(\s*)(?:"(?:[^"]*[\\/])?claude(?:\.exe|\.cmd)?"|(?:[^\s"]*[\\/])?claude(?:\.exe|\.cmd)?)(?=\s|$)/i;
// PowerShell 은 곡선 작은따옴표도 작은따옴표로 읽는다
const PS_QUOTES = /['‘’‚‛]/g;

function quotePart(part: string, shell: TermInfo["shell"]): string {
  return shell === "cmd" ? `"${part}"` : `'${part.replace(PS_QUOTES, (q) => q + q)}'`;
}

/** prefix = 프로그램과 앞쪽 인자(절대 경로). null·빈 배열이면 시험 모드가 아니라 그대로 */
export function applyTestClaude(command: string, prefix: string[] | null, shell: TermInfo["shell"]): string {
  if (!prefix || prefix.length === 0) return command;
  const start = LEADING.exec(command);
  if (!start) return command;
  const quoted = prefix.map((part) => quotePart(part, shell)).join(" ");
  const replacement = shell === "cmd" ? quoted : `& ${quoted}`;
  return `${start[1]}${replacement}${command.slice(start[0].length)}`;
}

let cached: string[] | null = null;

/** 백엔드가 알려 준 바꿔칠 값 — 시작 때 한 번 받아 둔다. 시험 모드가 아니면 null */
export function currentTestClaudePrefix(): string[] | null {
  return cached;
}

/** 값을 받아 둔다. 문자열 조각이 아닌 것·빈 값은 시험 모드가 아닌 것으로 본다 */
export function setTestClaudePrefix(value: unknown): void {
  const ok = Array.isArray(value) && value.length > 0 && value.every((part) => typeof part === "string" && part !== "");
  cached = ok ? (value as string[]) : null;
}

/** 백엔드에 한 번 묻는다 — 실패하면 시험 모드가 아닌 것으로 둔다. 시험 하니스는 첫 키 전에 `testClaudePrefix()` 값을 직접 확인한다 */
export async function loadTestClaudePrefix(fetchPrefix: () => Promise<string[] | null>): Promise<void> {
  try {
    setTestClaudePrefix(await fetchPrefix());
  } catch {
    setTestClaudePrefix(null);
  }
}

// 브라우저(보드 창)에서만 이 모듈이 올라오는 즉시 한 번 묻는다. node:test 에는 window 가 없어 아무 일도 없다. 첫 명령은 셸이 300ms 조용해진 뒤에 쳐지므로 그 전에 도착한다
if (typeof window !== "undefined") {
  void import("../backend").then((backend) => loadTestClaudePrefix(backend.testClaudePrefix));
}
