// 수명 주기 — 보드를 켤 때 탭 되살리기(restore.ts), 탭이 바뀔 때 저장, 끌 때 확인 창(board://confirm-shutdown)을 hub 에 이어 준다.
// 무엇을 저장·복원하고 누구를 확인 창에 보이는지는 restore.ts·shutdownList.ts 가 정한다. 여기는 그 결과를 hub 상태와 백엔드에 실행한다.
import { untrack } from "svelte";
import {
  answerShutdown,
  loadTabs,
  onConfirmShutdown,
  onSnapshot,
  pidsAlive,
  saveTabs,
  showBoard,
  shutdownDialogShown,
} from "$lib/backend";
import { board } from "$lib/board/store.svelte";
import type { BoardSurface, ConfirmShutdownPayload, Session, Snapshot, TabEntry } from "$lib/types";
import { TerminalHub, controllerOf, focusActive, select, shownName, surfaceOf } from "./hub.svelte";
import { heldLaunch, parseEntries, restorePlan, savedSessionIds, savedTabs, waitOldPids, type Clock } from "./restore";
import { shutdownList, type ShutdownList } from "./shutdownList";
import { hasLeaf, leaves } from "./split";
import { sleep, typeCommand } from "./typeCommand";

/** 확인 창 상태 — ShutdownList.svelte 가 읽는다. 떠 있지 않으면 null */
export const Lifecycle = $state({
  confirm: null as ConfirmShutdownPayload | null,
});

/** 저장한 탭 목록을 쓰기 전에 기다리는 시간(ms) — 짧은 사이에 여러 번 바뀌는 것을 한 번으로 묶는다 */
const SAVE_DEBOUNCE_MS = 300;
/** 첫 상태 묶음을 기다리는 시간(ms) — 넘으면 세션을 모르는 채 되살린다(이어 열기는 세션 id 로 한다) */
const FIRST_SNAPSHOT_WAIT_MS = 3000;
/** 끄기·업데이트를 고른 뒤 이 시간(ms) 동안은 저장하지 않는다 — 터미널이 먼저 죽어 탭이 닫혀 가는 모습을 저장하면 목록이 줄어든다.
 * 끄기가 끝내 일어나지 않았으면(업데이트 실패 등) 이 시간 뒤의 지금 모습을 저장한다 */
const FROZEN_AFTER_SHUTDOWN_MS = 10_000;

const realClock: Clock = { now: () => Date.now(), sleep };

const sessionsOf = (snap: Snapshot | null | undefined): Session[] => snap?.groups.flatMap((g) => g.sessions) ?? [];

// ── 저장 ──────────────────────────────────────────────────────

let lastSent: string | null = null;
let pendingSave: { json: string; entries: TabEntry[] } | null = null;
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let frozenUntil = 0;

function currentEntries() {
  return savedTabs(TerminalHub.tabs, { surfaces: TerminalHub.surfaces, terms: TerminalHub.terms, selected: TerminalHub.selected });
}

function sendPendingSave(force = false) {
  clearTimeout(saveTimer);
  saveTimer = undefined;
  const frozenFor = frozenUntil - Date.now();
  if (!force && frozenFor > 0) {
    saveTimer = setTimeout(() => sendPendingSave(), frozenFor);
    return;
  }
  if (!pendingSave || pendingSave.json === lastSent) return void (pendingSave = null);
  lastSent = pendingSave.json;
  saveTabs(pendingSave.entries);
  pendingSave = null;
}

/** 바뀐 탭 목록을 잠깐 모았다가 저장한다(mac saveTabs — 바뀔 때마다. 같으면 쓰지 않는다) */
function scheduleSave(entries: TabEntry[] = currentEntries()) {
  pendingSave = { json: JSON.stringify(entries), entries };
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => sendPendingSave(), SAVE_DEBOUNCE_MS);
}

/** 지금 탭 목록을 기다리지 않고 저장한다 — 끄기 직전·되살리기를 마친 직후 */
function flushSave() {
  scheduleSave();
  sendPendingSave(true);
}

// ── 이어 열기(resumeHeld) ─────────────────────────────────────

/** 되살린 뒤 아직 들어가지 않은 화면의 claude 를 이어 연다 — 그 탭을 고를 때. 명령은 그때 다시 정한다(그사이 다른 곳에서 이어 열렸으면 셸로 둔다) */
function resumeHeld(surface: BoardSurface) {
  const held = surface.held;
  const tab = TerminalHub.tabs.find((t) => hasLeaf(t.root, surface.id));
  const state = TerminalHub.leaves[surface.id];
  if (!held || !tab || !state) return;
  // 셸이 못 떴으면 값을 쥐고 있는다 — 다시 시도해 뜨면 그때 이어 연다
  if (state.phase === "failed" || state.phase === "exited") return;
  surface.held = null;
  const launch = heldLaunch(held, { sessions: board.snap ? sessionsOf(board.snap) : null, tabName: shownName(tab) });
  if (!launch) return;
  if (launch.sessionId) surface.sessionId = launch.sessionId;
  if (state.phase === "idle" || state.phase === "starting" || !surface.termId) {
    // 셸이 뜨는 중이거나 아직 안 떴다 — startLeaf 가 셸이 뜬 뒤 한 번 친다
    surface.command = launch.command;
    surface.typedAt = Date.now();
    return;
  }
  const termId = surface.termId;
  surface.command = launch.command;
  void typeCommand(controllerOf(surface.id), termId, launch.command, {
    alive: () => surfaceOf(surface.id)?.termId === termId && !!TerminalHub.terms[termId],
    touch: () => {
      const live = surfaceOf(surface.id);
      if (live) live.typedAt = Date.now();
    },
    shell: TerminalHub.terms[termId]?.shell ?? "pwsh",
    mdRenderOff: surface.mdRenderOff,
  });
}

