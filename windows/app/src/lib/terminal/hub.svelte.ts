// 터미널 저장소(TerminalHub) — 화면이 보는 터미널 목록·탭·칸 배치와 칸별 컨트롤러.
// 터미널 출력은 보드의 1초 상태 묶음에 싣지 않고, 여기서 term_list 와 채널로 채운다(이벤트 기반).
// 1단계는 탭 하나·칸 하나지만 모든 명령이 TermId 를 받고 칸은 leafId 로 따로 이름을 가져 탭·분할로 늘릴 수 있다.
import { termList, termSpawn } from "$lib/backend";
import type { LeafPhase, PaneNode, TermId, TermInfo, TerminalTab } from "$lib/types";
import { TermController } from "./term";

export interface LeafState {
  phase: LeafPhase;
  exitCode: number | null;
  error: string;
}

export const hub = $state({
  terms: {} as Record<TermId, TermInfo>,
  tabs: [] as TerminalTab[],
  activeTabId: "",
  leaves: {} as Record<string, LeafState>,
});

// 컨트롤러는 xterm 을 쥔 일반 객체라 반응형 상태에 넣지 않는다
const controllers = new Map<string, TermController>();
let started = false;

export function controllerOf(leafId: string): TermController {
  let ctl = controllers.get(leafId);
  if (!ctl) {
    ctl = new TermController(leafId, { onExit: (code) => markExited(leafId, code) });
    controllers.set(leafId, ctl);
  }
  return ctl;
}

function newTab(termId: TermId | null): TerminalTab {
  const leafId = crypto.randomUUID();
  hub.leaves[leafId] = { phase: "idle", exitCode: null, error: "" };
  const root: PaneNode = { type: "leaf", leafId, termId };
  return { id: crypto.randomUUID(), root, activeLeaf: leafId };
}

export function findLeaf(node: PaneNode, leafId: string): (PaneNode & { type: "leaf" }) | null {
  if (node.type === "leaf") return node.leafId === leafId ? node : null;
  return findLeaf(node.first, leafId) ?? findLeaf(node.second, leafId);
}

function leafOf(leafId: string) {
  for (const tab of hub.tabs) {
    const leaf = findLeaf(tab.root, leafId);
    if (leaf) return leaf;
  }
  return null;
}

/** 이미 도는 터미널이 있으면(페이지를 새로 고친 경우) 그것을 이어 쓰고, 없으면 빈 칸을 하나 만든다. 여러 번 불러도 한 번만 한다 */
export async function startHub() {
  if (started) return;
  started = true;
  let list: TermInfo[] = [];
  try {
    list = await termList();
  } catch (e) {
    console.error("터미널 목록을 읽지 못함", e);
  }
  for (const info of list) hub.terms[info.id] = info;
  const alive = list.filter((t) => t.status === "running").at(-1);
  const tab = newTab(alive?.id ?? null);
  hub.tabs.push(tab);
  hub.activeTabId = tab.id;
}

export function activeTab(): TerminalTab | undefined {
  return hub.tabs.find((t) => t.id === hub.activeTabId);
}

export function focusActive() {
  const tab = activeTab();
  if (tab) controllers.get(tab.activeLeaf)?.focus();
}

/** 칸의 셸을 띄우거나(없으면 새로, 있으면 이어서) 붙는다. 칸이 처음 보일 때 부른다 */
export async function startLeaf(leafId: string) {
  const leaf = leafOf(leafId);
  const state = hub.leaves[leafId];
  if (!leaf || !state || state.phase !== "idle") return;
  state.phase = "starting";
  state.error = "";
  const ctl = controllerOf(leafId);
  try {
    await ctl.open();
    let info = leaf.termId ? hub.terms[leaf.termId] : undefined;
    if (!info) {
      const dims = ctl.dims();
      if (!dims) throw new Error("터미널 크기를 잴 수 없음");
      info = await termSpawn({ cols: dims.cols, rows: dims.rows });
      hub.terms[info.id] = info;
      leaf.termId = info.id;
    }
    await ctl.bind(info);
    // 붙는 사이에 셸이 이미 끝났을 수 있다
    if (state.phase === "starting") state.phase = "running";
  } catch (e) {
    state.phase = "failed";
    state.error = String(e);
    console.error("터미널을 시작하지 못함", e);
  }
}

/** 끝난(또는 시작에 실패한) 칸에 새 셸을 띄운다. 칸은 그대로 두고 터미널만 바꾼다 */
export function respawn(leafId: string) {
  const leaf = leafOf(leafId);
  const state = hub.leaves[leafId];
  if (!leaf || !state || state.phase === "starting" || state.phase === "running") return;
  controllerOf(leafId).reset();
  leaf.termId = null;
  state.phase = "idle";
  state.exitCode = null;
  void startLeaf(leafId).then(() => focusActive());
}

function markExited(leafId: string, code: number | null) {
  const state = hub.leaves[leafId];
  if (!state) return;
  state.phase = "exited";
  state.exitCode = code;
  const termId = leafOf(leafId)?.termId;
  const info = termId ? hub.terms[termId] : undefined;
  if (info) {
    info.status = "exited";
    info.exitCode = code;
  }
}

/** 경계를 끄는 동안 칸 크기를 맞추지 않고, 놓을 때 한 번 맞춘다 */
export function setDragging(dragging: boolean) {
  for (const ctl of controllers.values()) ctl.setDragging(dragging);
}
