// 다른 터미널에서 도는 세션을 보드 탭으로 옮기기(mac moveToBoard) — 순서·확인·실패 알림 규칙. 효과는 deps 로 받는다.
// TODO(P1.3): 지금은 계약만 있다(아무것도 하지 않는다). 규칙·표 시험은 P1.3 이 채운다 — Esc, 0.4초, `/exit`, 0.2초, Enter 를 콘솔에 넣고
// 끝나기를 8초 기다린 뒤 보드 탭에서 이어 연다. 일하는·기다리는 세션은 먼저 묻는다. 실패하면 알림 한 줄.
import type { DriveStep, Session } from "../types.ts";

/** 세션을 끝내는 키 순서 */
export const MOVE_STEPS: DriveStep[] = [];

/** 옮기기 전에 확인을 받아야 하는 세션인가 */
export function needsConfirm(_s: Session): boolean {
  return false;
}

export interface MoveDeps {
  confirm(title: string, text: string): Promise<boolean>;
  drive(id: string, steps: DriveStep[]): Promise<boolean>;
  waitExit(pid: number, timeoutMs: number): Promise<boolean>;
  resumeSession(s: Session): void;
  toast(text: string): void;
}

export async function moveToBoard(_s: Session, _deps: MoveDeps): Promise<void> {}
