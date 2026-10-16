<script lang="ts">
  // 꼬리가 오른쪽(캐릭터 쪽)으로 난 말풍선 (ClaudeBoard.swift SpeechBubble) — 글자 크기에 맞춰 모양을 다시 그린다
  let { text, tint, compact = false }: { text: string; tint: string; compact?: boolean } = $props();

  let size = $state<ResizeObserverSize[]>();

  const shape = $derived.by(() => {
    const w = size?.[0]?.inlineSize ?? 0;
    const h = size?.[0]?.blockSize ?? 0;
    if (w === 0 || h === 0) return "";
    const tail = 7;
    const bw = w - tail;
    const r = Math.min(10, h / 2);
    const y = h / 2 + 2;
    return (
      `M${r},0H${bw - r}A${r},${r} 0 0 1 ${bw},${r}V${h - r}A${r},${r} 0 0 1 ${bw - r},${h}H${r}A${r},${r} 0 0 1 0,${h - r}V${r}A${r},${r} 0 0 1 ${r},0Z` +
      `M${bw - 1},${y - 5}Q${bw + 2},${y} ${w},${y + 3}Q${bw + 1},${y + 4} ${bw - 1},${y + 4}Z`
    );
  });
</script>

<span class="bubble" bind:borderBoxSize={size}>
  <svg class="shape" aria-hidden="true">
    <path d={shape} fill={tint} />
    <path d={shape} fill="none" stroke="rgba(255,255,255,0.18)" stroke-width="0.6" />
  </svg>
  <span class="text" class:one={compact}>{text}</span>
</span>

<style>
  .bubble {
    position: relative;
    display: block;
    box-sizing: border-box;
    max-width: 100%;
    /* 오른쪽 16 중 7 은 꼬리 자리 */
    padding: 5px 16px 5px 9px;
  }
  .shape {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }
  .text {
    position: relative;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    /* macOS 11pt 굵은 글자의 줄 높이 — 한 줄 말풍선 높이 24 */
    line-height: 14px;
    text-align: left;
    /* 한국어도 낱말 단위로 줄을 바꾼다(macOS 와 같게) */
    word-break: keep-all;
    overflow-wrap: anywhere;
  }
  /* 미니 화면 — 한 줄, 넘치면 … */
  .text.one {
    display: block;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
