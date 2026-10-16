// Tauri 에는 SSR 할 Node 서버가 없으므로 SPA 로 쓴다. 경로마다 빈 껍데기 HTML 을 미리 만들어
// build/index.html·build/notify/index.html 이 생기게 한다(Tauri 는 notify → notify/index.html 순으로 찾는다)
export const ssr = false;
export const prerender = true;
export const trailingSlash = "always";
