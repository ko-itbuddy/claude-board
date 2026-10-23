// @ts-nocheck — node:test 로만 돌고 svelte-check 대상이 아니다
// 실행: cd windows/app && node --test src/lib/board/clickGate.test.ts
import assert from "node:assert/strict";
import { test } from "node:test";

import { createClickGate } from "./clickGate.ts";

// 가짜 시계 — advance 로만 시간이 흐른다
function harness(delayMs) {
  let now = 0;
  let nextHandle = 1;
  const timers = new Map();
  const log = [];
  const gate = createClickGate({
    delayMs,
    setTimer: (callback, ms) => {
      const handle = nextHandle++;
      timers.set(handle, { at: now + ms, callback });
      return handle;
    },
    clearTimer: (handle) => timers.delete(handle),
    onActivate: () => log.push("activate"),
    onFocus: () => log.push("focus"),
  });
  const advance = (ms) => {
    now += ms;
    for (const [handle, timer] of [...timers]) {
      if (timer.at <= now) {
        timers.delete(handle);
        timer.callback();
      }
    }
  };
  return { gate, log, advance, timers };
}

test("한 번 클릭 — T 전에는 아무것도 없고 T 에 activate 한 번", () => {
  const { gate, log, advance } = harness(500);
  gate.press(1);
  advance(499);
  assert.deepEqual(log, []);
  advance(1);
  assert.deepEqual(log, ["activate"]);
  advance(5000);
  assert.deepEqual(log, ["activate"]);
});

test("T 안의 두 번째 클릭 — focus 한 번, activate 없음", () => {
  const { gate, log, advance } = harness(500);
  gate.press(1);
  advance(200);
  gate.press(2);
  advance(5000);
  assert.deepEqual(log, ["focus"]);
});

test("세 번째 클릭은 focus 를 더 내지 않는다", () => {
  const { gate, log, advance } = harness(500);
  gate.press(1);
  gate.press(2);
  gate.press(3);
  advance(5000);
  assert.deepEqual(log, ["focus"]);
});

test("T 보다 떨어진 두 번의 한 번 클릭 — activate 두 번", () => {
  const { gate, log, advance } = harness(500);
  gate.press(1);
  advance(600);
  gate.press(1);
  advance(600);
  assert.deepEqual(log, ["activate", "activate"]);
});

test("T 전에 dispose 하면 아무것도 나가지 않는다", () => {
  const { gate, log, advance, timers } = harness(500);
  gate.press(1);
  gate.dispose();
  advance(5000);
  assert.deepEqual(log, []);
  assert.equal(timers.size, 0);
});

test("지연 0 — detail 1 에서 바로 activate, detail 2 에서 바로 focus, 타이머 없음", () => {
  const { gate, log, timers } = harness(0);
  gate.press(1);
  assert.deepEqual(log, ["activate"]);
  gate.press(2);
  assert.deepEqual(log, ["activate", "focus"]);
  assert.equal(timers.size, 0);
});

test("대기 중 또 detail 1 이 오면 앞 타이머는 취소되고 새 T 로 센다", () => {
  const { gate, log, advance } = harness(500);
  gate.press(1);
  advance(300);
  gate.press(1);
  advance(300);
  assert.deepEqual(log, []);
  advance(200);
  assert.deepEqual(log, ["activate"]);
});
