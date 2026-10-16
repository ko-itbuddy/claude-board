<svelte:options namespace="svg" />

<script lang="ts">
  // 보네카 암발라부 — 개구리 머리, 타이어 몸통, 사람 다리. 타이어가 구르며 걷고, 재촉하면 입을 벌린다
  import type { Mood } from "$lib/types";
  import { INK, capsule, circle, cycle, ellipse, gray, period, rect, rgb, smile, swing } from "../draw";
  import CartoonEye from "./CartoonEye.svelte";

  let { mood, t, blink }: { mood: Mood; t: number; blink: boolean } = $props();
  const frog = rgb(0.44, 0.74, 0.34);
  const frogDark = rgb(0.3, 0.55, 0.24);
  const skin = rgb(0.93, 0.74, 0.58);
  const beat = $derived(cycle(t, period(mood)));
  const roll = $derived(mood === "sleepy" ? 0 : (t / period(mood)) * 90);
</script>

<!-- 사람 다리 — 번갈아 든다 -->
{#each [0, 1] as i}
  {@const x = i * 7 - 3.5}
  {@const y = 16 - (mood === "sleepy" ? 0 : 2 * Math.max(0, Math.sin(2 * Math.PI * beat + i * Math.PI)))}
  <rect {...capsule(x, y - 0.75, 3, 9)} fill={skin} />
  <rect {...capsule(x - 1.5, y + 4, 6, 2.5)} fill={INK} />
{/each}
<!-- 타이어 몸통 — 굴러간다 -->
<g transform="rotate({roll} 0 4)">
  <circle {...circle(0, 4, 20)} fill={gray(0.17)} />
  {#each { length: 10 }, k}
    <rect {...rect(0, 4 - 8.5, 2, 3)} fill={gray(0.32)} transform="rotate({k * 36} 0 4)" />
  {/each}
  <circle {...circle(0, 4, 8)} fill={gray(0.55)} />
  <circle {...circle(0, 4, 3)} fill={gray(0.3)} />
</g>
<!-- 개구리 머리 -->
<ellipse {...ellipse(0, -9, 26, 13)} fill={frog} />
<!-- 툭 튀어나온 눈 -->
{#each [-7, 7] as x}
  <circle {...circle(x, -15, 9)} fill={frog} />
  <CartoonEye {x} y={-15} size={6} {mood} {blink} />
{/each}
<!-- 점 -->
<circle {...circle(-9, -8, 2)} fill={frogDark} />
<circle {...circle(9, -7, 1.5)} fill={frogDark} />
{#if mood === "urgent"}
  <ellipse {...ellipse(0, -6, 12, 3 + 2 * swing(t, 0.3))} fill={rgb(0.55, 0.15, 0.2)} />
{:else}
  <path d={smile(-8, -7.5, 16, 3)} fill="none" stroke={frogDark} stroke-width="1.2" stroke-linecap="round" />
{/if}
