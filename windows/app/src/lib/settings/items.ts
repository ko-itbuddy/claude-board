// 설정 화면의 항목 표(mac `SettingsItem`) — 검색(search.ts)이 이 표의 제목·키워드로 모든 묶음에서 찾고, 묶음 화면이 같은 표로 줄을 그린다.
//
// TODO(P2.1): 2단계 항목 행. 3단계(P3.0)가 글자 크기·테마·애니메이션 행을 더한다.
import type { SettingsSectionId } from "$lib/types";

export interface SettingsItem {
  /** 항목 하나의 안정 식별자 — 검색 결과 이동과 시험이 가리킨다 */
  id: string;
  section: SettingsSectionId;
  title: string;
  /** 제목에 없는 검색어(동의어·영문) */
  keywords: string[];
}

export const SETTINGS_ITEMS: readonly SettingsItem[] = [];
