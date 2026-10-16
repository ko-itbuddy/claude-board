<script lang="ts">
  import "../app.css";
  import { isTauri } from "@tauri-apps/api/core";
  import type { Snippet } from "svelte";

  let { children }: { children: Snippet } = $props();

  $effect(() => {
    const root = document.documentElement;
    if (!isTauri()) root.classList.add("browser");
    // 창이 숨거나 최소화되면 늘 도는 애니메이션을 멈춘다 — 보이지 않는 회전·파문을 다시 그리지 않게
    const sync = () => root.classList.toggle("anim-paused", document.hidden);
    sync();
    document.addEventListener("visibilitychange", sync);
    return () => document.removeEventListener("visibilitychange", sync);
  });
</script>

{@render children()}
