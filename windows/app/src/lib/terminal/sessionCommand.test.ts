// @ts-nocheck — node:test 로만 돌고 svelte-check 대상이 아니다
// 실행: cd windows/app && node --test src/lib/terminal/sessionCommand.test.ts   (Node 22.18+/24 는 .ts 를 바로 읽는다)
// 실제 cmd.exe 에 주는 시험은 sessionCommand.cmd.test.ts 에 있다.
import { test } from "node:test";
import assert from "node:assert/strict";
import { CARRY_ON_PROMPT, freshSessionCommand, namedCommand, resumeCommand } from "./sessionCommand.ts";
import { sessionNameArg } from "./sessionName.ts";
import { hostileFolders, newSession } from "./sessionCommand.fixtures.ts";

const UUID = "[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}";
const NAME = String.raw`("[\p{L}\p{N} _.-]+"|[\p{L}\p{N}_.-]+)`;
const SHAPE = new RegExp(`^claude --name ${NAME} --dangerously-skip-permissions --session-id ${UUID}$`, "u");

test("새 세션 줄 모양 — 이름은 claude 바로 뒤, 공백이 있으면 큰따옴표", () => {
  const { line, sessionId } = newSession("shop-api");
  assert.equal(line, `claude --name shop-api --dangerously-skip-permissions --session-id ${sessionId}`);
  assert.equal(newSession("my app").line.split(" --dangerously")[0], 'claude --name "my app"');
  assert.equal(newSession("내 프로젝트").line.split(" --dangerously")[0], 'claude --name "내 프로젝트"');
});

test("어떤 폴더 이름이든 줄은 한 모양 — 이름 밖에는 셸 특수문자가 없다", () => {
  for (const folder of hostileFolders) assert.match(newSession(folder).line, SHAPE, JSON.stringify(folder));
});

test("이어 열기·이어 하기는 세션 id 를 붙이지 않고, --name 이 있으면 이름도 붙이지 않는다", () => {
  const san = (command) => namedCommand(freshSessionCommand(command).command, sessionNameArg("proj"));
  assert.equal(san("claude --resume abc"), "claude --name proj --resume abc");
  assert.equal(san("claude -r abc --model x"), "claude --name proj -r abc --model x");
  assert.equal(san("claude --resume=abc"), "claude --name proj --resume=abc");
  assert.equal(san("claude --continue"), "claude --name proj --continue");
  assert.equal(san("claude --name foo --resume x"), "claude --name foo --resume x");
  assert.equal(san("claude-x --foo"), "claude-x --foo");
});

test("실어 보내는 인자는 그대로 두고 옛 --session-id 만 새 id 로 바꾼다", () => {
  const line = namedCommand(freshSessionCommand("claude --session-id old-id --model claude-opus-5-5").command, sessionNameArg("proj"));
  assert.match(line, new RegExp(`^claude --name proj --model claude-opus-5-5 --session-id ${UUID}$`));
});

test("namedCommand 는 정리하지 않은 이름을 명령줄에 넣지 않는다", () => {
  for (const raw of ["a & calc", 'a"b', "%PATH%", "a^b", "-n", "", "a\r\nb"]) {
    assert.throws(() => namedCommand("claude --model m", raw), raw);
  }
  assert.equal(namedCommand("notclaude --x", "a & calc"), "notclaude --x");
});

const ID = "8f3c2a10-1111-4222-8333-444455556666";

test("resumeCommand — 기본은 인자 없이 claude --resume <id>", () => {
  assert.equal(resumeCommand([], ID, false), `claude --resume ${ID}`);
});

test("resumeCommand — 허용 글자만 든 인자는 그대로 남긴다", () => {
  assert.equal(
    resumeCommand(["--model", "claude-opus-5-5", "--dangerously-skip-permissions"], ID, false),
    `claude --resume ${ID} --model claude-opus-5-5 --dangerously-skip-permissions`,
  );
  assert.equal(resumeCommand(["--permission-mode=plan"], ID, false), `claude --resume ${ID} --permission-mode=plan`);
});

test("resumeCommand — -p·--print 는 뺀다", () => {
  assert.equal(resumeCommand(["-p", "--model", "m"], ID, false), `claude --resume ${ID} --model m`);
  assert.equal(resumeCommand(["--print"], ID, false), `claude --resume ${ID}`);
});

test("resumeCommand — 허용 밖 글자가 든 인자가 하나라도 있으면 인자를 모두 버린다", () => {
  for (const bad of ["a b", "a&b", "100%", 'a"b', "$HOME", "a;b", "a`b", "한글", "a^b", "a!b", "", "a\nb"]) {
    assert.equal(resumeCommand(["--model", "ok", bad], ID, false), `claude --resume ${ID}`, JSON.stringify(bad));
  }
});

test("resumeCommand — 이어 열기와 부딪히는 인자(--resume·--name 따위)도 기본 줄로 돌아간다", () => {
  for (const flag of ["--resume", "-r", "--continue", "-c", "--session-id", "--name", "-n", "--name=x", "--resume=abc"]) {
    assert.equal(resumeCommand([flag, "--model", "m"], ID, false), `claude --resume ${ID}`, flag);
  }
});

test("resumeCommand — 세션 id 모양이 틀리면 던진다", () => {
  for (const bad of ["", "short", "a b c d e f g h", "abc&def-ghi", `${ID}x"`, "x".repeat(65), "한글한글한글한글한글"]) {
    assert.throws(() => resumeCommand([], bad, false), JSON.stringify(bad));
  }
});

test("resumeCommand — carryOn 이면 큰따옴표로 묶은 재개 문구를 뒤에 붙인다", () => {
  assert.equal(resumeCommand([], ID, true), `claude --resume ${ID} "${CARRY_ON_PROMPT}"`);
  assert.equal(resumeCommand(["--model", "m"], ID, true), `claude --resume ${ID} --model m "${CARRY_ON_PROMPT}"`);
});

test("CARRY_ON_PROMPT — 큰따옴표 안에서 풀리는 셸 특수문자가 없다", () => {
  assert.doesNotMatch(CARRY_ON_PROMPT, /["$%!`^&\\\r\n]/);
  assert.ok(CARRY_ON_PROMPT.length > 0);
});

test("resumeCommand 결과에 namedCommand 가 --name 을 claude 바로 뒤에 붙인다", () => {
  assert.equal(namedCommand(resumeCommand([], ID, false), sessionNameArg("proj")), `claude --name proj --resume ${ID}`);
});
