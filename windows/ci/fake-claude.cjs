'use strict';
// 가짜 Claude Code — CI 확인 스크립트(common.ps1 의 Start-FakeClaude)가 node.exe 를 claude.exe 라는 이름으로 복사해 이 파일을 돌린다.
// hook 은 조상 가운데 실행 파일 이름이 claude.exe 인 프로세스를 Claude Code 로 본다(crates/core/src/proc.rs 의 is_claude_exe).
//
//   claude.exe fake-claude.cjs <주고받기 폴더>
//
// 시작하면 <폴더>/ready.json 에 자기 pid 를 쓰고, <폴더>/cmd-<n>.json 을 0 번부터 차례로 기다려 읽는다.
// cmd 의 steps 마다 settings.json 에 등록된 hook 명령(command·args)을 셸 없이(spawnSync, shell:false) 표준입력 JSON 과 함께 실행하고,
// 결과(종료 코드·걸린 시간·출력)를 <폴더>/res-<n>.json 에 쓴다. <폴더>/stop 이 생기면 끝난다.
// 확인하는 동안 살아 있어야 세션 기록의 pid 가 산 프로세스로 보인다(보드가 죽은 pid 의 세션을 ended 로 바꾼다).
const fs = require('fs');
const path = require('path');
const cp = require('child_process');

const dir = process.argv[2];
if (!dir) {
  console.error('사용법: claude.exe fake-claude.cjs <주고받기 폴더>');
  process.exit(2);
}
const logFile = path.join(dir, 'fake-claude.log');

function log(msg) {
  try {
    fs.appendFileSync(logFile, `[${new Date().toISOString()}] ${msg}\n`);
  } catch (_) {
    // 기록 실패는 무시한다
  }
}

// 읽는 쪽(PowerShell)이 반쯤 쓴 파일을 읽지 않게 임시 파일에 쓰고 이름을 바꾼다
function writeJson(name, value) {
  const tmp = path.join(dir, `${name}.tmp`);
  fs.writeFileSync(tmp, JSON.stringify(value, null, 2));
  fs.renameSync(tmp, path.join(dir, name));
}

// hook 하나 — Claude Code 처럼 작업 폴더는 프로젝트, CLAUDE_PROJECT_DIR 을 붙이고, 시간 제한은 등록된 timeout
function runStep(step) {
  const started = Date.now();
  const out = { label: step.label, command: step.command, args: step.args, timeoutMs: step.timeoutMs };
  try {
    const r = cp.spawnSync(step.command, step.args, {
      input: step.input,
      cwd: step.cwd,
      timeout: step.timeoutMs,
      windowsHide: true,
      shell: false,
      encoding: 'utf8',
      env: Object.assign({}, process.env, { CLAUDE_PROJECT_DIR: step.cwd }),
    });
    out.status = r.status;
    out.signal = r.signal;
    out.error = r.error ? String(r.error.code || r.error.message) : null;
    out.stdout = r.stdout;
    out.stderr = r.stderr;
  } catch (e) {
    out.status = null;
    out.error = String((e && e.message) || e);
  }
  out.ms = Date.now() - started;
  log(`${step.label}: status=${out.status} signal=${out.signal} error=${out.error} ms=${out.ms}`);
  return out;
}

let next = 0;
let busy = false;

writeJson('ready.json', { pid: process.pid, ppid: process.ppid, execPath: process.execPath, argv: process.argv });
log(`시작 pid=${process.pid} execPath=${process.execPath}`);

const timer = setInterval(() => {
  if (busy) return;
  if (fs.existsSync(path.join(dir, 'stop'))) {
    log('stop 파일 — 끝낸다');
    clearInterval(timer);
    process.exit(0);
  }
  const file = path.join(dir, `cmd-${next}.json`);
  if (!fs.existsSync(file)) return;
  busy = true;
  const res = { results: [] };
  try {
    const cmd = JSON.parse(fs.readFileSync(file, 'utf8'));
    for (const step of cmd.steps || []) res.results.push(runStep(step));
    // Node 는 Windows 에서 process.title 을 처음 읽을 때 콘솔 제목(GetConsoleTitleW)을 읽고 그 뒤로는 기억한 값을 준다 — 한 번만 쓸모 있다
    if (cmd.readTitle) res.title = process.title;
  } catch (e) {
    res.error = String((e && e.stack) || e);
    log(`cmd-${next} 처리 실패: ${res.error}`);
  }
  writeJson(`res-${next}.json`, res);
  next += 1;
  busy = false;
}, 100);

// 확인 스크립트가 끝내지 못해도 CI 작업보다 오래 남지 않게 30분 뒤 스스로 끝난다
setTimeout(() => {
  log('30분이 지나 끝낸다');
  process.exit(0);
}, 30 * 60 * 1000);
