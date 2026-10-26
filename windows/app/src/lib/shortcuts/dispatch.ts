// 앱 안 키 감시의 판단(mac `Shortcuts.handle`) — 눌린 키가 어느 동작인가. 실행은 부른 쪽(TerminalAreaView)이 `runShortcutAction` 으로 한다.
// 값으로 import 하는 것은 같은 폴더의 순수 모듈뿐이라 node:test 로 시험한다.
//
// 순서: 입력기 조합 중이면 null → AltGraph 면 null → 키 받는 중이면 받는 쪽으로 → 모달이 떠 있으면 null →
// 설정 키(Ctrl+,) → 설정 화면이 떠 있을 때 Esc → 표에서 찾기.
import type { ShortcutOverrides } from "../types.ts";
import { SETTINGS_ESCAPE, SETTINGS_KEY, SETTINGS_TOGGLE, SHORTCUT_RECORD, type ShortcutActionId } from "./actions.ts";
import { fromEvent, isModifierCode, same } from "./keyCombo.ts";
import { find } from "./rules.ts";

export interface DispatchContext {
  /** 새 키를 받는 중인 동작 id(설정 > 단축키), 아니면 null */
  recording: ShortcutActionId | null;
  /** 입력기가 글자를 조합하는 중 */
  composing: boolean;
  /** 확인 창·새 세션 창 같은 모달이 떠 있다 */
  modalOpen: boolean;
  settingsShown: boolean;
  /** 사용자가 바꾼 키(설정 shortcuts). 없으면 기본 키 표 그대로 */
  overrides?: ShortcutOverrides;
}

export type DispatchKey = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey" | "getModifierState">;

/** 눌린 키에 맞는 동작 id. 어느 것도 아니면 null — 키를 그대로 둔다 */
export function dispatch(e: DispatchKey, context: DispatchContext): ShortcutActionId | null {
  if (context.composing) return null;
  // AltGr 은 Windows 가 Ctrl+Alt 로 보고한다 — 글자(@ { [ 등)를 치는 키라 가로채지 않는다
  if (e.getModifierState("AltGraph")) return null;
  if (context.recording) return SHORTCUT_RECORD;
  if (context.modalOpen) return null;
  const pressed = fromEvent(e);
  if (isModifierCode(pressed.code)) return null;
  if (same(pressed, SETTINGS_KEY)) return SETTINGS_TOGGLE;
  if (context.settingsShown && pressed.code === "Escape" && !pressed.ctrl && !pressed.alt && !pressed.shift && !pressed.win) return SETTINGS_ESCAPE;
  return find(pressed, context.overrides ?? {});
}
