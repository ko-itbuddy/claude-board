// 탭 저장·복원 규칙(mac saveTabs·restoreTabs·resumeHeld) — hub 를 모르는 순수 함수다. node:test 로 바로 시험한다.
// 저장 모양은 types.ts 의 TabEntry·SavedNode·SavedSurface(mac `entry`·`savedNode`). 규칙:
// - 셸 화면은 경로만 남기고, claude 화면은 명령·세션 id·꾸미기 끔까지 남긴다. 명령이 끝나 셸만 남은 화면(command 가 null)은 셸로 저장한다.
// - 다시 뜰 때 모든 화면이 우선 셸로 돌아오고, claude 화면의 이어 열 값은 held 에 담긴다. 고른 탭의 화면만 바로 이어 열고(resumeNow),
//   나머지는 그 탭에 들어갈 때(lifecycle 의 resumeHeld) 이어 연다.
// - 앞 보드가 남긴 세션이 보드 밖에서 아직 도는 중이면(옛 pid 가 살아 있음) 5초까지 기다리고, 그래도 살아 있으면 그 화면은 되살리지 않는다.
import type { BoardSurface, BoardTab, Session, SplitDirection, SplitNode, TabEntry, TermId, TermInfo } from "../types.ts";
import type { SavedNode, SavedSurface } from "../types.ts";
import { freshSessionCommand, namedCommand, resumeCommand } from "./sessionCommand.ts";
import { sessionNameArg } from "./sessionName.ts";
import { leaves } from "./split.ts";

export type { SavedNode, SavedSurface, TabEntry } from "../types.ts";

/** 저장할 때 보는 화면 — 탭이 id 로만 가리키는 화면들·터미널 정보(cwd)·고른 탭 */
export interface RestoreView {
  surfaces: BoardSurface[];
  terms: Record<TermId, TermInfo>;
  selected: string;
}

// ── 저장 ──────────────────────────────────────────────────────

// 아직 들어가지 않아 이어 열지 않은 화면은 남겨 둔 값(held) 그대로 다시 저장한다(mac entry 의 `if let held`)
function savedFromHeld(held: Record<string, string>): SavedSurface {
  return {
    cwd: held.cwd ?? "",
    command: held.command ?? null,
    sessionId: held.session ?? null,
    mdRenderOff: held.mdoff !== undefined,
  };
}

function savedSurface(surface: BoardSurface | undefined, tab: BoardTab, view: RestoreView): SavedSurface {
  if (surface?.held) return savedFromHeld(surface.held);
  const cwd = (surface?.termId ? view.terms[surface.termId]?.cwd : undefined) || tab.workspace;
  if (!surface || surface.command === null) return { cwd, command: null, sessionId: null, mdRenderOff: false };
  return { cwd, command: surface.command, sessionId: surface.sessionId, mdRenderOff: surface.mdRenderOff };
}

function savedNode(node: SplitNode, tab: BoardTab, view: RestoreView): SavedNode {
  if ("leaf" in node) return { leaf: savedSurface(view.surfaces.find((s) => s.id === node.leaf), tab, view) };
  return {
    split: node.split,
    ratio: node.ratio,
    first: savedNode(node.first, tab, view),
    second: savedNode(node.second, tab, view),
  };
}

/** 지금 탭들을 저장 항목으로 — 끝난 명령은 셸로, 아직 이어 열지 않은 화면은 남겨 둔 값 그대로 */
export function savedTabs(tabs: BoardTab[], view: RestoreView): TabEntry[] {
  return tabs.map((tab) => ({
    workspace: tab.workspace,
    name: tab.customName,
    selected: tab.id === view.selected,
    root: savedNode(tab.root, tab, view),
  }));
}

// ── 읽기(디스크의 값은 믿지 않는다) ─────────────────────────

const isObject = (x: unknown): x is Record<string, unknown> => typeof x === "object" && x !== null && !Array.isArray(x);

function parseSurface(x: unknown): SavedSurface | null {
  if (!isObject(x) || typeof x.cwd !== "string") return null;
  const command = typeof x.command === "string" ? x.command : null;
  return {
    cwd: x.cwd,
    command,
    sessionId: command !== null && typeof x.sessionId === "string" && x.sessionId !== "" ? x.sessionId : null,
    mdRenderOff: command !== null && x.mdRenderOff === true,
  };
}

