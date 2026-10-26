import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { VerdictCache, cachedJudge, sha256 } from './cache.mjs';
import { FakeJudge } from './adapters/fakeJudge.mjs';
import { tempRepo, writePng } from './testkit/helpers.mjs';

const PASS = { status: 'PASS', reason: 'jev pass', noul: 0.97 };

test('같은 진술과 같은 이미지는 안쪽을 한 번만 부른다', async () => {
  const t = tempRepo();
  try {
    const image = writePng(t.out, 'a.png');
    const fake = new FakeJudge({ check: PASS });
    const judge = cachedJudge(fake, new VerdictCache(path.join(t.out, 'jev-cache')));
    const first = await judge.check(image, 'the chip is visible');
    const second = await judge.check(image, 'the chip is visible');
    assert.equal(first.status, 'PASS');
    assert.equal(second.cached, true);
    assert.equal(fake.calls.length, 1);
  } finally {
    t.cleanup();
  }
});

test('진술·기대값·이미지가 다르면 따로 묻는다', async () => {
  const t = tempRepo();
  try {
    const a = writePng(t.out, 'a.png', 320, 300);
    const b = writePng(t.out, 'b.png', 330, 300);
    const fake = new FakeJudge({ check: PASS });
    const judge = cachedJudge(fake, new VerdictCache(path.join(t.out, 'jev-cache')));
    await judge.check(a, 'one');
    await judge.check(a, 'two');
    await judge.check(a, 'one', 'no');
    await judge.check(b, 'one');
    assert.equal(fake.calls.length, 4);
  } finally {
    t.cleanup();
  }
});

test('UNVERIFIED 는 저장하지 않아 키가 생기면 다시 묻는다', async () => {
  const t = tempRepo();
  try {
    const image = writePng(t.out, 'a.png');
    const dir = path.join(t.out, 'jev-cache');
    const fake = new FakeJudge({ check: [{ status: 'UNVERIFIED', reason: 'no Jev key' }, PASS] });
    const judge = cachedJudge(fake, new VerdictCache(dir));
    assert.equal((await judge.check(image, 's')).status, 'UNVERIFIED');
    assert.equal(fs.existsSync(dir) ? fs.readdirSync(dir).length : 0, 0);
    assert.equal((await judge.check(image, 's')).status, 'PASS');
    assert.equal(fake.calls.length, 2);
  } finally {
    t.cleanup();
  }
});

test('FAIL 과 UNSURE 는 저장한다', async () => {
  const t = tempRepo();
  try {
    const image = writePng(t.out, 'a.png');
    const cache = new VerdictCache(path.join(t.out, 'jev-cache'));
    const fake = new FakeJudge({ check: [{ status: 'FAIL', reason: 'jev fail' }, { status: 'UNSURE', reason: 'jev unsure' }] });
    const judge = cachedJudge(fake, cache);
    await judge.check(image, 'x');
    await judge.check(image, 'y');
    assert.equal((await judge.check(image, 'x')).status, 'FAIL');
    assert.equal((await judge.check(image, 'y')).status, 'UNSURE');
    assert.equal(fake.calls.length, 2);
  } finally {
    t.cleanup();
  }
});

test('checkAll: 적중하지 못한 질문만 한 요청으로 묶어 묻는다', async () => {
  const t = tempRepo();
  try {
    const image = writePng(t.out, 'a.png');
    const fake = new FakeJudge({ check: PASS });
    const judge = cachedJudge(fake, new VerdictCache(path.join(t.out, 'jev-cache')));
    await judge.check(image, 'one');
    const all = await judge.checkAll(image, [{ name: 'a', statement: 'one' }, { name: 'b', statement: 'two' }, { name: 'c', statement: 'three' }]);
    assert.deepEqual(all.map((v) => Boolean(v.cached)), [true, false, false]);
    const batch = fake.calls.find((c) => c.verb === 'checkAll');
    assert.deepEqual(batch.items.map((i) => i.statement), ['two', 'three']);
  } finally {
    t.cleanup();
  }
});

test('screen 도 캐시한다', async () => {
  const t = tempRepo();
  try {
    const image = writePng(t.out, 'a.png');
    const fake = new FakeJudge({ screen: { status: 'PASS', reason: 'screen=settings', choice: 'settings' } });
    const judge = cachedJudge(fake, new VerdictCache(path.join(t.out, 'jev-cache')));
    await judge.screen(image, ['settings', 'board-expanded'], 'settings');
    assert.equal((await judge.screen(image, ['settings', 'board-expanded'], 'settings')).cached, true);
    await judge.screen(image, ['settings', 'board-expanded'], 'board-expanded');
    assert.equal(fake.calls.length, 2);
  } finally {
    t.cleanup();
  }
});

test('깨진 캐시 파일은 삼키지 않고 던진다', async () => {
  const t = tempRepo();
  try {
    const image = writePng(t.out, 'a.png');
    const dir = path.join(t.out, 'jev-cache');
    const cache = new VerdictCache(dir);
    const key = VerdictCache.key(['check', 's', 'yes', sha256(fs.readFileSync(image))]);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, `v-${key}.json`), '{not json');
    const judge = cachedJudge(new FakeJudge({ check: PASS }), cache);
    await assert.rejects(() => judge.check(image, 's'), /깨졌다/);
  } finally {
    t.cleanup();
  }
});

test('캐시 키는 sha256 hex 만 받는다(경로 주입 방지)', () => {
  const cache = new VerdictCache('x');
  assert.throws(() => cache.get('../evil'), TypeError);
  assert.throws(() => cache.put('../evil', PASS), TypeError);
});
