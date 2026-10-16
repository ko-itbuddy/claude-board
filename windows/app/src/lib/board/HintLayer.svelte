<script lang="ts">
  import { hintState } from "./hint.svelte";

  let w = $state(0);
  let h = $state(0);

  // 시스템 툴팁처럼 커서 오른쪽 아래 — 아래 자리가 없으면 커서 위로, 가로는 창 안에 들게
  const left = $derived(Math.max(4, Math.min(hintState.x + 2, window.innerWidth - w - 4)));
  const top = $derived(hintState.y + 22 + h > window.innerHeight - 4 ? Math.max(4, hintState.y - 10 - h) : hintState.y + 22);
</script>

{#if hintState.visible}
  <div class="hint" style:left="{left}px" style:top="{top}px" bind:offsetWidth={w} bind:offsetHeight={h}>
    {hintState.text}
  </div>
{/if}

<style>
  .hint {
    position: fixed;
    z-index: 1000;
    max-width: 280px;
    padding: 5px 8px;
    font-size: 11px;
    font-weight: 500;
    line-height: 1.35;
    color: rgb(255 255 255 / 0.92);
    background: rgb(26 26 38 / 0.96);
    border: 0.5px solid rgb(255 255 255 / 0.14);
    border-radius: 6px;
    box-shadow: 0 4px 14px rgb(0 0 0 / 0.45);
    pointer-events: none;
    white-space: pre-line;
    word-break: keep-all;
    overflow-wrap: anywhere;
  }
</style>