function parseNode(x: unknown): SavedNode | null {
  if (!isObject(x)) return null;
  if ("leaf" in x) {
    const leaf = parseSurface(x.leaf);
    return leaf && { leaf };
  }
  if (x.split !== "right" && x.split !== "down") return null;
  const first = parseNode(x.first);
  const second = parseNode(x.second);
  if (!first || !second) return null;
  const ratio = typeof x.ratio === "number" && Number.isFinite(x.ratio) ? Math.min(Math.max(x.ratio, 0.1), 0.9) : 0.5;
  return { split: x.split as SplitDirection, ratio, first, second };
}

/** 설정에서 읽은 값 가운데 모양이 맞는 탭 항목만 — 깨진 항목 하나가 보드 시작을 막지 않게 건너뛴다 */
export function parseEntries(raw: unknown): TabEntry[] {
  if (!Array.isArray(raw)) return [];
  const entries: TabEntry[] = [];
  for (const x of raw) {
    if (!isObject(x) || typeof x.workspace !== "string") continue;
    const root = parseNode(x.root);
    if (!root) continue;
    entries.push({ workspace: x.workspace, name: typeof x.name === "string" ? x.name : null, selected: x.selected === true, root });
  }
  return entries;
}

// ── 되살리기 ──────────────────────────────────────────────────

export interface RestorePlan {
  tabs: BoardTab[];
  /** tabs 가 가리키는 화면들 */
  surfaces: BoardSurface[];
  /** 고를 탭 id — 저장 때 고른 탭, 표시가 없으면 마지막 탭. 탭이 없으면 null */
  selectedTab: string | null;
  /** 지금 이어 열 화면 id — 고른 탭의 고른 화면. 나머지는 held 에 담아 들어갈 때 이어 연다. 없으면 null */
  resumeNow: string | null;
}

/** claude 화면이 다시 뜰 때 이어 열 값 — held 에 그대로 담는다(mac `held`: cwd·command·session·mdoff) */
function heldOf(leaf: SavedSurface): Record<string, string> {
  return {
    cwd: leaf.cwd,
    command: leaf.command ?? "",
    ...(leaf.sessionId ? { session: leaf.sessionId } : {}),
    ...(leaf.mdRenderOff ? { mdoff: "1" } : {}),
  };
}

/** 저장 항목들로 다시 세울 탭·화면과 지금 이어 열 화면을 정한다. `aliveOutside` 는 보드 밖에서 아직 도는 세션 id — 그 화면은 되살리지 않는다 */
export function restorePlan(entries: TabEntry[], aliveOutside: ReadonlySet<string> = new Set()): RestorePlan {
  const surfaces: BoardSurface[] = [];

  // 화면 하나를 만든다. 저장 때 claude 화면이었으면 셸로 띄우고 이어 열 값만 쥔다
  function surfaceOf(leaf: SavedSurface): BoardSurface | null {
    const isClaude = leaf.command !== null;
    if (isClaude && leaf.sessionId && aliveOutside.has(leaf.sessionId)) return null;
    const surface: BoardSurface = {
      id: crypto.randomUUID(),
      termId: null,
      sessionId: isClaude ? leaf.sessionId : null,
      command: null,
      held: isClaude ? heldOf(leaf) : null,
      mdRenderOff: isClaude && leaf.mdRenderOff,
      title: "",
      typedAt: null,
    };
    surfaces.push(surface);
    return surface;
  }

  // 되살리지 않는 화면은 빼고 형제가 그 자리를 차지한다
  function nodeOf(node: SavedNode): SplitNode | null {
    if ("leaf" in node) {
      const surface = surfaceOf(node.leaf);
      return surface && { leaf: surface.id };
    }
    const first = nodeOf(node.first);
    const second = nodeOf(node.second);
    if (!first || !second) return first ?? second;
    return { split: node.split, ratio: node.ratio, first, second };
  }

  const tabs: BoardTab[] = [];
  let selectedTab: string | null = null;
  for (const entry of entries) {
    const root = nodeOf(entry.root);
    if (!root) continue;
    const tab: BoardTab = {
      id: crypto.randomUUID(),
      workspace: entry.workspace,
      customName: entry.name,
      renaming: false,
      root,
      focused: leaves(root)[0],
      zoomed: false,
    };
    tabs.push(tab);
    if (entry.selected) selectedTab = tab.id;
  }
  selectedTab ??= tabs.at(-1)?.id ?? null;
  const picked = tabs.find((t) => t.id === selectedTab);
  const focusedSurface = picked && surfaces.find((s) => s.id === picked.focused);
  return { tabs, surfaces, selectedTab, resumeNow: focusedSurface?.held ? focusedSurface.id : null };
}

