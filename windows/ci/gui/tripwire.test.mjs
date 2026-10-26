// tripwire.ps1 시험 — 임시 폴더의 사본과 임시 프로세스만 쓴다. 실제 settings.json·profile.ps1·진짜 claude 는 건드리지도 읽지도 않는다.
// 실행: node --test windows/ci/gui/tripwire.test.mjs   (pwsh 가 필요하다. Windows PowerShell 5.1 이 있으면 selftest 를 거기서도 돈다)
import { test } from 'node:test';
import assert from 'node:assert/strict';
import cp from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const SCRIPT = path.join(path.dirname(fileURLToPath(import.meta.url)), 'tripwire.ps1');
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const has = (exe) => cp.spawnSync(exe, ['-NoProfile', '-Command', '1'], { encoding: 'utf8' }).status === 0;
const HAS_PWSH = has('pwsh');
const HAS_PS51 = process.platform === 'win32' && has('powershell');

function run(shell, args) {
  const result = cp.spawnSync(shell, ['-NoProfile', '-File', SCRIPT, ...args], { encoding: 'utf8' });
  return { code: result.status, text: `${result.stdout}${result.stderr}` };
}

// 지킬 파일 사본과 상태 파일 자리
function fixture() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'tripwire-test-'));
  const settings = path.join(dir, 'home', '.claude', 'settings.json');
  const profile = path.join(dir, 'docs', 'PowerShell', 'profile.ps1');
  fs.mkdirSync(path.dirname(settings), { recursive: true });
  fs.mkdirSync(path.dirname(profile), { recursive: true });
  fs.writeFileSync(settings, '{"secret-marker-settings":1}');
  fs.writeFileSync(profile, '# secret-marker-profile');
  const state = path.join(dir, 'state', 'state.json');
  const watch = ['-State', state, '-SettingsPath', settings, '-ProfilePaths', profile];
  return { dir, settings, profile, state, watch, cleanup: () => fs.rmSync(dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 50 }) };
}

for (const [label, enabled, shell] of [['pwsh', HAS_PWSH, 'pwsh'], ['Windows PowerShell 5.1', HAS_PS51, 'powershell']]) {
  test(`selftest(${label}) — 임시 사본의 변화를 잡고 안 바뀐 것은 통과시킨다`, { skip: !enabled }, () => {
    const r = run(shell, ['-Phase', 'selftest']);
    assert.equal(r.code, 0, r.text);
    assert.match(r.text, /TRIPWIRE\|SELFTEST\|PASS/);
    assert.match(r.text, /TRIPWIRE\|CHANGED\|profile:PowerShell\|/);
    assert.doesNotMatch(r.text, /selftest profile/, '파일 내용을 출력하지 않는다');
  });
}

test('before → after: 그대로면 0, 설정 한 글자만 바뀌어도 1 과 CHANGED 줄(해시만 나온다)', { skip: !HAS_PWSH }, () => {
  const f = fixture();
  try {
    const before = run('pwsh', ['-Phase', 'before', ...f.watch]);
    assert.equal(before.code, 0, before.text);
    assert.match(before.text, /TRIPWIRE\|RECORDED\|settings\.json\|[0-9a-f]{32}/);
    const same = run('pwsh', ['-Phase', 'after', ...f.watch.slice(0, 2)]);
    assert.equal(same.code, 0, same.text);
    assert.match(same.text, /TRIPWIRE\|SAME\|settings\.json\|/);
    fs.appendFileSync(f.settings, ' ');
    const changed = run('pwsh', ['-Phase', 'after', '-State', f.state]);
    assert.equal(changed.code, 1, changed.text);
    assert.match(changed.text, /TRIPWIRE\|CHANGED\|settings\.json\|[0-9a-f]{32}\|[0-9a-f]{32}/);
    for (const text of [before.text, same.text, changed.text]) {
      assert.doesNotMatch(text, /secret-marker/, '파일 내용을 출력하지 않는다');
    }
  } finally {
    f.cleanup();
  }
});

test('지운 파일·새로 생긴 파일도 바뀐 것이다(ABSENT)', { skip: !HAS_PWSH }, () => {
  const f = fixture();
  try {
    fs.rmSync(f.profile);
    assert.equal(run('pwsh', ['-Phase', 'before', ...f.watch]).code, 0);
    assert.equal(run('pwsh', ['-Phase', 'after', '-State', f.state]).code, 0, '없던 파일이 계속 없으면 같다');
    fs.writeFileSync(f.profile, '# 새로 생김');
    const created = run('pwsh', ['-Phase', 'after', '-State', f.state]);
    assert.equal(created.code, 1, created.text);
    assert.match(created.text, /TRIPWIRE\|CHANGED\|profile:PowerShell\|ABSENT\|[0-9a-f]{32}/);
  } finally {
    f.cleanup();
  }
});

