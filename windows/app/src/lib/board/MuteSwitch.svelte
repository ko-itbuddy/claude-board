<script lang="ts">
  // 세션별 알림 켜기·끄기 — 끄면 작업 완료·승인 대기 패널과 소리를 모두 띄우지 않는다.
  // 전체 알림이 꺼져 있으면(locked) 회색 점선으로 잠그고, 손잡이는 세션 설정 위치 그대로 둔다(다시 켜면 돌아갈 상태)
  import type { Session } from "$lib/types";
  import { setMute } from "$lib/backend";
  import Icon from "./Icon.svelte";
  import { hint } from "./hint.svelte";

  let { s, locked }: { s: Session; locked: boolean } = $props();

  const on = $derived(!s.muted);
  const text = $derived(
    locked
      ? `전체 알림이 꺼져 있어 바꿀 수 없습니다 — 윗줄 종 단추로 전체 알림을 켜면 이 세션의 설정(${on ? "켜짐" : "꺼짐"})대로 돌아갑니다`
      : !on
        ? "알림 꺼짐 — 켜면 작업 완료·승인 대기 알림을 다시 띄웁니다"
        : "알림 켜짐 — 끄면 이 세션의 작업 완료·승인 대기 알림을 띄우지 않습니다",
  );
</script>

<button
  class="switch"
  class:on
  class:locked
  aria-disabled={locked}
  role="switch"
  aria-checked={on}
  aria-label="알림"
  onclick={(e) => {
    e.stopPropagation();
    if (!locked) setMute(s.id, on);
  }}
  ondblclick={(e) => e.stopPropagation()}
  use:hint={text}
>
  <span class="knob">
    <Icon name={locked || !on ? "bell-slash" : "bell"} size={9} />
  </span>
</button>

<style>
  .switch {
    position: relative;
    flex: none;
    width: 36px;
    height: 20px;
    border-radius: 10px;
    background: rgb(255 255 255 / 0.14);
    box-shadow: inset 0 0 0 0.5px rgb(255 255 255 / 0.18);
    transition: background 0.2s;
  }
  .switch:hover:not(.on):not(.locked) {
    background: rgb(255 255 255 / 0.22);
  }
  .switch.on {
    background: rgb(var(--working));
    box-shadow: none;
  }
  .switch.locked {
    cursor: default;
    background: rgb(255 255 255 / 0.12);
    box-shadow: none;
    outline: 0.5px dashed rgb(255 255 255 / 0.35);
    outline-offset: -0.5px;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: rgb(255 255 255 / 0.75);
    color: rgb(0 0 0 / 0.55);
    --icon-cut: rgb(191 191 191);
    box-shadow: 0 0.5px 1px rgb(0 0 0 / 0.3);
    transition: transform 0.22s cubic-bezier(0.3, 1.3, 0.5, 1), background 0.2s;
  }
  .on .knob {
    transform: translateX(16px);
    background: #fff;
    color: rgb(var(--working));
    --icon-cut: #fff;
  }
  .locked .knob {
    background: rgb(255 255 255 / 0.45);
    color: rgb(0 0 0 / 0.6);
    box-shadow: none;
    --icon-cut: rgb(120 120 128);
  }
</style>
