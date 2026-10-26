// 단축키 화면 상태(mac `ShortcutSettings`) — 키 받는 중인 동작, 받지 않은 키 안내, 전역 단축키 등록 실패.
//
// TODO(P2.4): 본문 — 받기 시작·끝(전역 단축키를 풀었다 다시 건다), 받은 키 처리(rules.ts 로 가려 setShortcut). 지금은 상태만 있고 늘 비어 있다.
import type { ShortcutActionId } from "./actions";

export const ShortcutSettings = $state({
  /** 새 키를 받는 중인 동작 */
  recording: null as ShortcutActionId | null,
  /** 받지 않은 키 안내 */
  refused: null as string | null,
  /** 전역 단축키로 등록하지 못한 동작 */
  hotKeyFailed: [] as ShortcutActionId[],
});

/** 줄의 키 칸을 눌렀다 — 그 동작의 새 키를 받기 시작한다 */
export function startRecording(id: ShortcutActionId): void {
  void id; // TODO(P2.4)
}

/** 키 받기를 그만둔다(Esc·다른 곳 누름·설정 화면 닫힘) */
export function stopRecording(): void {
  // TODO(P2.4)
}