// ── 되살리기 ──────────────────────────────────────────────────

function firstSessions(): Promise<Session[] | null> {
  if (board.snap) return Promise.resolve(sessionsOf(board.snap));
  return new Promise((resolve) => {
    let settled = false;
    let off = () => {};
    const finish = (value: Session[] | null) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      off();
      resolve(value);
    };
    const timer = setTimeout(() => finish(null), FIRST_SNAPSHOT_WAIT_MS);
    off = onSnapshot((snap) => finish(sessionsOf(snap)));
    if (settled) off();
  });
}

/** 저장해 둔 탭을 되살린다. 이미 탭이 있으면(새로 고침으로 터미널을 이어받음) 건드리지 않는다 */
async function restoreSavedTabs() {
  if (TerminalHub.tabs.length > 0) return;
  let entries: TabEntry[] = [];
  try {
    entries = parseEntries(await loadTabs());
  } catch (e) {
    console.error("저장해 둔 탭 목록을 읽지 못함", e);
  }
  if (entries.length === 0) return;
  const sessions = await firstSessions();
  // 앞 보드가 남긴 claude 가 보드 밖에서 아직 도는 중이면(업데이트 직후 등) 같은 세션을 둘이 쥐지 않게 최대 5초 기다린다
  const candidates = new Map<number, string>();
  for (const id of savedSessionIds(entries)) {
    const session = sessions?.find((s) => s.id === id);
    if (session && session.state !== "ended" && session.pid > 0) candidates.set(session.pid, id);
  }
  const stillAlive = await waitOldPids([...candidates.keys()], pidsAlive, realClock);
  const aliveOutside = new Set([...stillAlive].map((pid) => candidates.get(pid)!));
  // 기다리는 사이 사용자가 탭을 열었으면 그 탭 뒤에 이어 붙인다
  const plan = restorePlan(entries, aliveOutside);
  for (const surface of plan.surfaces) {
    TerminalHub.surfaces.push(surface);
    TerminalHub.leaves[surface.id] = { phase: "idle", exitCode: null, error: "" };
  }
  TerminalHub.tabs.push(...plan.tabs);
  if (TerminalHub.selected === "" && plan.selectedTab) {
    select(plan.selectedTab, false);
    const resumeNow = plan.resumeNow ? surfaceOf(plan.resumeNow) : undefined;
    if (resumeNow) resumeHeld(resumeNow);
  }
}

// ── 끌 때 확인 ────────────────────────────────────────────────

function currentShutdownList(): ShutdownList {
  return shutdownList(TerminalHub.tabs, sessionsOf(board.snap), TerminalHub.surfaces, {
    atPrompt: board.snap?.atPrompt,
    runsClaude: board.snap?.runsClaude,
  });
}

function onConfirmRequest(request: ConfirmShutdownPayload) {
  const { items } = currentShutdownList();
  flushSave();
  if (items.length === 0) {
    frozenUntil = Date.now() + FROZEN_AFTER_SHUTDOWN_MS;
    return answerShutdown(true, []);
  }
  // 보드가 숨어 있거나 접혀 있어도 보이게 한다 — 창을 띄웠다고 알려야 백엔드가 사용자가 고를 때까지 기다린다
  Lifecycle.confirm = { reason: request.reason, items };
  showBoard();
  shutdownDialogShown();
}

/** 확인 창의 답 — 계속(ok)이면 고른 순간의 세션으로 다시 세어 `.paused` 대상을 넘긴다(창을 띄워 둔 사이 끝났거나 새로 일을 시작한 세션이 있을 수 있다) */
export function answerConfirm(ok: boolean) {
  if (!Lifecycle.confirm) return;
  const pause = ok ? currentShutdownList().pause : [];
  Lifecycle.confirm = null;
  flushSave();
  if (ok) frozenUntil = Date.now() + FROZEN_AFTER_SHUTDOWN_MS;
  answerShutdown(ok, pause);
  if (!ok) focusActive();
}

// ── 시작 ──────────────────────────────────────────────────────

let started = false;

/** 시작 때 한 번 — hub 의 startHub 가 부른다. 여러 번 불러도 한 번만 한다 */
export function startLifecycle(): void {
  if (started) return;
  started = true;
  onConfirmShutdown(onConfirmRequest);
  void restoreSavedTabs()
    .catch((e) => console.error("탭을 되살리지 못함", e))
    .then(() => {
      // 되살리기를 마친 뒤부터 바뀔 때마다 저장한다 — 마치기 전에 저장하면 지난 목록을 덮는다. 첫 저장은 백엔드에 수명 주기가 켜졌다고 알린다
      flushSave();
      $effect.root(() => {
        // 탭 목록·화면·터미널 경로·고른 탭을 읽는 것만으로 의존이 잡힌다
        $effect(() => {
          const entries = currentEntries();
          untrack(() => scheduleSave(entries));
        });
        // 탭에 들어갈 때 그 탭의 아직 안 이어 연 화면을 이어 연다
        $effect(() => {
          const tab = TerminalHub.tabs.find((t) => t.id === TerminalHub.selected);
          if (!tab) return;
          untrack(() => {
            for (const id of leaves(tab.root)) {
              const surface = surfaceOf(id);
              if (surface?.held) resumeHeld(surface);
            }
          });
        });
      });
    });
}
