// Judge 를 감싸 이미지를 먼저 검사하는 데코레이터. 통과하지 못한 이미지는 안쪽(캐시·하위 프로세스)에 닿지 않는다.
// 위반은 GuardError 를 던진다 — 호출한 쪽이 GUARD 로 보고한다. 판정으로 삼키지 않는다.
import { guardImage } from '../domain/guard.mjs';

export function guardedJudge(inner, { repo }) {
  const guard = (imagePath) => guardImage(imagePath, repo).path;
  return {
    check: (imagePath, statement, expect) => inner.check(guard(imagePath), statement, expect),
    checkAll: (imagePath, items) => inner.checkAll(guard(imagePath), items),
    screen: (imagePath, choices, expect) => inner.screen(guard(imagePath), choices, expect),
    selftest: inner.selftest ? () => inner.selftest() : undefined,
  };
}
