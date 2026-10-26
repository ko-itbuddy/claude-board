// @ts-nocheck — node:test 로만 돌고 svelte-check 대상이 아니다
// 실행: cd windows/app && node --test src/lib/board/opacity.test.ts   (Node 22.18+/24 는 .ts 를 바로 읽는다)
import { test } from "node:test";
import assert from "node:assert/strict";
import { boardOpaque } from "./opacity.ts";

// +page.svelte 의 옛 식: expanded || (!win.previewing && (pointer.inside || win.focused))
const oldRule = (expanded, previewing, inside, focused) => expanded || (!previewing && (inside || focused));

test("오늘의 규칙과 입력 열여섯 가지 모두 같다", () => {
  for (const expanded of [false, true])
    for (const previewing of [false, true])
      for (const inside of [false, true])
        for (const focused of [false, true]) {
          const got = boardOpaque({ covering: expanded, previewing, pointerInside: inside || focused });
          assert.equal(got, oldRule(expanded, previewing, inside, focused), JSON.stringify({ expanded, previewing, inside, focused }));
        }
});

test("덮고 있으면 미리 보는 중에도 불투명, 접힌 보드는 마우스가 밖이면 투명", () => {
  assert.equal(boardOpaque({ covering: true, previewing: true, pointerInside: false }), true);
  assert.equal(boardOpaque({ covering: false, previewing: true, pointerInside: true }), false);
  assert.equal(boardOpaque({ covering: false, previewing: false, pointerInside: false }), false);
  assert.equal(boardOpaque({ covering: false, previewing: false, pointerInside: true }), true);
});
