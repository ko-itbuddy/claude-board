<svelte:options namespace="svg" />

<script lang="ts">
  // 리릴리 라릴라 — 코끼리 머리에 선인장 몸, 샌들, 자명종. 귀가 펄럭이고 코가 흔들리며, 재촉하면 자명종이 울린다
  import type { Mood } from "$lib/types";
  import { INK, capsule, circle, cycle, ellipse, period, rgb } from "../draw";
  import CartoonEye from "./CartoonEye.svelte";

  let { mood, t, blink }: { mood: Mood; t: number; blink: boolean } = $props();
  const cactus = rgb(0.36, 0.68, 0.4);
  const spine = rgb(0.86, 0.95, 0.8);
  const grey = rgb(0.64, 0.66, 0.72);
  const pink = rgb(0.95, 0.72, 0.76);
  const sandal = rgb(0.62, 0.4, 0.22);
  const bell = rgb(0.92, 0.3, 0.28);
  const spines: [number, number][] = [[-3, 4], [2, 7], [-2, 12], [3, 14], [-10, 2]];
  const wave = $derived(Math.sin(2 * Math.PI * cycle(t, period(mood))));
  const sway = $derived((mood === "sleepy" ? 4 : mood === "urgent" ? 22 : 14) * wave);
  const ring = $derived(mood === "urgent" ? 14 * Math.sin((2 * Math.PI * t) / 0.08) : 0);
  // 귀 — 머리 쪽 끝을 축으로 가로로 펄럭인다
  const flap = $derived(1 + 0.12 * wave);
</script>

<!-- 샌들 신은 발 -->
{#each [-5.5, 5.5] as x}
  <rect {...capsule(x, 18, 3, 5)} fill={cactus} />
  <rect {...capsule(x, 21.5, 7, 2)} fill={sandal} />
{/each}
<!-- 선인장 몸통과 가지 -->
<rect {...capsule(0, 8, 13, 20)} fill={cactus} />
<rect {...capsule(-10, 4, 4, 9)} fill={cactus} />
<rect {...capsule(-8, 8, 6, 3.5)} fill={cactus} />
<!-- 자명종 든 오른쪽 가지 -->
<rect {...capsule(8, 7, 7, 3.5)} fill={cactus} />
{#each spines as [x, y]}
  <circle {...circle(x, y, 1.2)} fill={spine} />
{/each}
<g transform="translate(-5 -10) scale({flap} 1) translate(5 10)">
  <ellipse {...ellipse(-10, -10, 10, 14)} fill={grey} />
  <ellipse {...ellipse(-10, -10, 6, 9)} fill={pink} />
</g>
<g transform="translate(5 -10) scale({flap} 1) translate(-5 10)">
  <ellipse {...ellipse(10, -10, 10, 14)} fill={grey} />
  <ellipse {...ellipse(10, -10, 6, 9)} fill={pink} />
</g>
<circle {...circle(0, -11, 18)} fill={grey} />
<CartoonEye x={-4.75} y={-13} size={4.5} {mood} {blink} />
<CartoonEye x={4.75} y={-13} size={4.5} {mood} {blink} />
<!-- 코 — 위 끝을 축으로 흔들린다 -->
<rect {...capsule(0, 0.5, 5, 13)} fill={grey} transform="rotate({sway} 0 -6)" />
<!-- 자명종 -->
<g transform="rotate({ring} 13 4)">
  <circle {...circle(9, -0.5, 4)} fill={bell} />
  <circle {...circle(17, -0.5, 4)} fill={bell} />
  <circle {...circle(13, 4, 9)} fill="#fff" stroke={bell} stroke-width="1.2" />
  <rect {...capsule(13, 2.8, 0.9, 3)} fill={INK} />
  <rect {...capsule(14, 4, 2.5, 0.9)} fill={INK} />
</g>
