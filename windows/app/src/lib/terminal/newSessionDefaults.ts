// 새 세션의 기본값 — 설정 > 터미널의 기본 명령(newSessionCommand)과 답변 꾸미기 기본값(newSessionMdRender)을 새 세션 창과 경로 줄의 "+" 가 쓴다.
// 아무것도 import 하지 않는 순수 함수라 node:test 로 시험한다.
//
// TODO(P2.3): 새 세션 창·"+" 가 이 함수들을 부르게 잇고 시험 표를 더한다. 지금은 아무도 부르지 않는다.

/** 설정이 비었을 때 새 세션이 치는 명령 */
export const DEFAULT_NEW_SESSION_COMMAND = "claude";

/** 새 세션의 기본 명령 — 설정 값(앞뒤 공백 뺀 것), 비면 claude */
export function defaultNewSessionCommand(settings: { newSessionCommand: string }): string {
  void settings;
  return DEFAULT_NEW_SESSION_COMMAND;
}

/** 새 세션을 열 때 답변 꾸미기를 켜 두는가 */
export function defaultNewSessionMdRender(settings: { newSessionMdRender: boolean }): boolean {
  void settings;
  return true;
}
