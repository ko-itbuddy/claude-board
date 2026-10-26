// 설정 화면의 상태와 열고 닫기(mac `SettingsScreen`) — 터미널 영역(제목 띠 아래)을 설정 화면으로 덮는다.
// 접힌 보드에서 열면 터미널 칸을 펼치는 것이 전환이 되고 닫을 때 다시 접는다(펼침 설정에는 남기지 않는다). 열린 동안 탭은 뒤에서 그대로 돈다.
//
// TODO(P2.1): 본문 — open/close/toggle, Esc 차례(키 받기 취소 → 검색어 지우기 → 닫기), 접힌 칸 펼침·되접기, board://settings 듣기.
// 지금은 한 번도 열리지 않는다(shown·visible 은 늘 false) — 부르는 자리(hub·sessionFlow·dispatch·SettingsView)만 이어 두었다.
import type { SettingsSectionId } from "$lib/types";

export const SettingsScreen = $state({
  /** 설정 화면이 떠 있다 */
  shown: false,
  section: "general" as SettingsSectionId,
  query: "",
  /** 화면에 올라와 있다(내려가는 전환 중에도) — 아니면 내용을 그리지 않는다 */
  visible: false,
});

export interface CloseOptions {
  /** 탭을 다루는 동작으로 닫을 때 — 접힌 보드에서 잠깐 펼쳤어도 펼친 채로 남긴다 */
  keep?: boolean;
}

/** 설정 화면을 연다(section 이 있으면 그 묶음으로) */
export function openSettings(section?: SettingsSectionId): void {
  void section; // TODO(P2.1)
}

/** 설정 화면을 닫는다. 떠 있지 않으면 아무것도 하지 않는다 */
export function closeSettings(options: CloseOptions = {}): void {
  void options; // TODO(P2.1)
}

/** 떠 있으면 닫고 아니면 연다 */
export function toggleSettings(): void {
  // TODO(P2.1)
}

/** Esc — 키 받는 중이면 취소, 검색어가 있으면 지우고, 아니면 닫는다 */
export function escapeSettings(): void {
  // TODO(P2.1)
}

/** 설정 화면이 마운트될 때 한 번 — 백엔드의 board://settings 를 듣고, 듣기를 끄는 함수를 돌려준다 */
export function connectSettings(): () => void {
  // TODO(P2.1)
  return () => {};
}
