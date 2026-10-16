// Tauri 에는 SSR 할 Node 서버가 없으므로 adapter-static 으로 SPA 를 만든다.
// 경로마다 미리 만든 껍데기(index.html·notify/index.html)를 쓰고, 그 밖의 경로는 200.html 이 받는다
// (index.html 을 fallback 으로 두면 미리 만든 index.html 을 덮어쓴다)
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      fallback: "200.html",
    }),
  },
};

export default config;
