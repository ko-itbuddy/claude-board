// 바꿀 수 있는 동작의 표(mac `Shortcuts.actions`) — id 는 설정 shortcuts 의 열쇠라 바꾸면 사용자가 바꾼 키가 사라진다.
//
// TODO(P2.4): 2단계 행(보드·터미널·탭으로 가기)과 `runShortcutAction`. 3단계(P3.0)가 화면 나누기·글자 크기 행을 더한다.
import type { KeyCombo } from "./keyCombo";

export type ShortcutActionId = string;

export interface ShortcutAction {
  id: ShortcutActionId;
  /** 설정 > 단축키에서 묶는 이름 */
  group: string;
  title: string;
  /** 기본 키, 없으면 null(비어 있는 동작) */
  initial: KeyCombo | null;
}

export const SHORTCUT_ACTIONS: readonly ShortcutAction[] = [];

/** 동작 하나를 실행한다. 모르는 id 는 아무것도 하지 않는다 */
export function runShortcutAction(id: ShortcutActionId): void {
  void id; // TODO(P2.4)
}
