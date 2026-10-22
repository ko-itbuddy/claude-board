// @ts-nocheck — node:test 로만 돌고 svelte-check 대상이 아니다
// 세션 줄 메뉴·클릭이 내는 동작 이름이 mac performAction 어휘(+ Windows 전용 activate)뿐인지 시험한다.
// 실행: cd windows/app && node --test src/lib/board/sessionActions.test.ts   (Node 22.18+/24 는 .ts 를 바로 읽는다)
import assert from "node:assert/strict";
import { test } from "node:test";

import { rowClickCall, sessionMenuEntries, type SessionMenuEntry } from "./sessionActions.ts";

const ALLOWED = ["activate", "focus", "reopen", "pause", "resume", "stop"];
const STATES = ["working", "waiting", "idle", "paused", "ended"] as const;

const sessionOf = (state: (typeof STATES)[number], paused = false, canDrive = true) => ({ state, paused, canDrive });

function names(entries: SessionMenuEntry[]): string[] {
  return entries.flatMap((entry) => ("separator" in entry ? [] : [entry.action]));
}

test("메뉴는 어느 상태에서든 mac 어휘의 동작 이름만 낸다", () => {
  for (const state of STATES) {
    for (const paused of [false, true]) {
      for (const name of names(sessionMenuEntries(sessionOf(state, paused)))) {
        assert.ok(ALLOWED.includes(name), `${state}/${paused}: ${name}`);
      }
    }
  }
});

test("끝난 세션은 reopen, 일시중지는 resume·stop, 그 밖은 pause·stop", () => {
  assert.deepEqual(names(sessionMenuEntries(sessionOf("ended"))), ["reopen"]);
  assert.deepEqual(names(sessionMenuEntries(sessionOf("paused"))), ["resume", "stop"]);
  for (const state of ["working", "waiting", "idle"] as const) {
    assert.deepEqual(names(sessionMenuEntries(sessionOf(state))), ["pause", "stop"]);
  }
});

test("끝난 세션의 이어 열기는 carryOn 을 false 로 고정해 보낸다(일시중지 표시 여부와 문구만 다르다)", () => {
  const [plain] = sessionMenuEntries(sessionOf("ended", false));
  const [carried] = sessionMenuEntries(sessionOf("ended", true));
  assert.ok(!("separator" in plain) && plain.carryOn === false && plain.label === "세션 다시 열기");
  assert.ok(!("separator" in carried) && carried.carryOn === false && carried.label === "세션 재개 (이어서 진행)");
});

test("일시중지 항목은 idle 이거나 키를 넣을 수 없으면 꺼진다", () => {
  const pauseOf = (state: (typeof STATES)[number], canDrive: boolean) => {
    const entry = sessionMenuEntries(sessionOf(state, false, canDrive)).find((e) => !("separator" in e) && e.action === "pause");
    return entry && !("separator" in entry) ? entry.enabled : undefined;
  };
  assert.equal(pauseOf("working", true), true);
  assert.equal(pauseOf("working", false), false);
  assert.equal(pauseOf("idle", true), false);
});

test("클릭 — 한 번은 activate, 두 번은 끝난 세션이면 reopen 아니면 focus", () => {
  assert.deepEqual(rowClickCall({ state: "working" }, 1), { action: "activate" });
  assert.deepEqual(rowClickCall({ state: "ended" }, 1), { action: "activate" });
  assert.deepEqual(rowClickCall({ state: "working" }, 2), { action: "focus" });
  assert.deepEqual(rowClickCall({ state: "ended" }, 2), { action: "reopen", carryOn: false });
  assert.deepEqual(rowClickCall({ state: "ended" }, 3), { action: "reopen", carryOn: false });
});

test("클릭이 내는 이름도 허용 집합 안이다", () => {
  for (const state of STATES) {
    for (const detail of [1, 2]) {
      assert.ok(ALLOWED.includes(rowClickCall({ state }, detail).action));
    }
  }
});
