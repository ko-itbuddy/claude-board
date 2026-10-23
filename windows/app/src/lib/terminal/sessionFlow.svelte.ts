// 세션 열기 흐름(mac openSession·reveal·resumeSession·toast) — 세션 줄을 눌렀을 때와 백엔드의 "세션 열기" 요청(reopen·focus)을
// 보드 탭으로 옮긴다. 무엇을 할지는 board/openSession.ts 의 순수 표가 정하고, 여기는 그 효과를 hub 에 실행한다.
import {
  doubleClickTime,
  onOpenSession,
  onSnapshot,
  sessionAction,
  setTerminalExpanded,
  showBoard,
  takeOpenSession,
} from "$lib/backend";
import { openSession, type OpenEffect } from "$lib/board/openSession";
import { createClickGate, type ClickGate } from "$lib/board/clickGate";
import { board } from "$lib/board/store.svelte";
import type { BoardSurface, OpenSessionPayload, Session } from "$lib/types";
import { TerminalHub, folderName, openShell, pair, place, select, surfaceOf, workspaceKey } from "./hub.svelte";
import { uniqueTabName } from "./launch";
import { applyRenames } from "./renameFlow.svelte";
import { namedCommand, resumeCommand } from "./sessionCommand";
import { quoteSessionName, sanitizeSessionName } from "./sessionName";
import { hasLeaf } from "./split";
import { typeAtPrompt } from "./typeCommand";

const NOTICE_MS = 6000;
let noticeTimer: ReturnType<typeof setTimeout> | undefined;

/** 보드 안 알림 한 줄을 6초 동안 보인다. 다시 부르면 시간이 새로 시작한다 */
export function toast(text: string) {
  clearTimeout(noticeTimer);
  TerminalHub.notice = text;
  noticeTimer = setTimeout(dismissNotice, NOTICE_MS);
}

export function dismissNotice() {
  clearTimeout(noticeTimer);
  TerminalHub.notice = null;
}

const allSessions = (sessions = board.snap?.groups ?? []): Session[] => sessions.flatMap((g) => g.sessions);
const findSession = (id: string): Session | undefined => allSessions().find((s) => s.id === id);
const tabOfSurface = (surfaceId: string) => TerminalHub.tabs.find((t) => hasLeaf(t.root, surfaceId));

/** 그 화면의 탭을 펼쳐 고르고 보드 창을 앞으로(mac reveal) */
export function reveal(surfaceId: string) {
  const tab = tabOfSurface(surfaceId);
  if (!tab) return;
  if (!board.snap?.settings.terminalExpanded) setTerminalExpanded(true);
  tab.focused = surfaceId;
  select(tab.id);
  showBoard();
}

// 셸 프롬프트에 있고 claude 가 없는 같은 경로의 화면 — 끝난 세션을 새 탭 대신 이어 열 자리(mac launch 의 idle 탭)
function idleSurface(dir: string): BoardSurface | undefined {
  const atPrompt = new Set(board.snap?.atPrompt ?? []);
  const key = workspaceKey(dir);
  return TerminalHub.surfaces.find((x) => {
    const tab = tabOfSurface(x.id);
    const typing = x.typedAt != null && Date.now() - x.typedAt < 3000;
    return x.termId !== null && atPrompt.has(x.termId) && x.command === null && !typing && tab && workspaceKey(tab.workspace) === key;
  });
}

/** 끝난 세션을 보드 탭에서 이어 연다(mac resumeSession) — 이미 이어 여는 중이거나 탭에 있으면 그 탭을 앞으로 */
function resumeSession(p: OpenSessionPayload) {
  const s = findSession(p.id);
  const where = s && place(s);
  if (where?.kind === "tab") return reveal(where.surfaceId);
  const pending = TerminalHub.surfaces.find((x) => x.sessionId === p.id && x.command !== null);
  if (pending) return reveal(pending.id);
  let command: string;
  try {
    const name = quoteSessionName(uniqueTabName(sanitizeSessionName(folderName(p.projectDir || "~"))));
    command = namedCommand(resumeCommand(p.resumeArgs, p.id, p.carryOn), name);
  } catch (e) {
    return toast(`세션을 이어 열 수 없습니다: ${e instanceof Error ? e.message : e}`);
  }
  if (!board.snap?.settings.terminalExpanded) setTerminalExpanded(true);
  const idle = idleSurface(p.projectDir);
  if (idle?.termId) {
    const termId = idle.termId;
    idle.sessionId = p.id;
    idle.command = command;
    void typeAtPrompt(termId, command, {
      alive: () => surfaceOf(idle.id)?.termId === termId && !!TerminalHub.terms[termId],
      touch: () => {
        idle.typedAt = Date.now();
      },
    });
    return reveal(idle.id);
  }
  const tab = openShell(p.projectDir || null, { command, sessionId: p.id });
  if (tab) reveal(tab.focused);
}

function handleOpenRequest(p: OpenSessionPayload) {
  if (p.action === "reopen") return resumeSession(p);
  const s = findSession(p.id);
  if (s) runEffect(s, openSession(s, place(s), true));
}

function runEffect(s: Session, effect: OpenEffect) {
  if (effect.kind === "reveal") reveal(effect.surfaceId);
  else if (effect.kind === "action") sessionAction(effect.action, s.id, effect.carryOn);
  else if (effect.kind === "notice") toast(effect.text);
}

let doubleClickMs = 500;

/** 보드 시작 때 한 번 — 상태 묶음마다 짝짓기·이름 바꾸기, 세션 열기 요청 받기, 더블 클릭 시간 읽기 */
export function startSessionFlow() {
  onSnapshot((snap) => {
    pair(allSessions(snap.groups), snap.runsClaude ?? null);
    void applyRenames();
  });
  onOpenSession(handleOpenRequest);
  takeOpenSession()
    .then((p) => p && handleOpenRequest(p))
    .catch(() => {});
  void doubleClickTime().then((ms) => (doubleClickMs = ms));
}

export interface RowClick {
  click(e: MouseEvent): void;
  dispose(): void;
}

/**
 * 세션 줄(또는 미니 보드의 경로 줄)의 클릭 처리. 다른 터미널의 줄만 한 번 클릭을 더블 클릭 시간만큼 미룬다(clickGate) —
 * 두 번째 클릭이 오면 activate 없이 focus 만 낸다. 그 밖의 자리는 바로 openSession 표대로 간다.
 * 끝난 세션의 두 번째 클릭은 첫 클릭이 이미 열고 있으므로 무시한다.
 */
export function createRowClick(getSession: () => Session | undefined): RowClick {
  let gate: ClickGate | undefined;
  const act = (action: "activate" | "focus") => {
    const s = getSession();
    if (s) sessionAction(action, s.id);
  };
  const gateOf = () =>
    (gate ??= createClickGate({
      delayMs: doubleClickMs,
      setTimer: (callback, ms) => setTimeout(callback, ms),
      clearTimer: (handle) => clearTimeout(handle),
      onActivate: () => act("activate"),
      onFocus: () => act("focus"),
    }));
  return {
    click(e) {
      const s = getSession();
      if (!s) return;
      const where = place(s);
      if (where.kind === "terminal") return gateOf().press(e.detail);
      gate?.dispose();
      if (where.kind === "ended" && e.detail >= 2) return;
      runEffect(s, openSession(s, where, e.detail >= 2));
    },
    dispose: () => gate?.dispose(),
  };
}
