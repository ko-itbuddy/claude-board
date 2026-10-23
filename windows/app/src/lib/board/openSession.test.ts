// @ts-nocheck — node:test 로만 돌고 svelte-check 대상이 아니다
// 실행: cd windows/app && node --test src/lib/board/openSession.test.ts
import assert from "node:assert/strict";
import { test } from "node:test";

import { backgroundNotice, openSession } from "./openSession.ts";

const sessionOf = (over = {}) => ({ title: "빌드 고치기", mode: null, headless: false, paused: false, ...over });

test("tab — 한 번이든 두 번이든 그 화면을 reveal 한다", () => {
  for (const double of [false, true]) {
    assert.deepEqual(openSession(sessionOf(), { kind: "tab", surfaceId: "s1", headless: false }, double), { kind: "reveal", surfaceId: "s1" });
  }
});

test("tab 에 붙들린 끝난 세션도 reveal 이다", () => {
  const ended = sessionOf({ paused: true });
  assert.deepEqual(openSession(ended, { kind: "tab", surfaceId: "s9", headless: false }, true), { kind: "reveal", surfaceId: "s9" });
});

test("terminal — 한 번은 activate, 두 번은 focus", () => {
  assert.deepEqual(openSession(sessionOf(), { kind: "terminal" }, false), { kind: "action", action: "activate" });
  assert.deepEqual(openSession(sessionOf(), { kind: "terminal" }, true), { kind: "action", action: "focus" });
});

test("background — 한 번·두 번 모두 같은 알림", () => {
  const s = sessionOf({ mode: "--bg" });
  const once = openSession(s, { kind: "background" }, false);
  assert.deepEqual(once, { kind: "notice", text: backgroundNotice(s) });
  assert.deepEqual(openSession(s, { kind: "background" }, true), once);
});

test("알림 문구는 mode 가 없으면 -p 를 쓴다", () => {
  assert.equal(
    backgroundNotice(sessionOf()),
    "빌드 고치기: claude -p 로 실행된 세션이라 터미널이 없습니다. 보드에서는 상태만 보여 주고, 업데이트·종료에도 영향받지 않습니다.",
  );
  assert.ok(backgroundNotice(sessionOf({ mode: "--bg" })).includes("claude --bg 로"));
});

test("ended — 일시중지 표시가 없으면 carryOn false 로 reopen", () => {
  for (const double of [false, true]) {
    assert.deepEqual(openSession(sessionOf(), { kind: "ended" }, double), { kind: "action", action: "reopen", carryOn: false });
  }
});

test("ended + paused — carryOn true 로 reopen", () => {
  assert.deepEqual(openSession(sessionOf({ paused: true }), { kind: "ended" }, false), { kind: "action", action: "reopen", carryOn: true });
});

test("ended + headless — 아무것도 하지 않는다", () => {
  for (const double of [false, true]) {
    assert.deepEqual(openSession(sessionOf({ headless: true, mode: "-p" }), { kind: "ended" }, double), { kind: "none" });
  }
});
