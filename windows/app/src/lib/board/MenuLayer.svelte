<script lang="ts">
  import MenuList from "./MenuList.svelte";
  import { closeMenu, menuState } from "./menu.svelte";

  $effect(() => {
    if (!menuState.items) return;
    const key = (e: KeyboardEvent) => e.key === "Escape" && closeMenu();
    window.addEventListener("keydown", key);
    window.addEventListener("blur", closeMenu);
    window.addEventListener("resize", closeMenu);
    return () => {
      window.removeEventListener("keydown", key);
      window.removeEventListener("blur", closeMenu);
      window.removeEventListener("resize", closeMenu);
    };
  });
</script>

{#if menuState.items}
  <!-- 메뉴 밖을 누르면 닫는다 — 그 누름은 아래 단추에 닿지 않는다(macOS 메뉴와 같게) -->
  <div
    class="scrim"
    role="presentation"
    onmousedown={closeMenu}
    oncontextmenu={(e) => {
      e.preventDefault();
      closeMenu();
    }}
    onwheel={closeMenu}
  ></div>
  <MenuList items={menuState.items} anchor={{ x: menuState.x, y: menuState.y }} onDone={closeMenu} />
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 899;
  }
</style>
