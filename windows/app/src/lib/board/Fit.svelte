<script lang="ts">
  // SwiftUI ViewThatFits 대응 — 단계 0(가장 자세함)부터 차례로 재어 칸 폭에 다 들어가는 첫 단계를 그린다.
  // 단계마다 보이지 않는 측정용 사본을 두고 그 고유 폭을 칸 폭과 견준다 — 그린 뒤 넘치면 줄이는 방식은 깜빡이며 오간다
  import type { Snippet } from "svelte";

  // 그 밖의 속성(data-tauri-drag-region 등)은 바깥 칸에 그대로 붙인다
  let {
    count,
    content,
    class: cls = "",
    ...rest
  }: { count: number; content: Snippet<[number]>; class?: string; [attr: string]: unknown } = $props();

  let box: HTMLElement;
  const probes: HTMLElement[] = $state([]);
  let level = $state(0);

  function pick() {
    if (!box) return;
    const w = box.clientWidth;
    let l = count - 1;
    for (let i = 0; i < count; i++) {
      if (probes[i] && probes[i].offsetWidth <= w + 0.5) {
        l = i;
        break;
      }
    }
    if (l !== level) level = l;
  }

  $effect(() => {
    const ro = new ResizeObserver(pick);
    ro.observe(box);
    for (const p of probes) if (p) ro.observe(p);
    pick();
    return () => ro.disconnect();
  });
</script>

<div class="fit {cls}" bind:this={box} {...rest}>
  {@render content(level)}
  <div class="probes" aria-hidden="true" inert>
    {#each { length: count } as _, i (i)}
      <div class="probe" bind:this={probes[i]}>{@render content(i)}</div>
    {/each}
  </div>
</div>

<style>
  .fit {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 0;
    overflow-x: clip;
  }
  .probes {
    position: absolute;
    left: 0;
    top: 0;
    width: 0;
    height: 0;
    overflow: hidden;
    visibility: hidden;
    pointer-events: none;
  }
  .probe {
    display: flex;
    align-items: center;
    width: max-content;
    white-space: nowrap;
  }
</style>