test('사용법 오류는 2 — 상태 파일 없음, -State 없음, 상태 파일이 지키는 폴더 안', { skip: !HAS_PWSH }, () => {
  const f = fixture();
  try {
    assert.equal(run('pwsh', ['-Phase', 'after', '-State', path.join(f.dir, 'none.json')]).code, 2);
    assert.equal(run('pwsh', ['-Phase', 'before']).code, 2);
    const inside = path.join(path.dirname(f.settings), 'state.json');
    const r = run('pwsh', ['-Phase', 'before', '-State', inside, '-SettingsPath', f.settings, '-ProfilePaths', f.profile]);
    assert.equal(r.code, 2, r.text);
    assert.equal(fs.existsSync(inside), false);
  } finally {
    f.cleanup();
  }
});

// ── 프로세스: 보드 자손 중 허용 폴더 밖 claude.exe ─────────────────────────────

// node.exe 사본(하드 링크가 되면 링크) — 이름을 claude.exe·board.exe 로 둔다
function nodeCopy(target) {
  fs.mkdirSync(path.dirname(target), { recursive: true });
  try {
    fs.linkSync(process.execPath, target);
  } catch {
    fs.copyFileSync(process.execPath, target);
  }
  return target;
}

const killTree = (pid) => cp.spawnSync('taskkill', ['/PID', String(pid), '/T', '/F'], { encoding: 'utf8' });

// 보드 역할 프로세스가 claudeExe 를 자식으로 띄운다. 자식 pid 는 파일로 받는다
async function startBoardWith(dir, claudeExe) {
  const board = nodeCopy(path.join(dir, 'board', 'board.exe'));
  const childPidFile = path.join(dir, 'child.pid');
  const boardScript = path.join(dir, 'board.cjs');
  fs.writeFileSync(
    boardScript,
    `const cp = require('child_process'), fs = require('fs');
const c = cp.spawn(${JSON.stringify(claudeExe)}, ['-e', 'setTimeout(()=>{}, 60000)'], { stdio: 'ignore' });
fs.writeFileSync(${JSON.stringify(childPidFile)}, String(c.pid));
setTimeout(() => {}, 60000);`,
  );
  const boardProcess = cp.spawn(board, [boardScript], { stdio: 'ignore', windowsHide: true });
  const deadline = Date.now() + 8000;
  while (!fs.existsSync(childPidFile)) {
    if (Date.now() > deadline) throw new Error('자식이 안 떴다');
    await sleep(30);
  }
  await sleep(300);
  return { boardPid: boardProcess.pid, childPid: Number(fs.readFileSync(childPidFile, 'utf8')) };
}

test('보드 자손 claude.exe 가 허용 폴더 밖이면 1(REAL-CLAUDE), 허용 폴더 안(가짜)이면 0', { skip: !HAS_PWSH || process.platform !== 'win32' }, async () => {
  const f = fixture();
  const fakeDir = path.join(f.dir, 'fake');
  const realDir = path.join(f.dir, 'realish');
  const started = [];
  try {
    assert.equal(run('pwsh', ['-Phase', 'before', ...f.watch]).code, 0);

    const real = await startBoardWith(f.dir, nodeCopy(path.join(realDir, 'claude.exe')));
    started.push(real.boardPid);
    const bad = run('pwsh', ['-Phase', 'check', '-State', f.state, '-BoardPid', String(real.boardPid), '-FakeDir', fakeDir]);
    assert.equal(bad.code, 1, bad.text);
    assert.match(bad.text, new RegExp(`TRIPWIRE\\|REAL-CLAUDE\\|${real.childPid}\\|`));
    // 보드와 이어지지 않는 같은 프로세스는 참고(OTHER-CLAUDE)일 뿐 실패가 아니다
    const unrelated = run('pwsh', ['-Phase', 'check', '-State', f.state, '-BoardPid', '999999', '-FakeDir', fakeDir]);
    assert.equal(unrelated.code, 0, unrelated.text);
    assert.match(unrelated.text, new RegExp(`TRIPWIRE\\|OTHER-CLAUDE\\|${real.childPid}\\|`));
    killTree(real.boardPid);

    fs.rmSync(path.join(f.dir, 'child.pid'));
    const fake = await startBoardWith(f.dir, nodeCopy(path.join(fakeDir, 'fake-bin', 'claude.exe')));
    started.push(fake.boardPid);
    const good = run('pwsh', ['-Phase', 'check', '-State', f.state, '-BoardPid', String(fake.boardPid), '-FakeDir', fakeDir]);
    assert.equal(good.code, 0, good.text);
    assert.doesNotMatch(good.text, new RegExp(`\\|${fake.childPid}\\|`), '허용 폴더 안은 목록에도 없다');
    // 허용 폴더 이름이 접두어만 같은 형제(fake2)는 허용이 아니다
    const sibling = run('pwsh', ['-Phase', 'check', '-State', f.state, '-BoardPid', String(fake.boardPid), '-FakeDir', path.join(f.dir, 'fak')]);
    assert.equal(sibling.code, 1, sibling.text);
  } finally {
    for (const pid of started) killTree(pid);
    await sleep(300);
    f.cleanup();
  }
});
