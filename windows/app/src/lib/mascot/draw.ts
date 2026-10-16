// 캐릭터 그림 도우미 — ClaudeBoard.swift 의 SwiftUI 도형을 SVG 로 옮길 때 쓴다.
// 좌표는 46×44 틀의 가운데가 (0, 0) 이라 SwiftUI 의 .offset(x, y) 값이 그대로 도형의 가운데가 된다
import type { Mood } from "$lib/types";

// 0 → 1 을 period 초마다 되풀이 (Swift cycle)
export const cycle = (t: number, period: number) => (t / period) % 1;
// 0 → 1 → 0 을 부드럽게 오가는 값 — period 동안 한 번 왕복 (Swift swing)
export const swing = (t: number, period: number) => (1 - Math.cos(2 * Math.PI * cycle(t, period))) / 2;

// 브레인롯 캐릭터의 기분별 박자 — 재촉 0.3초 · 작업 0.6초 · 대기 1.6초 · 졸음 3초에 한 번
export const period = (mood: Mood) =>
  mood === "urgent" ? 0.3 : mood === "busy" ? 0.6 : mood === "idle" ? 1.6 : 3.0;

// SwiftUI Color(red:green:blue:) 처럼 0~1 값으로 색을 만든다
export const rgb = (r: number, g: number, b: number, a = 1) =>
  `rgba(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)},${a})`;
export const gray = (w: number, a = 1) => rgb(w, w, w, a);
export const INK = gray(0.12);
// SwiftUI Color.yellow — 보드는 늘 어두운 모드라 어두운 모드 값
export const YELLOW = "rgb(255,214,10)";

// Capsule — 짧은 변의 절반이 모서리 반지름
export const capsule = (x: number, y: number, w: number, h: number) => ({
  x: x - w / 2, y: y - h / 2, width: w, height: h, rx: Math.min(w, h) / 2,
});
export const rect = (x: number, y: number, w: number, h: number, r = 0) => ({
  x: x - w / 2, y: y - h / 2, width: w, height: h, rx: r,
});
export const ellipse = (x: number, y: number, w: number, h: number) => ({ cx: x, cy: y, rx: w / 2, ry: h / 2 });
export const circle = (x: number, y: number, d: number) => ({ cx: x, cy: y, r: d / 2 });

// Poly — 0~1 비율 꼭짓점을 w×h 틀에 놓은 다각형
export const poly = (x: number, y: number, w: number, h: number, pts: [number, number][]) =>
  pts.map(([px, py]) => `${x - w / 2 + px * w},${y - h / 2 + py * h}`).join(" ");

// 모서리마다 반지름이 다른 사각형(UnevenRoundedRectangle).
// 한 변의 두 반지름 합이 변보다 길면 모든 반지름을 같은 비율로 줄인다(감독 안전모가 이 경우)
export function uneven(x: number, y: number, w: number, h: number, tl: number, bl: number, br: number, tr: number) {
  const f = Math.min(1, w / (tl + tr), w / (bl + br), h / (tl + bl), h / (tr + br));
  [tl, bl, br, tr] = [tl * f, bl * f, br * f, tr * f];
  const x0 = x - w / 2, y0 = y - h / 2, x1 = x + w / 2, y1 = y + h / 2;
  return `M${x0 + tl},${y0}H${x1 - tr}A${tr},${tr} 0 0 1 ${x1},${y0 + tr}V${y1 - br}A${br},${br} 0 0 1 ${x1 - br},${y1}` +
    `H${x0 + bl}A${bl},${bl} 0 0 1 ${x0},${y1 - bl}V${y0 + tl}A${tl},${tl} 0 0 1 ${x0 + tl},${y0}Z`;
}

// 입 모양 곡선 — (x0, y) 에서 (x0 + w, y) 까지, 조절점이 가운데 아래 dy
export const smile = (x0: number, y: number, w: number, dy: number) => `M${x0},${y}Q${x0 + w / 2},${y + dy} ${x0 + w},${y}`;
