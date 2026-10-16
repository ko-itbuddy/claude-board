<svelte:options namespace="svg" />

<script lang="ts">
  // 봄바르디로 크로코딜로 — 악어 머리를 단 폭격기. 프로펠러가 돌고, 재촉하면 폭탄을 떨어뜨린다
  import type { Mood } from "$lib/types";
  import { capsule, circle, cycle, gray, poly, rect, rgb } from "../draw";
  import CartoonEye from "./CartoonEye.svelte";
  import Teeth from "./Teeth.svelte";

  let { mood, t, blink }: { mood: Mood; t: number; blink: boolean } = $props();
  const green = rgb(0.38, 0.64, 0.34);
  const dark = rgb(0.26, 0.46, 0.24);
  const metal = rgb(0.62, 0.66, 0.7);
  const spin = $derived(Math.abs(Math.cos((2 * Math.PI * t) / (mood === "sleepy" ? 1.2 : mood === "idle" ? 0.25 : 0.1))));
  const drop = $derived(cycle(t, 0.6));
</script>

{#if mood === "urgent"}
  <rect {...capsule(4, 6 + 16 * drop, 4, 6)} fill={gray(0.22)} opacity={1 - drop} />
{/if}
<!-- 먼 쪽 날개 — 몸 뒤로 비스듬히 -->
<polygon points={poly(7, -9, 18, 8, [[0.3, 1], [0.8, 1], [1, 0], [0.6, 0]])} fill={rgb(0.62, 0.66, 0.7, 0.7)} />
<!-- 꼬리날개 -->
<polygon points={poly(17, -8, 8, 9, [[0, 1], [0.35, 0], [1, 0], [0.9, 1]])} fill={metal} />
<rect {...capsule(3, 0, 30, 12)} fill={green} />
<!-- 등 돌기 -->
{#each [4.5, 9, 13.5] as x}
  <polygon points={poly(x, -6.5, 2.5, 2, [[0, 1], [0.5, 0], [1, 1]])} fill={dark} />
{/each}
<!-- 주둥이 -->
<rect {...rect(-15, 2, 15, 6, 3)} fill={green} />
<rect {...capsule(-15, 3, 12, 1)} fill={dark} />
<Teeth x={-15} y={4.5} n={4} w={10} h={2} />
<!-- 콧구멍 -->
<circle {...circle(-21, 0, 1.5)} fill={dark} />
<!-- 툭 튀어나온 눈두덩 -->
<circle {...circle(-8, -5, 8)} fill={green} />
<CartoonEye x={-8} y={-5} size={5} {mood} {blink} />
<!-- 가까운 쪽 날개 — 몸 아래 앞으로 -->
<polygon points={poly(3, 8, 22, 8, [[0.15, 0], [0.75, 0], [0.6, 1], [0, 1]])} fill={metal} />
<!-- 날개 위 엔진 -->
<rect {...capsule(-3, 10, 7, 4.5)} fill={gray(0.35)} />
<!-- 프로펠러 — 길이가 줄었다 늘며 도는 것처럼 보인다 -->
<rect {...capsule(0, 0, 2, 12)} fill={gray(0.9, 0.85)} transform="translate(-7.5 10) scale(1 {0.15 + 0.85 * spin})" />
