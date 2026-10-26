<script lang="ts">
  // 설정 > 일반 — 항상 위에 고정 · 로그인할 때 열기 · 제목 옆 캐릭터 (mac 일반 묶음)
  import { board } from "$lib/board/store.svelte";
  import { loginLaunch, setLoginLaunch, setMascot, setPinned } from "$lib/backend";
  import CardSection from "../controls/CardSection.svelte";
  import SwitchControl from "../controls/SwitchControl.svelte";
  import MascotGrid from "../MascotGrid.svelte";
  import SettingsCard from "../SettingsCard.svelte";
  import { GENERAL_ITEMS } from "./generalItems";

  const title = (id: string) => GENERAL_ITEMS.find((item) => item.id === id)?.title ?? id;
  const settings = $derived(board.snap?.settings);

  // 로그인할 때 열기는 표지 파일이 정한다(상태 묶음에 없다) — 열 때 읽고, 바꾸면 바로 따라간다
  let login = $state(true);
  $effect(() => {
    let alive = true;
    loginLaunch().then((on) => {
      if (alive) login = on;
    });
    return () => (alive = false);
  });

  function switchLogin(on: boolean) {
    login = on;
    setLoginLaunch(on);
  }
</script>

{#if settings}
  <div class="group" data-testid="settings-group-general">
    <CardSection title="보드">
      <div data-settings-item="pin">
        <SettingsCard title={title("pin")} detail="다른 앱을 눌러도 보드가 위에 남습니다. 화면 전체를 덮도록 펼친 동안에는 쉽니다">
          <SwitchControl on={settings.pinned} label={title("pin")} onchange={setPinned} />
        </SettingsCard>
      </div>
      <div data-settings-item="login">
        <SettingsCard title={title("login")} detail="Windows 에 로그인하면 보드를 띄웁니다. 바꾸면 다음 로그인부터 적용됩니다">
          <SwitchControl on={login} label={title("login")} onchange={switchLogin} />
        </SettingsCard>
      </div>
    </CardSection>
    <CardSection title="캐릭터">
      <div class="block" data-settings-item="mascot">
        <SettingsCard title={title("mascot")} detail="세션 상태를 말풍선으로 알려 주는 캐릭터. 캐릭터를 오른쪽 클릭해도 고를 수 있습니다" />
        <MascotGrid selected={settings.mascot} onpick={setMascot} />
      </div>
    </CardSection>
  </div>
{/if}

<style>
  .group {
    padding-block: 20px;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
</style>
