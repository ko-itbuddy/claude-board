<script lang="ts">
  // 보드 화면(SPEC §7, ClaudeBoard.swift:3772-4095·4246-4460) — 창 크기에 따라 전체·중간·미니 세 단계로 그린다
  import { tick, untrack } from "svelte";
  import { flip } from "svelte/animate";
  import { fade, fly, slide } from "svelte/transition";
  import { setBoardMini, toggleCollapsed } from "$lib/backend";
  import type { Group } from "$lib/types";
  import EdgeDock from "$lib/board/EdgeDock.svelte";
  import GroupCard from "$lib/board/GroupCard.svelte";
  import Header from "$lib/board/Header.svelte";
  import HintLayer from "$lib/board/HintLayer.svelte";
  import Icon from "$lib/board/Icon.svelte";
  import MenuLayer from "$lib/board/MenuLayer.svelte";
  import MiniBoard from "$lib/board/MiniBoard.svelte";
  import OpacitySlider from "$lib/board/OpacitySlider.svelte";
  import TitleBar from "$lib/board/TitleBar.svelte";
  import UpdateBanner from "$lib/board/UpdateBanner.svelte";
  import UsagePanel from "$lib/board/UsagePanel.svelte";
  import { stateName, stateRgb } from "$lib/board/format";
  import { hint } from "$lib/board/hint.svelte";
  import { pointer, trackWindow, win } from "$lib/board/pointer.svelte";
  import { board, connect, ui, type Tier } from "$lib/board/store.svelte";

  const snap = $derived(board.snap);

  $effect(() => connect());
  $effect(() => trackWindow(() => board.snap?.terminals.length ?? 5));

  // 투명 모드 — 보드가 활성이 아니고 마우스가 창 밖이면 고른 투명도, 마우스가 들어오거나 활성이면 불투명.
  // 슬라이더를 끄는 동안과 놓은 뒤 1.2초는 고른 투명도를 미리 보여 준다
  $effect(() => {
    if (!snap) return;
    const level = win.dragging ? win.dragLevel : snap.settings.opacity;
    const solid = !win.previewing && (pointer.inside || win.focused);
    const root = document.documentElement;
    root.style.transition = win.previewing ? "none" : "opacity 0.22s ease";
    root.style.opacity = String(solid ? 1 : level);
  });

  // ── 창 크기 단계(BoardFit) ──────────────────────────────────
  // 폭 360 미만 = 미니. 아니면 목록 칸이 120 이상 남으면 전체, 56 이상이면 중간, 그 밖 미니.
  // 단계마다 목록 칸을 뺀 높이는 그 단계로 그렸을 때만 잴 수 있어 잰 값을 기억한다(재기 전엔 어림값)
  const fixed: Record<"full" | "medium", number> = { full: 330, medium: 200 };
  let contentEl = $state<HTMLElement>();
  let listEl = $state<HTMLElement>();
  let scrollerEl = $state<HTMLElement>();
  const cardEls: Record<string, HTMLElement> = {};

  function pickTier() {
    const w = window.innerWidth;
    const h = window.innerHeight;
    const t: Tier = w < 360 ? "mini" : h >= fixed.full + 120 ? "full" : h >= fixed.medium + 56 ? "medium" : "mini";
    if (t === ui.tier) return;
    ui.tier = t;
    // 미니로 둔 채 끄면 다음에도 그 크기로 연다
    setBoardMini(t === "mini");
  }

  function measure() {
    const t = ui.tier;
    if (t === "mini" || !contentEl || !listEl) return;
    // 목록 칸은 남는 높이를 다 받으므로, 창 높이에서 목록 칸을 빼면 나머지(제목 막대 포함) 높이다
    const rest = 32 + contentEl.scrollHeight - listEl.offsetHeight;
    if (Math.abs(fixed[t] - rest) > 0.5) fixed[t] = rest;
    pickTier();
  }

  $effect(() => {
    window.addEventListener("resize", pickTier);
    untrack(pickTier);
    return () => window.removeEventListener("resize", pickTier);
  });

  $effect(() => {
    if (!contentEl || !listEl) return;
    const ro = new ResizeObserver(() => {
      measure();
      updateHidden();
    });
    ro.observe(contentEl);
    ro.observe(listEl);
    for (const c of contentEl.children) ro.observe(c);
    return () => ro.disconnect();
  });

  // ── 걸러 보기·목록 ──────────────────────────────────────────
  const shown = $derived.by<Group[]>(() => {
    if (!snap) return [];
    const f = ui.filter;
    if (!f) return snap.groups;
    return snap.groups
      .map((g) => ({ ...g, sessions: g.sessions.filter((s) => s.state === f) }))
      .filter((g) => g.sessions.length > 0);
  });

  // 걸러 보는 상태의 세션이 없어지면 모두 보기로
  $effect(() => {
    if (snap && ui.filter && snap.counts[ui.filter] === 0) ui.filter = null;
  });

  const medium = $derived(ui.tier === "medium");

  function toggleCard(id: string) {
    if (medium) {
      // 중간 화면에서 눌러 펼친 카드는 따로 기억한다 — 저장한 접힘 상태는 그대로
      ui.mediumOpen = ui.mediumOpen.includes(id) ? ui.mediumOpen.filter((x) => x !== id) : [...ui.mediumOpen, id];
    } else {
      toggleCollapsed(id);
    }
  }

  // 아래로 가려진 카드 — 카드 윗변이 목록 칸 아래 끝 24 위보다 아래면 가려진 것으로 본다
  let hiddenIds = $state<string[]>([]);
  function updateHidden() {
    if (!scrollerEl) {
      if (hiddenIds.length) hiddenIds = [];
      return;
    }
    const top = scrollerEl.getBoundingClientRect().top;
    const limit = scrollerEl.clientHeight - 24;
    const ids = shown.map((g) => g.id).filter((id) => cardEls[id] && cardEls[id].getBoundingClientRect().top - top > limit);
    if (ids.join("\n") !== hiddenIds.join("\n")) hiddenIds = ids;
  }
  $effect(() => {
    void shown;
    void ui.tier;
    tick().then(updateHidden);
  });
  $effect(() => {
    if (!scrollerEl) return;
    const cards = scrollerEl.firstElementChild;
    if (!cards) return;
    const ro = new ResizeObserver(updateHidden);
    ro.observe(cards);
    return () => ro.disconnect();
  });

  const below = $derived.by(() => {
    const hidden = shown.filter((g) => hiddenIds.includes(g.id));
    return { n: hidden.length, waiting: hidden.some((g) => g.sessions.some((s) => s.state === "waiting")), first: hidden[0]?.id };
  });

  function scrollToCard(id: string, smooth: boolean) {
    const el = cardEls[id];
    if (!el || !scrollerEl) return;
    scrollerEl.scrollTo({ top: el.offsetTop, behavior: smooth ? "smooth" : "auto" });
  }

  // 걸러 보기 전환이 그려진 뒤에 애니메이션 없이 옮긴다
  function scrollAfterFilter(id: string) {
    tick().then(() => scrollToCard(id, false));
  }

  function cardRef(node: HTMLElement, id: string) {
    cardEls[id] = node;
    return {
      destroy() {
        if (cardEls[id] === node) delete cardEls[id];
      },
    };
  }
