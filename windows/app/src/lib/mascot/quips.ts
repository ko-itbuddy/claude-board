// 지금 세션 목록·사용 한도로 캐릭터가 할 말을 만든다 (ClaudeBoard.swift foremanQuips). 시각은 유닉스 밀리초
import type { Group, Limit, Mood, Session } from "$lib/types";

// 말풍선 한 줄 — 기분과 함께
export interface Quip {
  text: string;
  mood: Mood;
}

// 경로의 마지막 이름 — Windows 경로(\)와 / 를 모두 나눈다
const name = (s: Session) => s.cwd.split(/[\\/]/).filter(Boolean).pop() ?? s.cwd;

// Swift String.count 처럼 글자(grapheme) 단위로 자른다
const segmenter = new Intl.Segmenter();
const short = (t: string, n: number) => {
  const g = Array.from(segmenter.segment(t), (x) => x.segment);
  return g.length > n ? g.slice(0, n).join("") + "…" : t;
};

export function foremanQuips(groups: Group[], limits: Limit[], now: number): Quip[] {
  const live = groups.flatMap((g) => g.sessions).filter((s) => s.state !== "ended");
  const waiting = live.filter((s) => s.state === "waiting");
  const working = live.filter((s) => s.state === "working");
  const idle = live.filter((s) => s.state === "idle");
  const paused = live.filter((s) => s.state === "paused");
  const out: Quip[] = [];

  if (waiting.length > 0) {
    out.push({ text: waiting.length === 1 ? "승인 기다리는 세션이 있어요!" : `승인 대기 ${waiting.length}개! 확인해 줘요`, mood: "urgent" });
    for (const s of waiting.slice(0, 3)) {
      out.push({ text: s.topic === "" ? `${name(s)} 에서 기다려요!` : `${name(s)}: ${short(s.topic, 12)} — 승인 부탁!`, mood: "urgent" });
    }
    return out; // 승인 대기가 있으면 그것만 재촉한다
  }
  if (working.length > 0) {
    out.push({ text: working.length === 1 ? `${name(working[0])} 열일 중! 힘내!` : `세션 ${working.length}개 동시에 달리는 중!`, mood: "busy" });
    for (const s of working.slice(0, 2)) {
      const mins = Math.trunc((now - s.since) / 60000);
      if (mins >= 10) out.push({ text: `${name(s)} ${mins}분째 작업 중… 조금만 더!`, mood: "busy" });
      else if (s.topic !== "") out.push({ text: `지금은 ${short(s.topic, 14)} 처리 중`, mood: "busy" });
    }
    const subs = working.reduce((n, s) => n + s.subagents.filter((a) => a.state !== "done").length, 0);
    if (subs > 0) out.push({ text: `서브에이전트 ${subs}명 출동!`, mood: "busy" });
    const shells = working.reduce((n, s) => n + s.shells, 0);
    if (shells > 0) out.push({ text: `백그라운드 셸 ${shells}개 지켜보는 중`, mood: "busy" });
  }
  for (const s of live.filter((s) => (s.ctx?.pct ?? 0) >= 80).slice(0, 2)) {
    out.push({ text: `${name(s)} 컨텍스트 ${s.ctx!.pct}%! 정리할 때예요`, mood: working.length === 0 ? "idle" : "busy" });
  }
  for (const l of limits) {
    if (l.pct >= 80) out.push({ text: `${l.label} 한도 ${Math.round(l.pct)}% — 아껴 써요!`, mood: working.length === 0 ? "idle" : "busy" });
  }
  if (paused.length > 0) out.push({ text: `일시중지 ${paused.length}개, 잊지 마세요`, mood: "idle" });
  if (working.length === 0) {
    // 가장 최근에 끝난 대기 세션 — 같으면 앞의 것(Swift max(by:))
    const s = idle.reduce<Session | null>((m, x) => (m === null || x.updated > m.updated ? x : m), null);
    if (s) {
      if (now - s.updated > 30 * 60 * 1000) {
        out.push({ text: "다들 쉬는 중… 할 일 없나요?", mood: "sleepy" });
        out.push({ text: "Zzz… 다음 지시 기다려요", mood: "sleepy" });
      } else {
        out.push({ text: `${name(s)} 끝났어요! 확인해 볼까요?`, mood: "idle" });
        out.push({ text: idle.length === 1 ? "다음 작업 뭐 할까요?" : `세션 ${idle.length}개 대기 중 — 다음 지시!`, mood: "idle" });
      }
    } else if (out.length === 0) {
      out.push({ text: "세션을 열어 일을 맡겨 주세요!", mood: "sleepy" });
      out.push({ text: "오늘도 화이팅!", mood: "sleepy" });
    }
  }
  return out;
}
