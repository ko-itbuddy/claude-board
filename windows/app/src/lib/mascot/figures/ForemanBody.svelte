<svelte:options namespace="svg" />

<script lang="ts">
  // 현장 감독 — 안전모를 쓴 둥근 얼굴에 지휘봉 든 팔. 팔 각도·착지 들썩임은 Figure 가 계산해 준다
  import type { Mood } from "$lib/types";
  import { INK, YELLOW, capsule, circle, ellipse, gray, rgb, smile, swing, uneven } from "../draw";

  let { mood, t, blink, armDeg, land }: { mood: Mood; t: number; blink: boolean; armDeg: number; land: number } = $props();
  const skin = $derived(mood === "urgent" ? rgb(0.93, 0.42, 0.32) : rgb(0.85, 0.47, 0.34));
  const shout = $derived(3 + 3 * swing(t, 0.3));
</script>

<!-- 지휘봉 든 팔 — 어깨를 축으로 돈다(양수 = 위로 든다) -->
<g transform="rotate({armDeg} 2 4)">
  <rect {...capsule(-15, 4, 16, 4)} fill={skin} />
  <rect {...capsule(-27.5, 4, 13, 2.5)} fill={gray(0.95)} />
  <circle {...circle(-34, 4, 4)} fill={mood === "urgent" ? YELLOW : "#fff"} />
</g>
<!-- 얼굴 -->
<circle {...circle(0, 0, 28)} fill={skin} />
<!-- 눈 — 말풍선 쪽(왼쪽)을 흘끗 본다 -->
{#each [-1, 1] as side}
  {#if mood === "sleepy"}
    <rect {...capsule(side * 6, 1, 5, 1.2)} fill={INK} />
  {:else if mood === "urgent"}
    <circle {...circle(-1.5 + side * 6, 1, 5)} fill={INK} />
  {:else}
    <rect {...capsule(-1.5 + side * 5.25, 1, 3.5, blink ? 0.8 : 5)} fill={INK} />
  {/if}
{/each}
<!-- 입 — 아래 끝을 얼굴 아래에서 5 위에 맞춘다 -->
{#if mood === "urgent"}
  <!-- 소리치는 입 — 열렸다 닫힌다 -->
  <ellipse {...ellipse(0, 9 - shout / 2, 7, shout)} fill={INK} />
{:else if mood === "busy"}
  <rect {...capsule(0, 8, 8, 2)} fill={INK} />
{:else if mood === "idle"}
  <!-- 웃는 입 -->
  <path d={smile(-4, 5, 8, 4)} fill="none" stroke={INK} stroke-width="1.6" stroke-linecap="round" />
{:else}
  <circle {...circle(0, 7.5, 3)} fill={INK} />
{/if}
<!-- 볼 -->
{#each [-9, 9] as x}
  <ellipse {...ellipse(x, 6, 5, 3)} fill={rgb(1, 0.55, 0.55, mood === "urgent" ? 0.95 : 0.7)} />
{/each}
<!-- 안전모 — 착지할 때 살짝 들썩인다 -->
<path d={uneven(0, -13 - 2 * land, 30, 10, 10, 2, 2, 10)} fill={rgb(1, 0.8, 0.25)} />
