// @ts-nocheck — node:test 로만 돌고 svelte-check 대상이 아니다
// 세션 줄 메뉴가 내는 동작 이름이 mac performAction 어휘(+ Windows 전용 activate)뿐인지, 상태별 항목·설명이 mac 표와 같은지 시험한다.
// 실행: cd windows/app && node --test src/lib/board/sessionActions.test.ts   (Node 22.18+/24 는 .ts 를 바로 읽는다)
import assert from "node:assert/strict";
import { test } from "node:test";

import { sessionMenuEntries, type SessionMenuEntry } from "./sessionActions.ts";

const ALLOWED = ["activate", "focus", "reopen", "pause", "resume", "stop"];
const STATES = ["working", "waiting", "idle", "paused", "ended"] as const;
const TAB = { kind: "tab", surfaceId: "s1", headless: false };
const TERMINAL = { kind: "terminal" };
const BACKGROUND = { kind: "background" };
const ENDED = { kind: "ended" };

const sessionOf = (state: (typeof STATES)[number], over = {}) => ({ state, paused: false, canDrive: true, headless: false, ...over });

const actionsOf = (entries: SessionMenuEntry[]) => entries.filter((entry) => !("separator" in entry));
const names = (entries: SessionMenuEntry[]) => actionsOf(entries).map((entry) => entry.action);
const pauseEntry = (state, canDrive) => actionsOf(sessionMenuEntries(sessionOf(state, { canDrive }), TAB)).find((e) => e.action === "pause");

test("메뉴는 어느 상태·자리에서든 mac 어휘의 동작 이름만 낸다", () => {
  for (const state of STATES) {
    for (const paused of [false, true]) {
      for (const place of [TAB, TERMINAL, ENDED]) {
        for (const name of names(sessionMenuEntries(sessionOf(state, { paused }), place))) {
          assert.ok(ALLOWED.includes(name), `${state}/${paused}/${place.kind}: ${name}`);
        }
      }
    }
  }
});

test("끝난 세션은 reopen, 일시중지는 resume·stop, 그 밖은 pause·stop", () => {
  assert.deepEqual(names(sessionMenuEntries(sessionOf("ended"), ENDED)), ["reopen"]);
  assert.deepEqual(names(sessionMenuEntries(sessionOf("paused"), TAB)), ["resume", "stop"]);
  for (const state of ["working", "waiting", "idle"] as const) {
    assert.deepEqual(names(sessionMenuEntries(sessionOf(state), TERMINAL)), ["pause", "stop"]);
  }
});

test("살아 있는 background 세션은 메뉴가 없다", () => {
  for (const state of ["working", "waiting", "idle", "paused"] as const) {
    assert.deepEqual(sessionMenuEntries(sessionOf(state, { headless: true }), BACKGROUND), []);
  }
});

test("끝난 세션 — 일시중지 표시가 있으면 재개 문구와 carryOn true, 없으면 다시 열기와 carryOn false", () => {
  const [plain] = sessionMenuEntries(sessionOf("ended"), ENDED);
  const [carried] = sessionMenuEntries(sessionOf("ended", { paused: true }), ENDED);
  assert.deepEqual(
    { label: plain.label, carryOn: plain.carryOn, action: plain.action },
    { label: "세션 다시 열기", carryOn: false, action: "reopen" },
  );
  assert.deepEqual(
    { label: carried.label, carryOn: carried.carryOn, action: carried.action },
    { label: "세션 재개 (이어서 진행)", carryOn: true, action: "reopen" },
  );
  assert.equal(plain.detail, "보드 새 탭에서 이 대화를 이어 엽니다");
  assert.equal(carried.detail, "보드 새 탭에서 열고 멈춘 작업을 이어 갑니다");
});

test("끝난 headless 세션의 다시 열기는 대화형으로 이어 연다는 설명을 단다", () => {
  const [entry] = sessionMenuEntries(sessionOf("ended", { headless: true }), ENDED);
  assert.equal(entry.label, "세션 다시 열기");
  assert.equal(entry.detail, "보드 새 탭에서 대화형으로 이어 엽니다");
});

test("일시중지 세션 — 재개와 중단, 설명 포함", () => {
  const [resume, , stop] = sessionMenuEntries(sessionOf("paused"), TAB);
  assert.equal(resume.detail, "멈춘 지점부터 이어서 진행하게 합니다");
  assert.deepEqual({ label: stop.label, detail: stop.detail }, { label: "세션 중단", detail: "응답을 멈추고 끝냅니다. 대화 기록은 남습니다" });
});

test("working·waiting 에서 키를 넣을 수 있으면 일시중지가 켜진다", () => {
  for (const state of ["working", "waiting"] as const) {
    const entry = pauseEntry(state, true);
    assert.equal(entry.enabled, true);
    assert.equal(entry.detail, "지금 응답을 멈춥니다. 재개하면 이어 갑니다");
  }
});

test("idle 이면 일시중지가 꺼지고 대기 설명이 붙는다", () => {
  const entry = pauseEntry("idle", true);
  assert.equal(entry.enabled, false);
  assert.equal(entry.detail, "대기 중이라 멈출 응답이 없습니다");
});

test("키를 넣을 수 없으면 일시중지가 꺼지고 키 설명이 붙는다", () => {
  const entry = pauseEntry("working", false);
  assert.equal(entry.enabled, false);
  assert.equal(entry.detail, "이 터미널에는 키를 넣을 수 없습니다");
});
