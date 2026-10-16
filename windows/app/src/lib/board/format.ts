// 보드 글자·색 규칙 — ClaudeBoard.swift 의 Palette·elapsed·clock·resetText·tokens 와 같은 결과를 낸다
import type { Group, Session, State, Sub } from "$lib/types";

// "r g b" — CSS 에서 rgb(var(--c) / 알파) 로 쓴다
export function stateRgb(state: string): string {
  switch (state) {
    case "working":
      return "115 168 255";
    case "waiting":
      return "255 168 92";
    case "idle":
      return "179 191 219"; // 유휴 — 완료로 읽히지 않게 초록 대신 차분한 회청색
    case "paused":
      return "242 204 89";
    default:
      return "140 140 140";
  }
}

export function stateName(state: string): string {
  return (
    ({ working: "작업 중", waiting: "승인 대기", idle: "대기", paused: "일시중지", ended: "종료" } as Record<string, string>)[
      state
    ] ?? state
  );
}

export const rank: Record<State, number> = { waiting: 0, working: 1, paused: 2, idle: 2, ended: 3 };

// 모델 계열 색 — Opus·Sonnet·Haiku·Fable
export function familyRgb(family: string): string {
  switch (family) {
    case "Opus":
      return "255 153 115";
    case "Sonnet":
      return "122 179 255";
    case "Haiku":
      return "140 224 204";
    case "Fable":
      return "224 179 255";
    default:
      return "191 191 191";
  }
}

// 사용 한도 막대 색 — 50% 미만 초록, 80% 미만 주황, 그 이상 빨강
export function usageRgb(p: number): string {
  return p < 50 ? "107 217 153" : p < 80 ? "255 179 92" : "255 107 107";
}

// 컨텍스트 색 — 30% 미만 초록, 70% 미만 노랑, 그 이상 빨강(상태줄과 같은 기준)
export function ctxRgb(p: number): string {
  return p < 30 ? "107 217 153" : p < 70 ? "255 204 89" : "255 107 107";
}

const pad = (n: number) => String(n).padStart(2, "0");

// 종료된 세션은 흐르는 시간 대신 끝난 시각을 고정해서 보여 준다
export function clock(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function elapsed(from: number, now: number): string {
  const s = Math.max(0, Math.floor((now - from) / 1000));
  if (s < 60) return `${s}초`;
  if (s < 3600) return `${Math.floor(s / 60)}분 ${s % 60}초`;
  return `${Math.floor(s / 3600)}시간 ${Math.floor((s % 3600) / 60)}분`;
}

// 남은 시간 + 초기화 시각 — 하루 넘게 남으면 날짜를 붙인다
export function resetText(at: number | null, now: number): string {
  if (at == null) return "";
  const s = Math.floor((at - now) / 1000);
  if (s <= 0) return "곧 초기화";
  const left =
    s >= 86400
      ? `${Math.floor(s / 86400)}일 ${Math.floor((s % 86400) / 3600)}시간`
      : s >= 3600
        ? `${Math.floor(s / 3600)}시간 ${Math.floor((s % 3600) / 60)}분`
        : `${Math.max(1, Math.floor(s / 60))}분`;
  const d = new Date(at);
  const when =
    s >= 86400 ? `${d.getMonth() + 1}/${d.getDate()}(${"일월화수목금토"[d.getDay()]}) ${clock(at)}` : clock(at);
  return `${left} 뒤 · ${when}`;
}

export function tokens(n: number): string {
  return n >= 1_000_000 ? `${(n / 1_000_000).toFixed(1)}M` : n >= 1000 ? `${Math.floor(n / 1000)}K` : `${n}`;
}

export const grouped = (n: number) => n.toLocaleString("ko-KR");

// 그룹에서 가장 급한 상태
export function urgent(g: Group): State {
  let best: State = "idle";
  let r = 99;
  for (const s of g.sessions) {
    if (rank[s.state] < r) {
      r = rank[s.state];
      best = s.state;
    }
  }
  return best;
}

// 작업 중·승인 대기 세션 — sessions 는 급한 순이라 첫째가 대표다
export const active = (g: Group) => g.sessions.filter((s) => s.state === "waiting" || s.state === "working");

// 세션이 지금 하는 일 — 승인 대기면 요청 내용, 아니면 세션 제목. 접힌 카드와 미니 화면이 쓴다
export const doing = (s: Session) => (s.state === "waiting" && s.message ? s.message : s.title);

// 서브에이전트 상태를 보드 상태 이름으로 바꿔 아이콘·색을 세션과 같이 쓴다
export const subState = (s: Sub): State =>
  (({ running: "working", waiting: "waiting", done: "idle" }) as Record<string, State>)[s.state] ?? "working";

export const lastName = (path: string) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;
