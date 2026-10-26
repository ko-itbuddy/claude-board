<script lang="ts">
  // 설정 항목 한 줄을 담는 카드(제목·설명·오른쪽 조작) — TODO(P2.1): 모양. 묶음 화면들이 이 카드로 줄을 그린다
  import type { Snippet } from "svelte";

  let { title, detail, children }: { title: string; detail?: string; children?: Snippet } = $props();
</script>

<div class="card" data-testid="settings-card">
  <div class="text">
    <div class="title">{title}</div>
    {#if detail}<div class="detail">{detail}</div>{/if}
  </div>
  {#if children}<div class="control">{@render children()}</div>{/if}
</div>
