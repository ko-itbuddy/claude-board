<script lang="ts">
  // 설정 화면 — 터미널 영역을 덮는다(mac SettingsScreen 의 화면). 떠 있지 않으면(visible=false, 기본) 아무것도 그리지 않는다.
  // 어느 묶음을 그릴지만 여기서 고르고, 줄은 묶음 화면(groups/*)이 그린다.
  import { SettingsScreen, connectSettings } from "./screen.svelte";
  import SettingsSidebar from "./SettingsSidebar.svelte";
  import Account from "./groups/Account.svelte";
  import Board from "./groups/Board.svelte";
  import General from "./groups/General.svelte";
  import Notify from "./groups/Notify.svelte";
  import Shortcuts from "./groups/Shortcuts.svelte";
  import Terminal from "./groups/Terminal.svelte";
  import Update from "./groups/Update.svelte";
  import Window from "./groups/Window.svelte";

  $effect(() => connectSettings());
</script>

{#if SettingsScreen.visible}
  <div class="settings" data-testid="settings-view">
    <SettingsSidebar />
    <div class="content">
      {#if SettingsScreen.section === "general"}
        <General />
      {:else if SettingsScreen.section === "window"}
        <Window />
      {:else if SettingsScreen.section === "notify"}
        <Notify />
      {:else if SettingsScreen.section === "terminal"}
        <Terminal />
      {:else if SettingsScreen.section === "board"}
        <Board />
      {:else if SettingsScreen.section === "account"}
        <Account />
      {:else if SettingsScreen.section === "shortcuts"}
        <Shortcuts />
      {:else}
        <Update />
      {/if}
    </div>
  </div>
{/if}

<style>
  /* 터미널 칸 위를 덮는다 — 제목 띠(32px) 아래부터. TODO(P2.1): 바탕·전환 효과 */
  .settings {
    position: absolute;
    inset: 32px 0 0;
    z-index: 20;
    display: flex;
    background: var(--term-bar);
  }
  .content {
    flex: 1 1 0;
    min-width: 0;
    overflow-y: auto;
  }
</style>
