// 투명 모드 규칙 — 보드가 불투명해야 하는가. 순수 함수다. node:test 로 바로 시험한다(opacity.test.ts).
// TODO(P1.4): 지금은 오늘의 규칙 그대로다. 3.1.9 규칙(마우스가 창 밖이면 포커스가 있어도 고른 투명도)은 P1.4 가 `+page.svelte` 호출 인자와 함께 바꾼다.

export interface OpacityInput {
  /** 터미널을 펼친 창이 화면을 덮는가 — 지금은 펼쳤는가와 같다 */
  covering: boolean;
  /** 슬라이더를 끄는 중이거나 놓은 뒤 1.2초 — 고른 투명도를 미리 보여 준다 */
  previewing: boolean;
  /** 마우스가 창 안에 있거나 보드가 활성이다 */
  pointerInside: boolean;
}

/** 오늘의 규칙: 덮고 있으면 늘 불투명, 아니면 미리 보는 중이 아닐 때 마우스가 안에 있거나 활성일 때 불투명 */
export function boardOpaque({ covering, previewing, pointerInside }: OpacityInput): boolean {
  return covering || (!previewing && pointerInside);
}
