<svelte:options namespace="svg" />

<script lang="ts">
  // 트랄랄레로 트랄랄라 — 파란 상어가 세 다리에 운동화를 신고 걷는다. 재촉하면 제자리 뛰기, 졸리면 멈춘다
  import type { Mood } from "$lib/types";
  import { INK, capsule, cycle, gray, period, poly, rgb } from "../draw";
  import CartoonEye from "./CartoonEye.svelte";
  import Sneaker from "./Sneaker.svelte";
  import Teeth from "./Teeth.svelte";

  let { mood, t, blink }: { mood: Mood; t: number; blink: boolean } = $props();
  const blue = rgb(0.36, 0.58, 0.86);
  const belly = rgb(0.9, 0.95, 1.0);
  const beat = $derived(cycle(t, period(mood)));
  const wave = $derived(Math.sin(2 * Math.PI * beat));
  const lift = $derived(mood === "sleepy" ? 0 : mood === "idle" ? 1.5 : 3);
</script>

{#each [0, 1, 2] as i}
  {@const up = Math.max(0, Math.sin(2 * Math.PI * beat + i * 2.1))}
  {@const x = (i - 1) * 7 - 2}
  {@const y = 13 - lift * up}
  <rect {...capsule(x, y - 1.25, 2.6, 8)} fill={blue} />
  <Sneaker {x} y={y + 3.5} />
{/each}
<!-- 꼬리 — 박자에 맞춰 흔든다 -->
<polygon points={poly(18, -5, 10, 13, [[0, 0.5], [1, 0], [0.7, 0.5], [1, 1]])} fill={blue} transform="rotate({12 * wave} 13 -5)" />
<!-- 등지느러미 -->
<polygon points={poly(2, -14, 9, 8, [[0, 1], [0.55, 0], [1, 1]])} fill={blue} />
<rect {...capsule(0, -4, 32, 15)} fill={blue} />
<rect {...capsule(-3, 0.5, 22, 6)} fill={belly} />
<!-- 아가미 -->
{#each [-1.3, 1, 3.3] as x}
  <rect {...capsule(x, -5, 0.8, 5)} fill={gray(0.12, 0.35)} />
{/each}
<CartoonEye x={-9} y={-7} size={5.5} {mood} {blink} />
<!-- 입 -->
<rect {...capsule(-11, -1.5, 9, 1.2)} fill={INK} />
<Teeth x={-11} y={0.3} n={4} w={8} h={2.5} />
