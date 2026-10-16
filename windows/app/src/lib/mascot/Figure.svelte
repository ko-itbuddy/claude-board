<script lang="ts">
  // 캐릭터 그림과 동작 — 모두 시계 t(초)의 함수라 반복 애니메이션 없이 공유 시계 하나로 움직인다 (ClaudeBoard.swift BrainrotFigure·Foreman.figure).
  // 말풍선이 바뀌면 폴짝 뛰고, 기분에 따라 박자가 빨라지거나 느려진다. 틀은 46×44, compact 는 0.75배(35×33)
  import type { MascotKind, Mood } from "$lib/types";
  import { YELLOW, cycle, swing } from "./draw";
  import Bombardiro from "./figures/Bombardiro.svelte";
  import Boneca from "./figures/Boneca.svelte";
  import Cappuccino from "./figures/Cappuccino.svelte";
  import ForemanBody from "./figures/ForemanBody.svelte";
  import Lirili from "./figures/Lirili.svelte";
  import Patapim from "./figures/Patapim.svelte";
  import Tralalero from "./figures/Tralalero.svelte";
  import Tungtung from "./figures/Tungtung.svelte";

  let {
    kind,
    mood,
    t,
    sinceChange,
    blink,
    compact = false,
  }: { kind: MascotKind; mood: Mood; t: number; sinceChange: number; blink: boolean; compact?: boolean } = $props();

  const BODIES = { tralalero: Tralalero, bombardiro: Bombardiro, tungtung: Tungtung, lirili: Lirili, patapim: Patapim, boneca: Boneca, cappuccino: Cappuccino };
  const sin = Math.sin;
  const PI = Math.PI;

  const motion = $derived.by(() => {
    // 말풍선이 바뀌면 폴짝 — 0.45초 동안 뛰었다 내려와 착지에 찌그러진다
    const hopP = Math.min(Math.max(sinceChange / 0.45, 0), 1);
    let hop = sinceChange < 0.45 ? -9 * sin(PI * hopP) : 0;
    let land = sinceChange >= 0.45 && sinceChange < 0.62 ? sin((PI * (sinceChange - 0.45)) / 0.17) : 0;
    let tilt = 0, dx = 0, breathe = 0, armDeg = 0;
    if (kind === "foreman") {
      switch (mood) {
        case "urgent": {
          // 제자리에서 연달아 뛰며 지휘봉을 머리 위로 흔든다
          const p = cycle(t, 0.55);
          hop = Math.min(hop, -7 * sin(PI * p));
          land = Math.max(land, p > 0.85 ? sin((PI * (p - 0.85)) / 0.15) : 0);
          armDeg = 70 + 35 * sin((2 * PI * t) / 0.3);
          dx = 1.5 * sin((2 * PI * t) / 0.12);
          break;
        }
        case "busy":
          // 지휘 — 지휘봉이 8자를 그리고 몸이 박자에 맞춰 기운다
          armDeg = 18 + 30 * sin((2 * PI * t) / 0.9) + 10 * sin((4 * PI * t) / 0.9);
          tilt = 7 * sin((2 * PI * t) / 0.9);
          hop = Math.min(hop, -2.5 * Math.abs(sin((2 * PI * t) / 0.9)));
          break;
        case "idle": {
          // 말풍선 쪽을 두 번 콕콕 찌르고 쉰다
          const p = cycle(t, 2.4);
          const jab = p < 0.12 ? sin((PI * p) / 0.12) : p > 0.18 && p < 0.3 ? sin((PI * (p - 0.18)) / 0.12) : 0;
          armDeg = 6 + 8 * jab;
          dx = -2.5 * jab;
          tilt = 5 * sin((2 * PI * t) / 2.4);
          breathe = swing(t, 1.6);
          break;
        }
        case "sleepy":
          armDeg = -25;
          tilt = 9 * sin((2 * PI * t) / 3.2);
          breathe = swing(t, 3.2);
          break;
      }
    } else {
      switch (mood) {
        case "urgent": {
          const p = cycle(t, 0.55);
          hop = Math.min(hop, -6 * sin(PI * p));
          land = Math.max(land, p > 0.85 ? sin((PI * (p - 0.85)) / 0.15) : 0);
          dx = 1.5 * sin((2 * PI * t) / 0.12);
          break;
        }
        case "busy":
          tilt = 5 * sin((2 * PI * t) / 0.9);
          hop = Math.min(hop, -2 * Math.abs(sin((2 * PI * t) / 0.9)));
          break;
        case "idle":
          tilt = 4 * sin((2 * PI * t) / 2.4);
          breathe = swing(t, 1.6);
          break;
        case "sleepy":
          tilt = 8 * sin((2 * PI * t) / 3.2);
          breathe = swing(t, 3.2);
          break;
      }
    }
    const squash = land * 0.16;
    // SwiftUI 순서 그대로: 아래 가운데 기준 찌그러짐 → 아래 가운데 기준 기울기 → 이동
    const transform =
      `translate(${dx} ${hop + 2}) rotate(${tilt} 0 22) ` +
      `translate(0 22) scale(${1 + squash + 0.03 * breathe} ${1 - squash + 0.03 * breathe}) translate(0 -22)`;
    return { transform, land, armDeg };
  });

  const Body = $derived(kind === "foreman" ? null : BODIES[kind]);
</script>

<!-- 그림은 틀 밖(폴짝·"!"·지휘봉)으로 나가므로 자르지 않는다 -->
<svg class="figure" width={compact ? 35 : 46} height={compact ? 33 : 44} viewBox="0 0 46 44" overflow="visible" aria-hidden="true">
  <g transform="translate(23 22)">
    <g transform={motion.transform}>
      {#if Body}
        <Body {mood} {t} {blink} />
      {:else}
        <ForemanBody {mood} {t} {blink} armDeg={motion.armDeg} land={motion.land} />
      {/if}
      <!-- 머리 위 표시: 재촉 "!" · 졸음 "z" -->
      {#if mood === "urgent"}
        {@const s = 1 + 0.25 * swing(t, 0.4)}
        <text x="14" y="-19" font-size="13" font-weight="900" fill={YELLOW} text-anchor="middle" dominant-baseline="central"
          transform="translate(14 -19) scale({s}) translate(-14 19)">!</text>
      {:else if mood === "sleepy"}
        {#each [0, 1] as i}
          {@const p = cycle(t + i * 1.2, 2.4)}
          <text x={12 + 6 * p} y={-10 - 14 * p} font-size={8 + 3 * p} font-weight="700" fill="#fff" opacity={1 - p}
            text-anchor="middle" dominant-baseline="central">z</text>
        {/each}
      {/if}
    </g>
  </g>
</svg>

<style>
  .figure {
    display: block;
    flex: none;
    overflow: visible;
  }
</style>
