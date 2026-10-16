// 제목 오른쪽 캐릭터 — 브레인롯 7종과 현장 감독 (ClaudeBoard.swift Mascot)
import type { MascotKind } from "$lib/types";

export const MASCOTS: { kind: MascotKind; name: string; catchphrase: string | null }[] = [
  { kind: "tralalero", name: "트랄랄레로 트랄랄라", catchphrase: "트랄랄레로 트랄랄라~" },
  { kind: "bombardiro", name: "봄바르디로 크로코딜로", catchphrase: "봄바르디로 크로코딜로!" },
  { kind: "tungtung", name: "퉁 퉁 퉁 사후르", catchphrase: "퉁 퉁 퉁 퉁 사후르!" },
  { kind: "lirili", name: "리릴리 라릴라", catchphrase: "리릴리 라릴라~" },
  { kind: "patapim", name: "브르르 브르르 파타핌", catchphrase: "브르르 브르르 파타핌!" },
  { kind: "boneca", name: "보네카 암발라부", catchphrase: "보네카 암발라부!" },
  { kind: "cappuccino", name: "카푸치노 아사시노", catchphrase: "카푸치노 아사시노!" },
  // 말풍선에 가끔 끼우는 유행어 — 감독은 없다
  { kind: "foreman", name: "현장 감독", catchphrase: null },
];
