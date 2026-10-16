<svelte:options namespace="svg" />

<script lang="ts">
  // 운동화 — 흰 신발에 빨간 곡선, 앞코가 왼쪽. (x, y) 는 SwiftUI 배치 자리이고 그림은 1.5 왼쪽으로 비켜 있다
  import { rect, rgb } from "../draw";

  let { x, y }: { x: number; y: number } = $props();
  const ox = $derived(x - 1.5 - 3.5);
  const oy = $derived(y - 1.75);
</script>

<rect {...rect(x - 1.5, y, 7, 3.5, 1.5)} fill="#fff" />
<path d="M{ox + 1.5},{oy + 1.4}Q{ox + 3},{oy + 3.3} {ox + 6},{oy + 0.8}" fill="none" stroke={rgb(0.95, 0.3, 0.25)} stroke-width="0.8" />
