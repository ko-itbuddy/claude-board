// 백엔드(src-tauri)와 주고받는 곳 — windows/docs/SPEC.md §6.
// Tauri 안에서는 invoke·이벤트를 쓰고, 브라우저(npm run dev)에서는 mock.ts 의 가짜 상태로 화면을 끝까지 눌러 볼 수 있게 한다.
// 명령 인자 이름은 Rust 명령의 매개변수 이름과 같아야 한다(Tauri 가 camelCase ↔ snake_case 로 맞춘다)
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { MascotKind, NotifyOptions, NotifyPosition, SessionAction, Snapshot, State } from "./types";

export const inTauri = isTauri();

type Patch = (s: Snapshot) => void;

let current: Snapshot | null = null;
const listeners = new Set<(s: Snapshot) => void>();
// 누른 즉시 화면에 보이도록 먼저 고친 내용 — 백엔드의 다음 상태 묶음이 아직 반영하지 못했을 수 있어 잠깐 다시 덮는다.
// 그래서 패치는 "뒤집기" 가 아니라 "이 값으로" 처럼 몇 번 덮어도 같은 결과여야 한다
let pending: { until: number; patch: Patch }[] = [];
let started = false;

function emit(s: Snapshot) {
  current = s;
  for (const l of listeners) l(s);
}

function publish(s: Snapshot) {
  const now = Date.now();
  pending = pending.filter((p) => p.until > now);
  for (const p of pending) p.patch(s);
  emit(s);
}

async function start() {
  if (started) return;
  started = true;
  if (inTauri) {
    await listen<Snapshot>("board://snapshot", (e) => publish(e.payload));
    publish(await invoke<Snapshot>("get_snapshot"));
  } else {
    const mock = await import("./mock");
    mock.startMock(publish);
  }
}

// 상태 묶음을 받는다 — 처음 부를 때 백엔드(또는 가짜 상태)를 연결한다
export function onSnapshot(fn: (s: Snapshot) => void): () => void {
  listeners.add(fn);
  if (current) fn(current);
  void start();
  return () => listeners.delete(fn);
}

function optimistic(patch: Patch) {
  pending.push({ until: Date.now() + 1500, patch });
  if (!current) return;
  const s = structuredClone(current);
  patch(s);
  emit(s);
}

async function call<T = void>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    if (inTauri) return await invoke<T>(cmd, args);
    const mock = await import("./mock");
    return (await mock.handle(cmd, args ?? {})) as T;
  } catch (e) {
    console.error(`${cmd} 실패`, e);
    throw e;
  }
}

// 실패해도 화면은 다음 상태 묶음으로 바로잡히므로 누른 쪽에서는 기다리지 않는다
function fire(cmd: string, args?: Record<string, unknown>) {
  call(cmd, args).catch(() => {});
}

function setIn(list: string[], item: string, on: boolean) {
  const i = list.indexOf(item);
  if (on && i < 0) list.push(item);
  if (!on && i >= 0) list.splice(i, 1);
}

// 세션을 뺀 뒤 상태별 개수를 다시 센다
function recount(s: Snapshot) {
  const c: Record<State, number> = { waiting: 0, working: 0, idle: 0, paused: 0, ended: 0 };
  for (const g of s.groups) for (const x of g.sessions) c[x.state]++;
  s.counts = c;
}

const has = (list: string[] | undefined, item: string) => !!list?.includes(item);

// ── 보드 설정·표지 ─────────────────────────────────────────────

export function toggleCollapsed(cwd: string) {
  const on = !has(current?.settings.collapsed, cwd);
  optimistic((s) => setIn(s.settings.collapsed, cwd, on));
  fire("toggle_collapsed", { cwd });
}

export function setAllCollapsed(collapse: boolean) {
  optimistic((s) => {
    s.settings.collapsed = collapse ? s.groups.map((g) => g.id) : [];
  });
  fire("set_all_collapsed", { collapse });
}

export function toggleEndedOpen(cwd: string) {
  const on = !has(current?.settings.endedOpen, cwd);
  optimistic((s) => setIn(s.settings.endedOpen, cwd, on));
  fire("toggle_ended_open", { cwd });
}

export function togglePinGroup(cwd: string) {
  const on = !has(current?.settings.pinnedGroups, cwd);
  optimistic((s) => setIn(s.settings.pinnedGroups, cwd, on));
  fire("toggle_pin_group", { cwd });
}

export function hideGroup(cwd: string) {
  optimistic((s) => {
    s.groups = s.groups.filter((g) => g.id !== cwd);
    setIn(s.settings.pinnedGroups, cwd, false);
    recount(s);
  });
  fire("hide_group", { cwd });
}

export function removeEnded(cwd: string) {
  optimistic((s) => {
    for (const g of s.groups) if (g.id === cwd) g.sessions = g.sessions.filter((x) => x.state !== "ended");
    s.groups = s.groups.filter((g) => g.sessions.length > 0);
    recount(s);
  });
  fire("remove_ended", { cwd });
}

export function removeSession(id: string) {
  optimistic((s) => {
    for (const g of s.groups) g.sessions = g.sessions.filter((x) => x.id !== id);
    s.groups = s.groups.filter((g) => g.sessions.length > 0);
    recount(s);
  });
  fire("remove_session", { id });
}

export function toggleKindCollapsed(key: string) {
  const on = !has(current?.settings.kindCollapsed, key);
  optimistic((s) => setIn(s.settings.kindCollapsed, key, on));
  fire("toggle_kind_collapsed", { key });
}

