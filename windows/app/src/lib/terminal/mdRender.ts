// 답변 꾸미기(md-render) 명령 규칙(mac mdRenderCommand·mdRenderOff) — 순수 함수다. node:test 로 바로 시험한다.
// TODO(P1.2): 지금은 계약만 있다(명령을 그대로 둔다). 규칙·표 시험은 P1.2 가 채운다 — 이미 든 `--plugin-dir <...\plugins\md-render>` 는 모두 걷어 내고,
// 켜져 있고 폴더를 알면 하나만 붙인다(공백·한글 경로 따옴표).

/** 명령의 `--plugin-dir` 을 정리한다 — 끄면(off) 없음, 켜고 pluginDir 을 알면 하나 */
export function mdRenderCommand(command: string, _pluginDir: string | null, _off: boolean): string {
  return command;
}

/** 이 세션이 꾸미기를 끈 채 떴는가 */
export function isMdRenderOff(_id: string, _list: string[]): boolean {
  return false;
}

/** 끈 세션 목록에 id 를 더한 새 목록 */
export function rememberMdRenderOff(list: string[], _id: string): string[] {
  return list;
}

/** 아직 있는 세션(liveIds)만 남긴 새 목록 */
export function pruneMdRenderOff(list: string[], _liveIds: string[]): string[] {
  return list;
}

/** 옮겨 둔 md-render 폴더 — 시작 때 한 번 받아 둔다. 모르면 null(플래그를 붙이지 않는다) */
export function currentPluginDir(): string | null {
  return null;
}
