<svelte:options namespace="svg" />

<script lang="ts">
  // 카푸치노 아사시노 — 닌자 복면을 쓴 카푸치노 잔. 카타나를 휘두르고, 바쁠 땐 속도선을 남기며 오간다
  import type { Mood } from "$lib/types";
  import { INK, capsule, circle, cycle, ellipse, gray, period, rect, rgb, swing, uneven } from "../draw";
  import CartoonEye from "./CartoonEye.svelte";

  let { mood, t, blink }: { mood: Mood; t: number; blink: boolean } = $props();
  const cup = gray(0.96);
  const coffee = rgb(0.55, 0.36, 0.22);
  const foam = rgb(0.97, 0.89, 0.76);
  const band = rgb(0.88, 0.22, 0.24);
  const wave = $derived(Math.sin(2 * Math.PI * cycle(t, period(mood))));
  const slash = $derived(
    mood === "urgent" ? -70 + 110 * swing(t, 0.25)
    : mood === "busy" ? -40 + 60 * swing(t, 0.6)
    : mood === "idle" ? -35 + 6 * wave
    : 40,
  );
  const dash = $derived(mood === "busy" ? 3 * Math.sin((2 * Math.PI * t) / 0.6) : 0);
  const flutter = $derived(18 * Math.sin((2 * Math.PI * t) / (mood === "sleepy" ? 2.0 : 0.35)));
</script>

<g transform="translate({dash} 0)">
  {#if mood === "busy" || mood === "urgent"}
    <!-- 속도선 -->
    {#each [0, 1, 2] as i}
      <rect {...capsule(20, i * 5 - 4, 6 + 3 * (i % 2), 1)} fill={gray(1, 0.35)} />
    {/each}
  {/if}
  <!-- 다리 -->
  {#each [-3.75, 3.75] as x}
    <rect {...capsule(x, 18, 2.5, 7)} fill={INK} />
  {/each}
  <!-- 머리띠 꼬리 — 바람에 날린다 -->
  {#each [0, 1] as i}
    <rect {...capsule(14, -9 + i * 2, 8, 2)} fill={band} transform="rotate({flutter + i * 25 - 10} 10 {-9 + i * 2})" />
  {/each}
  <!-- 손잡이 -->
  <circle {...circle(11, 4, 8)} fill="none" stroke={cup} stroke-width="2.5" />
  <path d={uneven(0, 3, 22, 21, 2, 7, 7, 2)} fill={cup} />
  <ellipse {...ellipse(0, -7.5, 22, 6)} fill={coffee} />
  <ellipse {...ellipse(0, -7.5, 15, 3.5)} fill={foam} />
  <!-- 닌자 복면 — 눈만 드러난다 -->
  <rect {...rect(0, -1, 22, 7)} fill={gray(0.14)} />
  <rect {...rect(0, -4.5, 22, 1.5)} fill={band} />
  <CartoonEye x={-4.25} y={-1} size={4.5} {mood} {blink} />
  <CartoonEye x={4.25} y={-1} size={4.5} {mood} {blink} />
  <!-- 팔 -->
  <rect {...capsule(-12, 6, 5, 2)} fill={INK} />
  <!-- 카타나 — 손잡이 가운데(손)가 회전 중심 -->
  <g transform="rotate({slash} -14 6)">
    <rect {...capsule(-14, -6.5, 2, 16)} fill={gray(0.88)} />
    <rect {...capsule(-14, 2.3, 6, 1.6)} fill={rgb(0.85, 0.7, 0.3)} />
    <rect {...capsule(-14, 6.1, 2.4, 6)} fill={gray(0.15)} />
  </g>
</g>