export function toggleSubCollapsed(id: string) {
  const on = !has(current?.settings.subCollapsed, id);
  optimistic((s) => setIn(s.settings.subCollapsed, id, on));
  fire("toggle_sub_collapsed", { id });
}

export function setUsageCompact(on: boolean) {
  optimistic((s) => {
    s.settings.usageCompact = on;
  });
  fire("set_usage_compact", { on });
}

export function setMute(id: string, muted: boolean) {
  optimistic((s) => {
    for (const g of s.groups) for (const x of g.sessions) if (x.id === id) x.muted = muted;
  });
  fire("set_mute", { id, muted });
}

export function setAllMuted(on: boolean) {
  optimistic((s) => {
    s.allMuted = on;
  });
  fire("set_all_muted", { on });
}

// 요약 칩으로 찾아갈 때 — 카드와 그 안의 터미널·백그라운드 구역을 편다
export function reveal(cwd: string) {
  optimistic((s) => {
    setIn(s.settings.collapsed, cwd, false);
    s.settings.kindCollapsed = s.settings.kindCollapsed.filter((k) => !k.startsWith(cwd + "|"));
  });
  fire("reveal", { cwd });
}

export function setPinned(on: boolean) {
  optimistic((s) => {
    s.settings.pinned = on;
  });
  fire("set_pinned", { on });
}

export function setMascot(kind: MascotKind) {
  optimistic((s) => {
    s.settings.mascot = kind;
  });
  fire("set_mascot", { kind });
}

export function setOpacity(v: number) {
  optimistic((s) => {
    s.settings.opacity = v;
  });
  fire("set_opacity", { v });
}

export function setNotifyPosition(pos: NotifyPosition) {
  optimistic((s) => {
    s.settings.notifyPosition = pos;
  });
  fire("set_notify_position", { pos });
}

// 창 크기 단계가 미니인지 — 미니로 둔 채 끄면 다음에도 그 크기로 연다(§5.9 boardMini).
// SPEC §6.2 목록에 아직 없는 명령이라 백엔드에 없으면 조용히 넘어간다
export function setBoardMini(on: boolean) {
  call("set_board_mini", { on }).catch(() => {});
}

// ── 세션·계정·업데이트 ─────────────────────────────────────────

// activate = 그 세션의 터미널 앱만 앞으로, focus = 그 세션 창(탭)으로 이동, resume = 종료 세션 이어 열기,
// pause·resume_paused·stop = 일시중지·재개·중단(중단 확인 대화상자는 백엔드가 띄운다)
export function sessionAction(action: SessionAction, id: string) {
  fire("session_action", { action, id });
}

export function switchAccount(id: string) {
  fire("switch_account", { id });
}

export function addAccount(email?: string) {
  fire("add_account", { email: email ?? null });
}

export function cancelAddAccount() {
  fire("cancel_add_account");
}

export function removeAccount(id: string) {
  fire("remove_account", { id });
}

export function setAutoSwitch(on: boolean) {
  optimistic((s) => {
    s.accounts.autoSwitch = on;
    s.settings.autoSwitch = on;
  });
  fire("set_auto_switch", { on });
}

export function checkUpdate(manual: boolean) {
  if (manual) {
    optimistic((s) => {
      s.updater.checking = true;
    });
  }
  fire("check_update", { manual });
}

export function runUpdate() {
  fire("run_update");
}

export function openChangelog() {
  fire("open_changelog");
}

// ── 창·터미널 ──────────────────────────────────────────────────

export function openTerminal(id: string) {
  fire("open_terminal", { id });
}

export function enterMini(rows: number) {
  fire("enter_mini", { rows });
}

export function leaveMini() {
  fire("leave_mini");
}

export function snapRight() {
  fire("snap_right");
}

export function hideBoard() {
  fire("hide_board");
}

export function minimizeBoard() {
  fire("minimize_board");
}

export function log(text: string) {
  fire("log", { text });
}

// ── 알림 패널(/notify) — §6.3 ──────────────────────────────────

// 브라우저에서는 주소의 쿼리(title·subtitle·body·kind·summary·duration·position)로 받는다
export async function getNotifyOptions(): Promise<NotifyOptions> {
  if (inTauri) return invoke<NotifyOptions>("get_notify_options");
  const q = new URLSearchParams(location.search);
  const kind = q.get("kind") === "wait" ? "wait" : "done";
  const summary: NotifyOptions["summary"] = {};
  for (const pair of (q.get("summary") ?? "").split(",")) {
    const [k, v] = pair.split("=");
    const n = Number(v);
    if ((k === "waiting" || k === "working" || k === "idle") && Number.isFinite(n)) summary[k] = n;
  }
  return {
    title: q.get("title") ?? (kind === "wait" ? "승인 대기" : "작업 완료"),
    subtitle: q.get("subtitle") ?? "",
    body: q.get("body") ?? "",
    kind,
    summary,
    duration: Number(q.get("duration")) || 10,
    position: (q.get("position") as NotifyPosition) || "center",
  };
}

// 백엔드가 이 높이로 자리를 잡고 창을 보인 뒤에 돌아온다 — 그다음에 나타나는 애니메이션을 시작한다
export async function notifyReady(height: number) {
  if (inTauri) await invoke("notify_ready", { height }).catch((e) => console.error(e));
}

export function notifyHover(inside: boolean) {
  if (inTauri) invoke("notify_hover", { inside }).catch(() => {});
}

export function notifyClicked() {
  if (inTauri) invoke("notify_clicked").catch(() => {});
}

export function notifyFinish() {
  if (inTauri) invoke("notify_finish").catch(() => {});
}
