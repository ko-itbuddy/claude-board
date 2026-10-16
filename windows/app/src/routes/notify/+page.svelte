<script lang="ts">
  // 작업 완료·승인 대기 알림 패널(SPEC §4·§6.3, ClaudeNotifier.swift:277-447).
  // 1줄 = 색 배지 · "<프로젝트> <상태>" · 시각, 2줄 = 본문(최대 2줄), 3줄 = 상태별 개수, 맨 아래 2px 남은 시간 막대.
  // 마우스를 올려 두는 동안 카운트다운을 멈추고, 누르면 보드를 띄우고 닫힌다
  import { tick } from "svelte";
  import { getNotifyOptions, notifyClicked, notifyFinish, notifyHover, notifyReady } from "$lib/backend";
  import Icon from "$lib/board/Icon.svelte";
  import { clock } from "$lib/board/format";
  import type { NotifyOptions } from "$lib/types";

  let opts = $state<NotifyOptions | null>(null);
  let card = $state<HTMLElement>();
  let phase = $state<"before" | "shown" | "closing">("before");
  let left = $state(1); // 남은 시간 비율
  const time = clock(Date.now());

  let elapsed = 0;
  let hovered = false;
  let timer: ReturnType<typeof setInterval> | undefined;

  const accent = $derived(opts?.kind === "wait" ? "255 168 64" : "77 217 140");
  const parts = $derived(
    opts
      ? (
          [
            ["waiting", "승인 대기", "255 168 92"],
            ["working", "작업 중", "115 168 255"],
            ["idle", "대기", "107 217 153"],
          ] as const
        )
          .map(([k, name, rgb]) => ({ name, rgb, n: opts!.summary[k] ?? 0 }))
          .filter((p) => p.n > 0)
      : [],
  );

  $effect(() => {
    let alive = true;
    (async () => {
      opts = await getNotifyOptions();
      await tick();
      await document.fonts.ready;
      if (!alive || !card) return;
      // 내용에 맞춘 높이를 알리면 백엔드가 그 높이로 자리를 잡고 창을 보인다
      await notifyReady(Math.ceil(card.getBoundingClientRect().height));
      requestAnimationFrame(() => (phase = "shown"));
      timer = setInterval(countdown, 50);
    })();
    return () => {
      alive = false;
      clearInterval(timer);
    };
  });

  function countdown() {
    if (!opts || phase === "closing") return;
    if (!hovered) elapsed += 0.05;
    const remaining = Math.max(0, opts.duration - elapsed);
    left = opts.duration > 0 ? remaining / opts.duration : 0;
    if (remaining <= 0) dismiss();
  }

  function dismiss() {
    if (phase === "closing") return;
    phase = "closing";
    clearInterval(timer);
    // 0.25초 동안 흐려지며 10px 올라간 뒤 끝낸다
    setTimeout(notifyFinish, 300);
  }

  function hover(inside: boolean) {
    hovered = inside;
    notifyHover(inside);
  }
</script>

{#if opts}
  <div
    class="card"
    class:shown={phase === "shown"}
    class:closing={phase === "closing"}
    style:--a={accent}
    bind:this={card}
    role="button"
    tabindex="-1"
    onmouseenter={() => hover(true)}
    onmouseleave={() => hover(false)}
    onclick={() => {
      notifyClicked();
      dismiss();
    }}
    onkeydown={() => {}}
  >
    <div class="column">
      <div class="header">
        <span class="badge"><Icon name={opts.kind === "wait" ? "hand" : "seal-check"} size={opts.kind === "wait" ? 19 : 17} /></span>
        <span class="title">
          {#if opts.subtitle}<b>{opts.subtitle}</b>{/if}<span class="kind">{opts.title}</span>
        </span>
        <span class="clock num">{time}</span>
      </div>
      {#if opts.body || parts.length}
        <div class="text">
          {#if opts.body}<div class="body">{opts.body}</div>{/if}
          {#if parts.length}
            <div class="summary">
              {#each parts as p (p.name)}
                <span class="part num"><span class="dot" style:background="rgb({p.rgb})"></span>{p.name} {p.n}</span>
              {/each}
            </div>
          {/if}
        </div>
      {/if}
      <div class="track"><div class="fill" style:width="{left * 100}%"></div></div>
    </div>
  </div>
{/if}

<style>
  /* 창은 투명 — 카드가 바탕(보드와 같은 남색 그라데이션 + 왼쪽 위 강조색 빛 + 강조색 35% 테두리)을 직접 그린다 */
  .card {
    position: relative;
    width: 440px;
    border-radius: 14px;
    overflow: hidden;
    cursor: pointer;
    background:
      radial-gradient(ellipse 67% 140% at 8% 0%, rgb(var(--a) / 0.28), rgb(var(--a) / 0)),
      linear-gradient(to bottom, rgb(31 31 48 / 0.94), rgb(18 18 28 / 0.94));
    box-shadow: inset 0 0 0 1px rgb(var(--a) / 0.35);
    opacity: 0;
    transform: translateY(14px);
  }
  /* 14px 아래에서 0.28초 동안 올라오며 나타난다 */
  .card.shown {
    opacity: 1;
    transform: none;
    transition:
      opacity 0.28s ease-out,
      transform 0.28s ease-out;
  }
  /* 시간이 다 되거나 누르면 0.25초 동안 흐려지며 10px 올라간다 */
  .card.closing {
    opacity: 0;
    transform: translateY(-10px);
    transition:
      opacity 0.25s ease-in-out,
      transform 0.25s ease-in-out;
  }
  :global(html.browser) .card {
    margin: 32px;
  }
  .column {
    display: flex;
    flex-direction: column;
    padding: 14px 16px 12px;
  }
  .header {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .badge {
    flex: none;
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 9px;
    color: rgb(var(--a));
    background: rgb(var(--a) / 0.18);
  }
  .title {
    flex: 1;
    min-width: 0;
    font-size: 15px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .title b {
    font-weight: 700;
    color: #fff;
    margin-right: 8px;
  }
  .kind {
    font-weight: 600;
    color: rgb(var(--a));
  }
  .clock {
    flex: none;
    font-size: 12px;
    font-weight: 500;
    color: rgb(255 255 255 / 0.45);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 7px;
    margin: 6px 0 0 40px;
  }
  .body {
    font-size: 13px;
    line-height: 1.4;
    color: rgb(255 255 255 / 0.82);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }
  .summary {
    display: flex;
    gap: 12px;
  }
  .part {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 11.5px;
    font-weight: 500;
    color: rgb(255 255 255 / 0.6);
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 3px;
  }
  .track {
    position: relative;
    height: 2px;
    margin-top: 12px;
    border-radius: 1px;
    background: rgb(255 255 255 / 0.08);
  }
  .header + .track {
    margin-top: 6px;
  }
  .fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    border-radius: 1px;
    background: rgb(var(--a) / 0.85);
    transition: width 50ms linear;
  }
</style>
