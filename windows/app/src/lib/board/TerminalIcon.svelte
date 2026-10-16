<script lang="ts">
  // 터미널 독 아이콘 — 앱 아이콘을 읽지 않고 앱마다 직접 그린 그림(SPEC §5.11)
  let { id }: { id: string } = $props();
  const uid = $props.id();
</script>

<svg width="36" height="36" viewBox="0 0 36 36" aria-hidden="true">
  <defs>
    <linearGradient id="{uid}-bg" x1="0" y1="0" x2="0" y2="1">
      {#if id === "wt"}
        <stop offset="0" stop-color="#2b2b33" /><stop offset="1" stop-color="#16161b" />
      {:else if id === "pwsh"}
        <stop offset="0" stop-color="#4b5a6e" /><stop offset="1" stop-color="#26303d" />
      {:else if id === "powershell"}
        <stop offset="0" stop-color="#1d5fa8" /><stop offset="1" stop-color="#012456" />
      {:else if id === "cmd"}
        <stop offset="0" stop-color="#2a2a2a" /><stop offset="1" stop-color="#0c0c0c" />
      {:else}
        <stop offset="0" stop-color="#2e2e34" /><stop offset="1" stop-color="#17171b" />
      {/if}
    </linearGradient>
  </defs>
  <rect x="2" y="2" width="32" height="32" rx="8" fill="url(#{uid}-bg)" stroke="rgb(255 255 255 / 0.16)" stroke-width="1" />
  {#if id === "wt"}
    <!-- Windows Terminal: 파란 탭 막대 + 프롬프트 -->
    <path d="M2 10V10a8 8 0 0 1 8-8h16a8 8 0 0 1 8 8z" fill="#0f6cbd" />
    <rect x="6" y="5" width="9" height="3" rx="1.5" fill="#4cc2ff" />
    <path d="M9 16l5 4-5 4" fill="none" stroke="#fff" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M17 26h9" stroke="#fff" stroke-width="2.4" stroke-linecap="round" />
  {:else if id === "pwsh" || id === "powershell"}
    <!-- PowerShell: 기울어진 판 위의 >_ -->
    <path d="M11 9h18l-4 18H7z" fill={id === "pwsh" ? "#2f3b4b" : "#0a3d7a"} stroke="rgb(255 255 255 / 0.25)" stroke-width="0.8" />
    <path d="M12.5 13.5l6 4.5-7.5 5" fill="none" stroke="#fff" stroke-width="2.3" stroke-linecap="round" stroke-linejoin="round" />
    <path d="M17 23.5h6.5" stroke="#fff" stroke-width="2.3" stroke-linecap="round" />
    {#if id === "pwsh"}<text x="27" y="31" font-size="7" font-weight="800" fill="#7fb2ff" font-family="Segoe UI, sans-serif">7</text>{/if}
  {:else if id === "cmd"}
    <!-- 명령 프롬프트: C:\_ -->
    <rect x="2" y="2" width="32" height="6" rx="3" fill="rgb(255 255 255 / 0.12)" />
    <text x="6" y="24" font-size="10.5" font-weight="700" fill="#e6e6e6" font-family="Consolas, Menlo, monospace">C:\_</text>
  {:else if id === "git-bash"}
    <!-- Git Bash: Git 마름모와 가지 -->
    <rect x="9" y="9" width="18" height="18" rx="3" transform="rotate(45 18 18)" fill="#f05133" />
    <path d="M15 13.5l7.5 7.5M15 13.5v9" stroke="#fff" stroke-width="1.8" stroke-linecap="round" />
    <circle cx="15" cy="13.5" r="1.9" fill="#fff" /><circle cx="15" cy="22.5" r="1.9" fill="#fff" /><circle cx="22.5" cy="21" r="1.9" fill="#fff" />
  {:else}
    <path d="M10 14l5 4-5 4M17 24h8" fill="none" stroke="#fff" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" />
  {/if}
</svg>
