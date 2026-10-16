// 화면이 보는 상태 — 백엔드 상태 묶음(통째로 바뀐다)과 보드 화면에만 있는 값(걸러 보기·단계·중간 화면에서 펼친 카드)
import { onSnapshot } from "$lib/backend";
import type { Snapshot, State } from "$lib/types";

export type Tier = "full" | "medium" | "mini";

// 1초마다 새 묶음으로 통째로 바꾸므로 깊은 반응형 대신 raw 로 둔다
let snap = $state.raw<Snapshot | null>(null);

export const board = {
  get snap() {
    return snap;
  },
};

export function connect() {
  return onSnapshot((s) => (snap = s));
}

export const ui = $state({
  // 요약 칩으로 고른 상태 — 그 상태의 세션만 보인다
  filter: null as State | null,
  tier: "full" as Tier,
  // 중간 화면에서 눌러 펼친 카드 — 저장한 접힘 상태는 그대로 둔다
  mediumOpen: [] as string[],
});
