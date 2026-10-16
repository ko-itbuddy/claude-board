// 캐릭터 동작의 공유 시계 — 모든 캐릭터가 requestAnimationFrame 하나를 함께 본다(초당 30번으로 묶음).
// 따로 돌면 캐릭터마다 화면을 따로 다시 그리고, 화면 주사율만큼 그리면 보드가 버벅인다.
// 보는 캐릭터가 없거나 창이 숨으면(document.hidden) 멈춘다
import { createSubscriber } from "svelte/reactivity";

let now = performance.now() / 1000;

const subscribe = createSubscriber((update) => {
  let raf = 0;
  let last = 0;
  const frame = (ms: number) => {
    raf = requestAnimationFrame(frame);
    if (ms - last < 1000 / 30 - 2) return;
    last = ms;
    now = ms / 1000;
    update();
  };
  const start = () => {
    if (!raf && !document.hidden) raf = requestAnimationFrame(frame);
  };
  const stop = () => {
    cancelAnimationFrame(raf);
    raf = 0;
  };
  const visibility = () => (document.hidden ? stop() : start());
  document.addEventListener("visibilitychange", visibility);
  start();
  return () => {
    stop();
    document.removeEventListener("visibilitychange", visibility);
  };
});

export const clock = {
  // 지금 시각(초) — 읽는 쪽은 초당 30번 다시 그려진다
  get t() {
    subscribe();
    return now;
  },
  // 다시 그리기를 걸지 않고 지금 시각만 읽는다(말풍선이 바뀐 시각 기록용)
  peek: () => performance.now() / 1000,
};
