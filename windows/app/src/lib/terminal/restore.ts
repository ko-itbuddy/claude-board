// 탭 저장·복원 규칙(mac saveTabs·restoreTabs·resumeHeld) — hub 를 모르는 순수 함수다. node:test 로 바로 시험한다.
// TODO(P1.1): 지금은 계약만 있다(중립 값). 규칙·표 시험은 P1.1 이 채운다 — 끝난 명령은 셸로 저장, 고른 탭만 바로 이어 열고 나머지는 held,
// 옛 pid 가 5초 뒤에도 살아 있으면 그 화면은 버린다.
import type { BoardSurface, BoardTab, TabEntry, TermId, TermInfo } from "../types.ts";

export type { SavedNode, SavedSurface, TabEntry } from "../types.ts";

/** 저장할 때 보는 화면 — 탭이 id 로만 가리키는 화면들·터미널 정보(cwd)·고른 탭 */
export interface RestoreView {
  surfaces: BoardSurface[];
  terms: Record<TermId, TermInfo>;
  selected: string;
}

/** 지금 탭들을 저장 항목으로 — 끝난 명령은 셸로, 끝난 탭은 저장하지 않는다 */
export function savedTabs(_tabs: BoardTab[], _view: RestoreView): TabEntry[] {
  return [];
}

export interface RestorePlan {
  tabs: BoardTab[];
  /** tabs 가 가리키는 화면들 */
  surfaces: BoardSurface[];
  /** 지금 이어 열 화면 id — 고른 탭의 화면. 나머지는 held 에 담아 들어갈 때 이어 연다. 없으면 null */
  resumeNow: string | null;
}

/** 저장 항목들로 다시 세울 탭·화면과 지금 이어 열 화면을 정한다 */
export function restorePlan(_entries: TabEntry[]): RestorePlan {
  return { tabs: [], surfaces: [], resumeNow: null };
}

export interface Clock {
  now(): number;
  sleep(ms: number): Promise<void>;
}

/** 앞 보드가 남긴 pid 들 가운데 `timeoutMs` 뒤에도 살아 있는 것 — `pollMs` 마다 `alive` 로 다시 묻는다 */
export async function waitOldPids(
  _pids: number[],
  _alive: (pids: number[]) => Promise<number[]>,
  _clock: Clock,
  _timeoutMs = 5000,
  _pollMs = 200,
): Promise<Set<number>> {
  return new Set();
}
