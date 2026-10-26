// 단축키 규칙(mac `Shortcuts.combo/set/conflicts`, `ShortcutSettings.record` 의 받기 조건) — 순수 함수만 둔다.
//
// TODO(P2.4): 본문. 지금은 중립 값을 돌려준다(아무도 부르지 않는다).
// - `set` — 기본값과 같은 키로 돌아오면 항목을 지우고, null 은 비움(`[]`)으로 저장한다.
// - `RESERVED` — Alt+F4·Alt+Tab·Alt+Space·Alt+Esc·Ctrl+Esc·Ctrl+Shift+Esc·Ctrl+C/V/X/A/Z/Y·Ctrl+,·Win 이 든 모든 키(판단 목록).
// - `acceptable` — Shift 만 든 키는 needsModifier(F1~F12 는 수정키 없이도 ok), RESERVED 는 reserved.
import type { ShortcutOverrides } from "$lib/types";
import type { ShortcutActionId } from "./actions";
import type { KeyCombo } from "./keyCombo";

/** 고를 수 없는 키 */
export const RESERVED: readonly KeyCombo[] = [];

/** 그 동작의 지금 키 — 바꾼 것이 있으면 그것(비움이면 null), 없으면 기본값 */
export function combo(id: ShortcutActionId, overrides: ShortcutOverrides): KeyCombo | null {
  void id;
  void overrides;
  return null;
}

/** 키를 바꾼 뒤의 저장 목록(새 객체). c 가 null 이면 비움 */
export function set(overrides: ShortcutOverrides, id: ShortcutActionId, c: KeyCombo | null): ShortcutOverrides {
  void id;
  void c;
  return overrides;
}

/** 같은 키를 쓰는 다른 동작의 id */
export function conflicts(id: ShortcutActionId, overrides: ShortcutOverrides): ShortcutActionId[] {
  void id;
  void overrides;
  return [];
}

/** 받아도 되는 키인가 */
export function acceptable(c: KeyCombo): "ok" | "reserved" | "needsModifier" {
  void c;
  return "ok";
}
