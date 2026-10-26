// 끌 때 확인 창의 목록 규칙(mac confirmShutdown 의 목록) — 누구를 보이고 누구에게 `.paused` 를 남기나. 순수 함수다.
// TODO(P1.1): 지금은 계약만 있다(빈 목록). 규칙·표 시험은 P1.1 이 채운다 — 쉬는 주 세션은 목록에 없이 pause 만, 일하는·기다리는 세션은 목록 + pause,
// 탭 안 `-p` 는 목록에만(pause 없음), 비 claude 프로그램이 도는 셸은 목록에, 백그라운드는 목록에 없음.
import type { BoardSurface, BoardTab, Session, ShutdownItem } from "../types.ts";

export type { ShutdownItem } from "../types.ts";

export interface ShutdownList {
  /** 확인 창에 보일 줄. 비면 확인 없이 끈다 */
  items: ShutdownItem[];
  /** `.paused` 를 남길 세션 id */
  pause: string[];
}

/** `surfaces` 는 탭 root 의 잎 id 가 가리키는 화면들 — 탭에서 세션·셸을 찾는 데 쓴다 */
export function shutdownList(_tabs: BoardTab[], _sessions: Session[], _surfaces: BoardSurface[]): ShutdownList {
  return { items: [], pause: [] };
}
