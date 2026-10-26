// 키 하나 — `KeyboardEvent.code`(자판 위치)와 수정키 넷. 글자가 아니라 코드로 맞추므로 한글 입력기에서도 같은 키다(mac `KeyCombo`).
// 아무것도 import 하지 않아 node:test 로 바로 시험한다.
//
// TODO(P2.4): 본문. 지금은 중립 값을 돌려준다(아무도 부르지 않는다).
// 저장 모양은 `[code, 수정키 비트]` — 비트는 Ctrl=1 · Alt=2 · Shift=4 · Win=8(백엔드는 모양만 본다).

export interface KeyCombo {
  code: string;
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  win: boolean;
}

/** 키 이벤트에서 읽은 키 */
export function fromEvent(e: Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">): KeyCombo {
  void e;
  return { code: "", ctrl: false, alt: false, shift: false, win: false };
}

/** 화면에 보이는 키 — "Ctrl+Alt+B" */
export function label(c: KeyCombo): string {
  void c;
  return "";
}