/** 저장 항목들이 가리키는 세션 id — 보드 밖에서 아직 도는지 볼 후보 */
export function savedSessionIds(entries: TabEntry[]): string[] {
  const ids = new Set<string>();
  const walk = (node: SavedNode) => {
    if ("leaf" in node) {
      if (node.leaf.command !== null && node.leaf.sessionId) ids.add(node.leaf.sessionId);
    } else {
      walk(node.first);
      walk(node.second);
    }
  };
  for (const entry of entries) walk(entry.root);
  return [...ids];
}

// ── 앞 보드가 남긴 프로세스 기다리기 ──────────────────────────

export interface Clock {
  now(): number;
  sleep(ms: number): Promise<void>;
}

/** 앞 보드가 남긴 pid 들 가운데 `timeoutMs` 뒤에도 살아 있는 것 — `pollMs` 마다 `alive` 로 다시 묻는다 */
export async function waitOldPids(
  pids: number[],
  alive: (pids: number[]) => Promise<number[]>,
  clock: Clock,
  timeoutMs = 5000,
  pollMs = 200,
): Promise<Set<number>> {
  const startedAt = clock.now();
  let remaining = new Set(pids.filter((pid) => pid > 0));
  if (remaining.size === 0) return remaining;
  remaining = new Set(await alive([...remaining]));
  while (remaining.size > 0 && clock.now() - startedAt < timeoutMs) {
    await clock.sleep(pollMs);
    remaining = new Set(await alive([...remaining]));
  }
  return remaining;
}

// ── 이어 열기(resumeHeld) ─────────────────────────────────────

/** 이어 열 때 아는 것 — sessions 는 보드 상태의 세션들(아직 못 받았으면 null), tabName 은 탭 줄 이름 */
export interface HeldContext {
  sessions: ReadonlyArray<Pick<Session, "id" | "state"> & { resumeArgs?: string[] }> | null;
  tabName: string;
}

export interface HeldLaunch {
  command: string;
  /** 화면이 쥘 세션 id — 새 세션이면 새로 붙인 id, claude 가 아닌 명령이면 null */
  sessionId: string | null;
}

/**
 * held 에 담아 둔 값으로 칠 명령을 정한다(mac restoreCommand). 돌려주는 null 은 이어 열지 않고 셸로 둔다는 뜻이다.
 * - 세션이 보드 상태에 있고 끝나지 않았다 = 보드 밖에서(다른 터미널·백그라운드) 이어 열렸다 → null
 * - 세션이 보드 상태에 있다(끝남) → `claude [그 세션의 이어 열기 인자] --resume <id>`
 * - 세션이 보드 상태에 없다 = 대화 없이 끝나 지워졌다 → 처음 명령을 새 세션 id 로
 * - 보드 상태를 아직 못 받았다 → 이어 열기(아는 인자가 없다)
 */
export function heldLaunch(held: Record<string, string>, context: HeldContext): HeldLaunch | null {
  const command = held.command;
  if (!command) return null;
  const name = sessionNameArg(context.tabName);
  const sessionId = held.session;
  if (sessionId) {
    const known = context.sessions?.find((s) => s.id === sessionId);
    if (known && known.state !== "ended") return null;
    if (known || context.sessions === null) {
      try {
        return { command: namedCommand(resumeCommand(known?.resumeArgs ?? [], sessionId, false), name), sessionId };
      } catch {
        return null; // 모양이 맞지 않는 세션 id — 명령에 넣지 않는다
      }
    }
  }
  const fresh = freshSessionCommand(command);
  return { command: namedCommand(fresh.command, name), sessionId: fresh.sessionId };
}
