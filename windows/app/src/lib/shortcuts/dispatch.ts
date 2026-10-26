// 앱 안 키 감시의 판단(mac `Shortcuts.handle`) — 눌린 키가 어느 동작인가. 실행은 부른 쪽(TerminalAreaView)이 `runShortcutAction` 으로 한다.
// 아무것도 import 하지 않는 순수 함수라 node:test 로 시험한다.
//
// TODO(P2.4): 본문. 지금은 늘 null(어느 동작도 아님)이라 부른 쪽이 예전 키 표(TerminalAreaView)로 처리한다 — 키 동작이 그대로다.
// 표(설계 §4): 입력기 조합 중이면 null · AltGraph 면 null · 키 받는 중이면 받는 쪽으로 · 설정 화면이 떠 있을 때 Esc 는 "settings.escape".
import type { ShortcutActionId } from "./actions";

export interface DispatchContext {
  /** 새 키를 받는 중인 동작 id(설정 > 단축키), 아니면 null */
  recording: ShortcutActionId | null;
  /** 입력기가 글자를 조합하는 중 */
  composing: boolean;
  /** 확인 창·새 세션 창 같은 모달이 떠 있다 */
  modalOpen: boolean;
  settingsShown: boolean;
}

export type DispatchKey = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey" | "getModifierState">;

/** 눌린 키에 맞는 동작 id. 어느 것도 아니면 null — 키를 그대로 둔다 */
export function dispatch(e: DispatchKey, context: DispatchContext): ShortcutActionId | null {
  void e;
  void context;
  return null;
}
