// @ts-nocheck — node:test 로만 돌고 svelte-check 대상이 아니다
// 실행: cd windows/app && node --test src/lib/terminal/resizeBarrier.test.ts
import { test } from "node:test";
import assert from "node:assert/strict";
import { ResizeBarrier } from "./resizeBarrier.ts";

const E = "\x1b";
const enc = (s) => Uint8Array.from(Buffer.from(s, "latin1"));
const vis = (s) => s.replaceAll(E, "~");
const flushPromises = () => new Promise((resolve) => setImmediate(resolve));
const label = (s) => `${s.cols}x${s.rows}`;

/** 가짜 port: 이벤트 기록 + 손으로 돌리는 시계 + 손으로 푸는 sendResize */
function makeHarness({ cur = { cols: 80, rows: 24 }, alt = false, bound = true, manualWrites = false } = {}) {
  const events = [];
  let size = { ...cur };
  let clock = 0;
  let nextTimer = 1;
  const timers = new Map();
  const sends = [];
  const pendingWrites = [];
  const port = {
    sendResize(s) {
      events.push(`send:${label(s)}`);
      return new Promise((resolve, reject) => sends.push({ size: s, resolve, reject }));
    },
    applyLocal(s) {
      size = { ...s };
      events.push(`apply:${label(s)}`);
    },
    write(bytes, done) {
      events.push(`write:${vis(Buffer.from(bytes).toString("latin1"))}`);
      if (manualWrites) pendingWrites.push(done);
      else done();
    },
    current: () => ({ ...size }),
    altScreen: () => alt,
    bound: () => bound,
    setTimer(fn, ms) {
      const id = nextTimer++;
      timers.set(id, { at: clock + ms, fn });
      return id;
    },
    clearTimer(id) {
      timers.delete(id);
    },
    log: (m) => events.push(`log:${m}`),
  };
  let seq = 0;
  let frameId = 0;
  const h = {
    events,
    sends,
    port,
    timers,
    barrier: new ResizeBarrier(port),
    /** 스트림에 프레임 하나를 보낸다. 반환: 프레임 번호 */
    frame(text) {
      const id = ++frameId;
      const bytes = enc(text);
      h.barrier.output(seq, bytes, () => events.push(`done:${id}`));
      seq += bytes.length;
      return id;
    },
    get seq() {
      return seq;
    },
    tick(ms) {
      clock += ms;
      for (const [id, t] of [...timers].sort((a, b) => a[1].at - b[1].at)) {
        if (t.at <= clock && timers.has(id)) {
          timers.delete(id);
          t.fn();
        }
      }
    },
    async resolveSend(index, anchor) {
      sends[index].resolve(anchor);
      await flushPromises();
    },
    completeWrite() {
      pendingWrites.shift()();
    },
    pendingWrites,
    size: () => size,
  };
  return h;
}

const SHRINK = { cols: 60, rows: 24 }; // 80x24 → 60x24: 한 단계, cut A
const NOTICE = `${E}[8;24;60t`;
const PRE = `${E}[?25l${NOTICE}\n`;
const only = (events, prefix) => events.filter((e) => e.startsWith(prefix));

test("대기 없는 Idle 이면 프레임을 그대로 쓰고 done 을 부른다", () => {
  const h = makeHarness();
  h.frame("hello");
  assert.deepEqual(h.events, ["write:hello", "done:1"]);
});

