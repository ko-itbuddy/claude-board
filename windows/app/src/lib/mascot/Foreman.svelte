<script lang="ts">
  // 제목 오른쪽의 작은 캐릭터 — 세션 상태에 따라 움직이고, 재촉하고, 졸고, 말풍선으로 상황을 알려 준다. 누르면 다음 말.
  // 그림은 고른 캐릭터(브레인롯 7종 또는 현장 감독)로 그린다 — 오른쪽 클릭으로 고른다 (ClaudeBoard.swift Foreman)
  import { hint } from "$lib/board/hint.svelte";
  import type { Group, Limit, MascotKind, Mood } from "$lib/types";
  import Bubble from "./Bubble.svelte";
  import { clock } from "./clock.svelte";
  import { rgb } from "./draw";
  import Figure from "./Figure.svelte";
  import { MASCOTS } from "./mascots";
  import { foremanQuips, type Quip } from "./quips";

  let {
    groups,
    limits,
    now,
    compact = false, // 미니 화면 — 말풍선은 한 줄, 캐릭터는 조금 작게
    mascot,
    onPickMascot,
  }: {
    groups: Group[];
    limits: Limit[];
    now: number;
    compact?: boolean;
    mascot: MascotKind;
    onPickMascot: (kind: MascotKind) => void;
  } = $props();

  // 말풍선 색 — 재촉 = 승인 대기 색, 작업 = 작업 중 색, 대기 = 강조색, 졸음 = 회색
  const TINT: Record<Mood, string> = {
    urgent: rgb(1, 0.66, 0.36, 0.92),
    busy: rgb(0.45, 0.66, 1, 0.92),
    idle: rgb(0.85, 0.47, 0.34, 0.92),
    sleepy: rgb(0.45, 0.45, 0.45, 0.92),
  };

  const info = $derived(MASCOTS.find((m) => m.kind === mascot) ?? MASCOTS[0]);

  // 승인 대기가 없으면 돌아가는 말 끝에 캐릭터 유행어를 한 번 끼운다
  const quips = $derived.by((): Quip[] => {
    const qs = foremanQuips(groups, limits, now);
    const first = qs[0];
    if (!info.catchphrase || !first || first.mood === "urgent") return qs;
    return [...qs, { text: info.catchphrase, mood: first.mood }];
  });

  // 말풍선 교체·눈 깜빡임 — 보드가 1초마다 새 묶음을 줘도 타이머가 새로 만들어지지 않게 여기 둔다
  let line = $state(0);
  let blink = $state(false);
  // 말풍선이 바뀐 시각 — 캐릭터가 그 순간 폴짝 뛴다(처음 나타날 때도)
  let changedAt = $state(clock.peek());
  const q = $derived(quips[line % Math.max(quips.length, 1)]);

  function next() {
    changedAt = clock.peek();
    line += 1;
  }

  $effect(() => {
    const lines = setInterval(next, 3200);
    let open: ReturnType<typeof setTimeout> | undefined;
    const blinks = setInterval(() => {
      blink = true;
      open = setTimeout(() => (blink = false), 140);
    }, 3700);
    return () => {
      clearInterval(lines);
      clearInterval(blinks);
      clearTimeout(open);
    };
  });

  // SwiftUI .spring(response: 0.38, dampingFraction: 0.55) — 0.6초면 거의 멈춘다
  const SPRING = 0.6;
  function spring(u: number) {
    const s = u * SPRING;
    const w = (2 * Math.PI) / 0.38;
    const z = 0.55;
    const wd = w * Math.sqrt(1 - z * z);
    return 1 - Math.exp(-z * w * s) * (Math.cos(wd * s) + ((z * w) / wd) * Math.sin(wd * s));
  }
  // 말풍선 갈아 끼우기 — 새 말은 0.4배에서 커지고, 지난 말은 0.8배로 줄며 흐려진다. 오른쪽(캐릭터 쪽)이 기준
  function pop(_: Element, { from }: { from: number }) {
    return {
      duration: SPRING * 1000,
      easing: spring,
      css: (t: number) =>
        `transform-origin: right center; transform: scale(${from + (1 - from) * t}); opacity: ${Math.min(Math.max(t, 0), 1)}`,
    };
  }

  // 오른쪽 클릭 캐릭터 메뉴 — popover 라 보드의 다른 층에 가리거나 잘리지 않는다
  let menu = $state<HTMLElement>();
  function openMenu(e: MouseEvent) {
    e.preventDefault();
    if (!menu) return;
    menu.showPopover();
    const x = Math.min(e.clientX, innerWidth - menu.offsetWidth - 4);
    const y = Math.min(e.clientY, innerHeight - menu.offsetHeight - 4);
    menu.style.left = `${Math.max(4, x)}px`;
    menu.style.top = `${Math.max(4, y)}px`;
  }
  function pick(kind: MascotKind) {
    menu?.hidePopover();
    onPickMascot(kind);
  }
</script>

<button
  type="button"
  class="foreman"
  tabindex="-1"
  use:hint={`${info.name} — 누르면 다음 말 · 오른쪽 클릭으로 캐릭터 바꾸기`}
  onclick={next}
  oncontextmenu={openMenu}
>
  <span class="bubbles" style:max-width="{compact ? 230 : 170}px">
    {#key q.text}
      <span class="slot" in:pop={{ from: 0.4 }} out:pop={{ from: 0.8 }}>
        <Bubble text={q.text} tint={TINT[q.mood]} {compact} />
      </span>
    {/key}
  </span>
  <Figure kind={mascot} mood={q.mood} t={clock.t} sinceChange={clock.t - changedAt} {blink} {compact} />
</button>

<div class="menu" popover="auto" bind:this={menu}>
  {#each MASCOTS as m (m.kind)}
    <button type="button" class="item" onclick={() => pick(m.kind)}>
      <span class="check">{m.kind === mascot ? "✓" : ""}</span>{m.name}
    </button>
  {/each}
</div>

<style>
  .foreman {
    display: flex;
    align-items: center;
    gap: 2px;
    min-width: 0;
    margin: 0;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: default;
    outline: none;
  }
  .bubbles {
    display: grid;
    justify-items: end;
    align-items: center;
    min-width: 0;
  }
  /* 바뀌는 동안 지난 말과 새 말이 한자리에 겹친다 — 캐릭터가 밀리지 않게 */
  .slot {
    grid-area: 1 / 1;
    display: block;
    max-width: 100%;
  }
  .menu {
    position: fixed;
    inset: auto;
    margin: 0;
    max-height: calc(100vh - 8px);
    overflow-y: auto;
    padding: 4px;
    border: 0.5px solid rgba(255, 255, 255, 0.14);
    border-radius: 7px;
    background: rgb(40, 40, 50);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45);
    color: rgba(255, 255, 255, 0.92);
    font-size: 12px;
  }
  .item {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 1px 12px 1px 4px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: inherit;
    font: inherit;
    line-height: 17px;
    text-align: left;
    white-space: nowrap;
    cursor: default;
  }
  .item:hover {
    background: rgba(255, 255, 255, 0.12);
  }
  .check {
    width: 16px;
    flex: none;
    text-align: center;
  }
</style>
