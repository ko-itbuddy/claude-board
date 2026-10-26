// 시험용 claude 바꿔치기(D5) — 환경변수 CLAUDE_BOARD_CLAUDE 가 있는 시험 실행에서만 보드가 치는 명령의 맨 앞 `claude` 를 가짜로 바꾼다. 순수 함수다.
// TODO(P1.T): 지금은 계약만 있다(명령을 그대로 둔다). 규칙·표 시험은 P1.T 가 채운다 — 맨 앞 낱말만(claude.exe·claude.cmd 포함) 바꾸고, 셸마다 따옴표가 다르다.
import type { TermInfo } from "../types.ts";

/** prefix = 프로그램과 앞쪽 인자(절대 경로). null 이면 시험 모드가 아니라 그대로 */
export function applyTestClaude(command: string, _prefix: string[] | null, _shell: TermInfo["shell"]): string {
  return command;
}

/** 백엔드가 알려 준 바꿔칠 값 — 시작 때 한 번 받아 둔다. 시험 모드가 아니면 null */
export function currentTestClaudePrefix(): string[] | null {
  return null;
}
