<svelte:options namespace="svg" />

<script lang="ts">
  // 브르르 브르르 파타핌 — 큰 코의 원숭이 머리에 나무 몸. 잎이 살랑이고 코가 출렁이며, 재촉하면 부르르 떤다
  import type { Mood } from "$lib/types";
  import { capsule, circle, cycle, ellipse, period, rect, rgb } from "../draw";
  import CartoonEye from "./CartoonEye.svelte";

  let { mood, t, blink }: { mood: Mood; t: number; blink: boolean } = $props();
  const fur = rgb(0.72, 0.46, 0.26);
  const face = rgb(0.96, 0.82, 0.64);
  const nose = rgb(0.94, 0.6, 0.46);
  const bark = rgb(0.5, 0.33, 0.2);
  const leaf = rgb(0.42, 0.72, 0.36);
  const leaves: [number, number][] = [[-10, 3], [-7, -1], [10, 3], [7, -1]];
  const wave = $derived(Math.sin(2 * Math.PI * cycle(t, period(mood))));
  const shake = $derived(mood === "urgent" ? 1.2 * Math.sin((2 * Math.PI * t) / 0.05) : 0);
</script>

<g transform="translate({shake} 0)">
  <!-- 큰 발 -->
  {#each [-7, 7] as x}
    <rect {...capsule(x, 20, 8, 3.5)} fill={bark} />
  {/each}
  {#each [-4, 4] as x}
    <rect {...capsule(x, 15, 3, 8)} fill={bark} />
  {/each}
  <!-- 나무 몸통 -->
  <rect {...rect(0, 6, 13, 14, 4)} fill={bark} />
  {#each leaves as [x, y], i}
    <circle {...circle(x, y + 0.8 * wave * (i % 2 === 0 ? 1 : -1), 7)} fill={leaf} />
  {/each}
  <circle {...circle(0, -9, 22)} fill={fur} />
  <ellipse {...ellipse(0, -7, 16, 14)} fill={face} />
  <CartoonEye x={-5} y={-11} size={4} {mood} {blink} />
  <CartoonEye x={5} y={-11} size={4} {mood} {blink} />
  <!-- 큰 코 — 아래로 늘어져 출렁인다 -->
  <ellipse {...ellipse(0, -2, 9, 10)} fill={nose} transform="translate(0 -7) scale(1 {1 + 0.08 * wave}) translate(0 7)" />
</g>
