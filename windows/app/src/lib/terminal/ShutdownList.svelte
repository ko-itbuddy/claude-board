<script lang="ts">
  // 끌 때 확인 창(mac ShutdownList) — 도는 세션 목록을 보이고 끌지 묻는다. 보드 창 안의 모달이다(D3).
  // TODO(P1.1): 지금은 자리만 있다 — Lifecycle.confirm 이 켜져도 아무것도 그리지 않는다. 목록·단추·answerShutdown 은 P1.1 이 채운다.
  import { Lifecycle } from "./lifecycle.svelte";
</script>

{#if Lifecycle.confirm}
  <!-- TODO(P1.1) -->
{/if}
