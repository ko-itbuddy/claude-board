<svelte:options namespace="svg" />

<script lang="ts">
  // 아래로 뾰족한 흰 이빨 줄 — 가운데 (x, y), 전체 w×h
  import { poly } from "../draw";

  let { x, y, n, w, h }: { x: number; y: number; n: number; w: number; h: number } = $props();
</script>

{#each { length: n }, i}
  <polygon points={poly(x - w / 2 + (i + 0.5) * (w / n), y, w / n, h, [[0, 0], [1, 0], [0.5, 1]])} fill="#fff" />
{/each}