</script>

<div class="board">
  {#if snap}
    <TitleBar mini={ui.tier === "mini"} counts={snap.counts} rows={snap.groups.length} />
    {#if ui.tier === "mini"}
      <MiniBoard {snap} />
    {:else}
      <div class="content" bind:this={contentEl}>
        <Header {snap} full={ui.tier === "full"} onScrollTo={scrollAfterFilter} />

        {#if snap.updater.latest != null || snap.updater.notice != null}
          <div transition:slide={{ duration: 220 }}><UpdateBanner updater={snap.updater} /></div>
        {/if}

        <UsagePanel {snap} forceCompact={medium} />

        <div class="list" bind:this={listEl}>
          {#if snap.groups.length === 0}
            <!-- 창이 낮으면 안내가 줄어들며 잘린다 — 줄지 않으면 보드가 넘쳐 아래 투명도 슬라이더가 밀려난다 -->
            <div class="empty" transition:fade={{ duration: 200 }}>
              <span class="moon"><Icon name="moon-zzz" size={30} /></span>
              <div class="t1">아직 기록된 세션이 없습니다</div>
              <div class="t2">Claude Code 세션에서 요청을 보내면 여기에 나타납니다</div>
            </div>
          {:else}
            {#if ui.filter}
              <div class="filter" style:--c={stateRgb(ui.filter)} transition:slide={{ duration: 180 }}>
                <span class="dot"></span>
                <span class="label">{stateName(ui.filter)}만 보는 중</span>
                <button class="all" onclick={() => (ui.filter = null)}>모두 보기</button>
              </div>
            {/if}
            <div class="scrollWrap">
              <div class="scroller" bind:this={scrollerEl} onscroll={updateHidden}>
                <div class="cards">
                  {#each shown as g (g.id)}
                    <div
                      class="cardSlot"
                      use:cardRef={g.id}
                      animate:flip={{ duration: 450 }}
                      in:fly|global={{ y: -16, duration: 380 }}
                      out:fade={{ duration: 200 }}
                    >
                      <GroupCard
                        {g}
                        now={snap.now}
                        collapsed={medium ? !ui.mediumOpen.includes(g.id) : snap.settings.collapsed.includes(g.id)}
                        toggle={() => toggleCard(g.id)}
                        settings={snap.settings}
                        locked={snap.allMuted}
                      />
                    </div>
                  {/each}
                </div>
              </div>
              {#if below.n > 0}
                <div class="below" transition:fade={{ duration: 200 }}>
                  <button
                    class="more"
                    class:waiting={below.waiting}
                    onclick={() => below.first && scrollToCard(below.first, true)}
                    use:hint={"눌러서 아래 카드로 이동"}
                  >
                    <Icon name="chevron-down" size={10} />
                    {below.waiting ? `아래에 경로 ${below.n}개 더 · 승인 대기 있음` : `아래에 경로 ${below.n}개 더`}
                  </button>
                </div>
              {/if}
            </div>
          {/if}
        </div>

        <!-- 창 오른쪽 아래에 투명도 슬라이더 -->
        <div class="opacity">
          <OpacitySlider value={snap.settings.opacity} />
        </div>
      </div>
      <!-- 터미널 열기 독 — 보드 위에 겹쳐 뜨므로 아래 배치는 그대로다 -->
      <EdgeDock terminals={snap.terminals} />
    {/if}
  {/if}
</div>

<HintLayer />
<MenuLayer />

<style>
  /* 창 바탕(Backdrop, ClaudeBoard.swift:2608-2622): 짙은 남색 그라데이션 위에 왼쪽 위에서 강조색 빛, 오른쪽 아래에서 푸른 빛.
     Tauri 창이 투명·테두리 없음이라 둥근 모서리와 바탕을 여기서 직접 그린다(뒤의 Mica 가 살짝 비치게 조금 투명) */
  .board {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    border-radius: 10px;
    background:
      radial-gradient(circle 360px at 0 0, rgb(217 120 87 / 0.22), transparent),
      radial-gradient(circle 420px at 100% 100%, rgb(115 128 255 / 0.14), transparent),
      linear-gradient(to bottom, rgb(31 31 48 / 0.94), rgb(18 18 28 / 0.94));
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.08);
  }
  .content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 20px 16px 0;
    overflow: hidden;
  }
  .content > :global(*) {
    flex: none;
  }
  .content > .list {
    flex: 1 1 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .empty {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    text-align: center;
  }
  .moon {
    color: var(--fg2);
  }
  .t1 {
    font-size: 14px;
    font-weight: 500;
  }
  .t2 {
    font-size: 12px;
    color: var(--fg2);
  }
  .filter {
    flex: none;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 10px;
    border-radius: 8px;
    background: rgb(var(--c) / 0.12);
  }
  .filter .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgb(var(--c));
  }
  .filter .label {
    flex: 1;
    font-size: 12px;
    font-weight: 600;
  }
  .filter .all {
    font-size: 12px;
    font-weight: 600;
    color: rgb(var(--accent));
  }
  .scrollWrap {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  /* 스크롤 막대는 오른쪽 여백 안에 두어 막대가 생겨도 카드 폭이 변하지 않게 한다 */
  .scroller {
    position: relative;
    height: 100%;
    margin: 0 -16px;
    padding: 0 8px 0 16px;
    overflow-y: auto;
    scrollbar-gutter: stable;
  }
  .cards {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 1px 0 12px;
  }
  .below {
    position: absolute;
    left: -16px;
    right: -16px;
    bottom: 0;
    height: 56px;
    display: flex;
    align-items: flex-end;
    justify-content: center;
    padding-bottom: 8px;
    background: linear-gradient(to bottom, transparent, rgb(18 18 28 / 0.9));
    pointer-events: none;
  }
  .more {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    color: rgb(255 255 255 / 0.9);
    background: rgb(255 255 255 / 0.14);
    box-shadow: inset 0 0 0 0.5px rgb(255 255 255 / 0.35);
    pointer-events: auto;
    transition: background 0.15s;
  }
  .more:hover {
    background: rgb(255 255 255 / 0.22);
  }
  .more.waiting {
    color: rgb(0 0 0 / 0.85);
    background: rgb(var(--waiting) / 0.9);
    box-shadow: inset 0 0 0 0.5px rgb(var(--waiting) / 0.35);
  }
  .more.waiting:hover {
    background: rgb(var(--waiting));
  }
  .opacity {
    display: flex;
    justify-content: flex-end;
    margin-top: -8px;
    padding-bottom: 6px;
  }
</style>
