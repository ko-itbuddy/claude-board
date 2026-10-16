<svelte:options namespace="svg" />

<script lang="ts">
  // 퉁 퉁 퉁 사후르 — 통나무 몸에 야구방망이. 박자에 맞춰 방망이로 바닥을 퉁퉁 내려친다
  import type { Mood } from "$lib/types";
  import { INK, YELLOW, capsule, circle, cycle, ellipse, period, poly, rect, rgb, smile, swing } from "../draw";
  import CartoonEye from "./CartoonEye.svelte";

  let { mood, t, blink }: { mood: Mood; t: number; blink: boolean } = $props();
  const wood = rgb(0.78, 0.57, 0.35);
  const top = rgb(0.92, 0.76, 0.52);
  const bark = rgb(0.52, 0.34, 0.19);
  const batC = rgb(0.95, 0.84, 0.62);
  const beat = $derived(cycle(t, period(mood)));
  // 바닥을 치는 정도(0~1) — 박자 가운데서 짧게 내려친다. 졸리면 방망이를 내려놓는다
  const hit = $derived(mood === "sleepy" ? 0 : Math.abs(Math.sin(Math.PI * beat)) ** (mood === "urgent" ? 2 : 4));
  const angle = $derived(mood === "sleepy" ? 160 : 20 + 130 * hit);
</script>

{#each [-8, 0] as x}
  <rect {...capsule(x, 18, 3, 7)} fill={bark} />
{/each}
<rect {...rect(-4, 1, 20, 30, 5)} fill={wood} />
<!-- 나뭇결 -->
{#each [-8.8, -4, 0.8] as x}
  <rect {...capsule(x, 4, 0.8, 22)} fill={rgb(0.52, 0.34, 0.19, 0.25)} />
{/each}
<!-- 윗면 나이테 -->
<ellipse {...ellipse(-4, -14, 20, 6)} fill={top} />
<ellipse {...ellipse(-4, -14, 11, 3)} fill="none" stroke={rgb(0.52, 0.34, 0.19, 0.45)} stroke-width="0.6" />
<CartoonEye x={-9.75} y={-6} size={6.5} {mood} {blink} />
<CartoonEye x={-0.25} y={-6} size={6.5} {mood} {blink} />
{#if mood === "urgent"}
  <ellipse {...ellipse(-5, 4, 7, 3 + 3 * swing(t, 0.3))} fill={INK} />
{:else}
  <path d={smile(-10, 2.5, 10, 5)} fill="none" stroke={INK} stroke-width="1.4" stroke-linecap="round" />
{/if}
<!-- 방망이 — 손이 회전 중심 -->
<polygon points={poly(8, -6.5, 5, 17, [[0.3, 1], [0, 0.06], [0.5, 0], [1, 0.06], [0.7, 1]])} fill={batC} transform="rotate({angle} 8 2)" />
<!-- 손 -->
<circle {...circle(8, 2, 4)} fill={bark} />
{#if hit > 0.7}
  <text x="19" y="19" font-size="7" font-weight="900" fill={YELLOW} opacity={(hit - 0.7) / 0.3} text-anchor="middle" dominant-baseline="central">퉁</text>
{/if}
