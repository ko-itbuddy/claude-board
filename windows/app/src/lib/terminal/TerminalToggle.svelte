<script lang="ts">
  // 제목 막대의 "터미널" 단추(ClaudeBoard.swift:7369-7398 TerminalToggleButton) — 캡슐. 펼침은 작업 중 색, 접힘은 흰 반투명.
  // 미니 단계에서는 부르는 쪽이 그리지 않는다
  import { setTerminalExpanded } from "$lib/backend";
  import Icon from "$lib/board/Icon.svelte";
  import { hint } from "$lib/board/hint.svelte";
  import type { BoardSide } from "$lib/types";

  let { expanded, side }: { expanded: boolean; side: BoardSide } = $props();

  const left = $derived(side === "left");
  const tip = $derived(
    expanded
      ? `터미널 접기: 보드만 ${left ? "왼쪽" : "오른쪽"}에 남깁니다 (Ctrl+Alt+T)`
      : `터미널 펼치기: 보드에서 띄운 세션을 ${left ? "오른쪽" : "왼쪽"} 탭에서 봅니다 (Ctrl+Alt+T)`,
  );
</script>

<button class="toggle" class:on={expanded} onclick={() => setTerminalExpanded(!expanded)} use:hint={tip} aria-pressed={expanded}>
  <span class="cap">
    <Icon name={left ? "panel-right" : "panel-left"} size={12} />
    <span class="text">{expanded ? "터미널 펼침" : "터미널 접힘"}</span>
    <span class="key">Ctrl+Alt+T</span>
  </span>
</button>

<style>
  /* 눌리는 칸은 높이 28, 캡슐은 그 안 20 */
  .toggle {
    flex: none;
    height: 28px;
    display: flex;
    align-items: center;
  }
  .cap {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 20px;
    padding: 0 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    color: rgb(255 255 255 / 0.8);
    background: rgb(255 255 255 / 0.18);
    box-shadow: inset 0 0 0 0.5px rgb(255 255 255 / 0.18);
    transition:
      color 0.15s ease-out,
      background 0.15s ease-out;
  }
  .toggle:hover .cap {
    color: #fff;
    background: rgb(255 255 255 / 0.3);
  }
  .toggle.on .cap {
    color: rgb(var(--working));
    background: rgb(var(--working) / 0.18);
    box-shadow: inset 0 0 0 0.5px rgb(var(--working) / 0.45);
  }
  .key {
    padding: 0 4px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 500;
    background: rgb(255 255 255 / 0.08);
  }
</style>