test("머리말이 오면 cut 앞을 쓴 뒤 콜백에서 applyLocal 하고 나머지를 푼다", async () => {
  const h = makeHarness({ manualWrites: true });
  h.barrier.request(SHRINK);
  assert.deepEqual(h.events, ["send:60x24"]);
  const early = h.frame("x".repeat(100)); // Sending 중: 붙잡는다
  assert.deepEqual(h.events, ["send:60x24"]);
  await h.resolveSend(0, 100);
  assert.deepEqual(h.events, ["send:60x24", `write:${"x".repeat(100)}`]);
  h.completeWrite();
  const late = h.frame(`${PRE}${E}[Hrepaint`);
  assert.equal(h.events.at(-1), `write:${vis(PRE)}`, "머리말 끝(ESC[H 앞)까지만 쓴다");
  assert.equal(only(h.events, "apply").length, 0, "쓰기 콜백 전에는 바꾸지 않는다");
  h.completeWrite();
  assert.equal(h.events.at(-2), "apply:60x24");
  assert.equal(h.events.at(-1), `write:${vis(`${E}[Hrepaint`)}`);
  h.completeWrite();
  assert.deepEqual(only(h.events, "done"), [`done:${early}`, `done:${late}`]);
});

test("붙잡은 프레임은 순서대로 쓰이고 쓴 뒤에만 done 이 불린다", async () => {
  const h = makeHarness({ manualWrites: true });
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.frame(`${PRE}${E}[Hone`);
  h.frame("two");
  h.frame("three");
  assert.deepEqual(only(h.events, "done"), [], "아직 아무것도 쓰이지 않았다");
  h.completeWrite(); // cut 앞
  const applyAt = h.events.indexOf("apply:60x24");
  assert.ok(applyAt > 0);
  assert.deepEqual(h.events.slice(applyAt + 1), [`write:${vis(`${E}[Hone`)}`, "write:two", "write:three"]);
  assert.deepEqual(only(h.events, "done"), [], "풀린 쓰기가 끝나기 전엔 done 없음");
  h.completeWrite();
  h.completeWrite();
  h.completeWrite();
  assert.deepEqual(only(h.events, "done"), ["done:1", "done:2", "done:3"]);
});

test("applied=null 이면 기다리지 않고 바로 바꾼다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  h.frame("queued");
  await h.resolveSend(0, null);
  assert.deepEqual(h.events.slice(1), ["apply:60x24", "write:queued", "done:1"]);
  h.frame("after");
  assert.equal(h.events.at(-2), "write:after");
});

test("sendResize 가 거부돼도 바로 바꾼다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  h.sends[0].reject(new Error("boom"));
  await flushPromises();
  assert.ok(h.events.includes("apply:60x24"));
  assert.ok(only(h.events, "log").some((l) => l.includes("boom")));
});

test("기한을 넘기면 줄 끝 폴백으로 다음 LF 뒤에서 자른다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.frame("hello");
  h.tick(999);
  h.frame("more");
  assert.equal(only(h.events, "apply").length, 0);
  h.tick(1);
  assert.equal(only(h.events, "apply").length, 0, "LF 가 아직 없다");
  h.frame("ab\ncd");
  assert.deepEqual(h.events.slice(-4), ["write:ab\n", "apply:60x24", "write:cd", "done:3"]);
});

test("64 KiB 를 훑고도 머리말이 없으면 줄 끝 폴백", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.frame("x".repeat(70000));
  assert.equal(only(h.events, "apply").length, 0);
  assert.ok(only(h.events, "log").some((l) => l.includes("64KiB")));
  h.frame("y\nz");
  assert.deepEqual(h.events.slice(-3), ["apply:60x24", "write:z", "done:2"]);
});

test("폴백에서 프레임이 50ms 안 오면 그 자리에서 바꾼다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.tick(1000);
  h.frame("abc");
  h.tick(49);
  assert.equal(only(h.events, "apply").length, 0);
  h.tick(1);
  assert.equal(h.events.at(-1), "apply:60x24");
});

test("폴백인데 아무 프레임도 없었으면 50ms 뒤 바로 바꾼다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.tick(1000);
  h.tick(50);
  assert.equal(h.events.at(-1), "apply:60x24");
});

