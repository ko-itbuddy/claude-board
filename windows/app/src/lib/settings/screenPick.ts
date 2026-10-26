// 알림 위치를 화면 그림에서 고르는 순수 규칙 — 그림 안의 누른 점을 알림 위치 다섯 자리 가운데 하나로.
//
// TODO(P2.2): 본문. 지금은 늘 가운데를 돌려준다(아무도 부르지 않는다).
import type { NotifyPosition } from "$lib/types";

/** 그림 크기 안에서 누른 점이 가장 가까운 알림 위치 */
export function pickPosition(size: { width: number; height: number }, point: { x: number; y: number }): NotifyPosition {
  void size;
  void point;
  return "center";
}
