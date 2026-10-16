<svelte:options namespace="svg" />

<script lang="ts">
  // 만화 눈 — 흰자에 눈동자. 졸리거나 깜빡이면 감고, 재촉할 땐 눈동자가 작아진다(놀란 눈). 눈동자는 말풍선 쪽(왼쪽)을 본다
  import type { Mood } from "$lib/types";
  import { INK, capsule, circle } from "../draw";

  let { x, y, size, mood, blink }: { x: number; y: number; size: number; mood: Mood; blink: boolean } = $props();
</script>

{#if mood === "sleepy" || blink}
  <rect {...capsule(x, y, size, 1.2)} fill={INK} />
{:else}
  <circle {...circle(x, y, size)} fill="#fff" />
  <circle {...circle(x - size * 0.12, y, size * (mood === "urgent" ? 0.42 : 0.56))} fill={INK} />
{/if}