test("두 단계 계획은 첫 단계를 바꾼 뒤에야 둘째를 보내고, 알림 없는 둘째 머리말을 찾는다", async () => {
  const h = makeHarness({ cur: { cols: 120, rows: 30 } });
  h.barrier.request({ cols: 60, rows: 40 });
  assert.deepEqual(h.events, ["send:60x30"]);
  await h.resolveSend(0, 0);
  const pre1 = `${E}[?25l${E}[8;30;60t\n`;
  h.frame(`old${pre1}${E}[Hp1`);
  assert.equal(only(h.events, "send").length, 2, "첫 단계를 바꾼 뒤 둘째를 보냈다");
  assert.ok(h.events.indexOf("apply:60x30") < h.events.lastIndexOf("send:60x40"));
  const afterFirst = h.events.length;
  await h.resolveSend(1, h.seq);
  // 앞 단계의 ESC[H 와 같은 꼴이 둘째 cut 자리 앞에 또 있다: 둘째는 자기 머리말(?25l CUP LF ESC[H)을 찾는다
  const pre2 = `${E}[?25l${E}[40;1H\n`;
  h.frame(`mid${pre2}${E}[Hp2`);
  assert.equal(h.events.at(-1), "done:2");
  assert.deepEqual(h.events.slice(afterFirst).filter((e) => e.startsWith("write") || e.startsWith("apply")), [
    `write:mid${vis(pre2)}`,
    "apply:60x40",
    `write:${vis(`${E}[Hp2`)}`,
  ]);
  assert.deepEqual(h.size(), { cols: 60, rows: 40 });
});

test("단계 중 요청 20개는 마지막 크기로 딱 한 번 더 계획된다", async () => {
  const h = makeHarness();
  h.barrier.request({ cols: 60, rows: 24 });
  for (let n = 1; n <= 19; n++) h.barrier.request({ cols: 60 - n, rows: 24 });
  assert.equal(h.sends.length, 1);
  await h.resolveSend(0, null);
  assert.deepEqual(only(h.events, "send"), ["send:60x24", "send:41x24"]);
  await h.resolveSend(1, null);
  assert.equal(only(h.events, "send").length, 2);
  assert.deepEqual(h.size(), { cols: 41, rows: 24 });
});

test("스캔 중 gap 이면 기다림을 버리고 계획한 크기로 바꾼 뒤 붙잡은 프레임을 순서대로 푼다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.frame(`a${E}`); // 끝의 ESC 가 붙잡힌다
  assert.deepEqual(h.events.slice(1), ["write:a"]);
  h.barrier.interrupt("gap");
  assert.deepEqual(h.events.slice(2).filter((e) => !e.startsWith("log")), ["apply:60x24", "write:~", "done:1"], "붙잡은 것은 바꾼 뒤에 푼다");
  assert.equal(h.events.filter((e) => e === "apply:60x24").length, 1);
  const before = h.events.length;
  h.tick(5000);
  assert.equal(h.events.length, before, "타이머가 남지 않았다");
  assert.equal(h.timers.size, 0);
});

test("Sending 중 exit 이면 바로 바꾸고 늦은 응답과 밀린 요청은 무시한다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  h.frame("data");
  h.barrier.request({ cols: 40, rows: 24 });
  h.barrier.interrupt("exit");
  assert.deepEqual(h.events.slice(1).filter((e) => !e.startsWith("log")), ["apply:60x24", "write:data", "done:1"]);
  await h.resolveSend(0, 0);
  assert.equal(only(h.events, "send").length, 1);
  assert.equal(h.events.length, 5);
});

test("붙기 전에는 sendResize 를 부르지 않고 상태도 두지 않는다", () => {
  const h = makeHarness({ bound: false });
  h.barrier.request(SHRINK);
  assert.equal(h.sends.length, 0);
  h.frame("raw");
  assert.deepEqual(h.events.slice(-2), ["write:raw", "done:1"]);
});

test("폴백 뒤에 늦게 온 머리말은 그냥 데이터다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.tick(1000);
  h.frame("a\nb");
  assert.equal(h.size().cols, 60);
  const count = h.events.length;
  h.frame(`${PRE}${E}[Hlate`);
  assert.deepEqual(h.events.slice(count), [`write:${vis(`${PRE}${E}[Hlate`)}`, "done:2"]);
  assert.equal(only(h.events, "send").length, 1);
});

test("dispose 는 타이머를 걷고 이후 port 를 부르지 않는다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.frame(`a${E}`);
  assert.ok(h.timers.size > 0);
  h.barrier.dispose();
  assert.equal(h.timers.size, 0);
  const count = h.events.length;
  h.tick(5000);
  h.barrier.request({ cols: 50, rows: 24 });
  h.frame("late");
  h.barrier.interrupt("gap");
  assert.equal(h.events.length, count);
});

test("Sending 중 dispose 한 뒤 늦은 응답도 port 를 부르지 않는다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  h.barrier.dispose();
  const count = h.events.length;
  await h.resolveSend(0, null);
  assert.equal(h.events.length, count);
});

test("스캔 중 exit 이면 바로 바꾸고 이후 요청은 다시 계획하지 않는다", async () => {
  const h = makeHarness();
  h.barrier.request(SHRINK);
  await h.resolveSend(0, 0);
  h.barrier.request({ cols: 40, rows: 24 });
  h.barrier.interrupt("exit");
  assert.ok(h.events.includes("apply:60x24"));
  assert.equal(only(h.events, "send").length, 1);
  assert.equal(h.timers.size, 0);
});

test("같은 크기 요청은 아무것도 하지 않는다", () => {
  const h = makeHarness();
  h.barrier.request({ cols: 80, rows: 24 });
  assert.deepEqual(h.events, []);
});
