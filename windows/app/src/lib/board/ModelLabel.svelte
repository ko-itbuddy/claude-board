<script lang="ts">
  // 모델·effort 라벨: 계열 이름(색으로 계열 구분) + effort 막대 5칸(low 1 … max 5).
  // level 0 계열·버전, 1 계열만, 2 계열 첫 글자 — 줄이 좁을수록 줄인다
  import type { Brain } from "$lib/types";
  import { familyRgb } from "./format";
  import { hint } from "./hint.svelte";

  let { brain, level = 0 }: { brain: Brain; level?: number } = $props();

  const text = $derived(
    level === 0
      ? [brain.family, brain.version].filter(Boolean).join(" ")
      : level === 1
        ? brain.family
        : brain.family.slice(0, 1),
  );
</script>

<span
  class="cap"
  style:--c={familyRgb(brain.family)}
  use:hint={`모델 ${brain.model}` + (brain.effort ? ` · effort ${brain.effort}` : "")}
>
  {text}
  {#if brain.effortLevel > 0}
    <span class="bars">
      {#each [1, 2, 3, 4, 5] as i (i)}
        <span class="bar" class:on={i <= brain.effortLevel} style:height="{3 + i * 1.3}px"></span>
      {/each}
    </span>
  {/if}
</span>

<style>
  .cap {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    flex: none;
    padding: 1.5px 6px;
    font-size: 10px;
    font-weight: 600;
    line-height: 13px;
    white-space: nowrap;
    color: rgb(var(--c));
    background: rgb(var(--c) / 0.12);
    box-shadow: inset 0 0 0 0.5px rgb(var(--c) / 0.22);
    border-radius: 999px;
  }
  .bars {
    display: inline-flex;
    align-items: flex-end;
    gap: 1.2px;
    height: 9.5px;
  }
  .bar {
    width: 2.2px;
    border-radius: 0.8px;
    background: rgb(var(--c) / 0.22);
  }
  .bar.on {
    background: rgb(var(--c));
  }
</style>
