// 수명 주기 — 보드를 켤 때 탭 되살리기(restore.ts), 탭이 바뀔 때 저장, 끌 때 확인 창(board://confirm-shutdown)을 hub 에 이어 준다.
// TODO(P1.1): 지금은 계약만 있다 — startLifecycle 은 아무것도 하지 않는다. 듣기·저장·되살리기는 P1.1 이 채운다.
import type { ConfirmShutdownPayload } from "$lib/types";

/** 확인 창 상태 — ShutdownList.svelte(P1.1 이 만든다)가 읽는다. 떠 있지 않으면 null */
export const Lifecycle = $state({
  confirm: null as ConfirmShutdownPayload | null,
});

/** 시작 때 한 번 — hub 의 startHub 가 부른다 */
export function startLifecycle(): void {}
