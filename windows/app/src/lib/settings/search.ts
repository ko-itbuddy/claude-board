// 설정 항목 검색 — 제목과 키워드에서 대소문자를 가리지 않고 찾는다(mac `SettingsItem` 검색).
//
// TODO(P2.1): 본문. 지금은 늘 빈 목록을 돌려준다(아무도 부르지 않는다).
import type { SettingsItem } from "./items";

/** query 가 제목이나 키워드에 든 항목, items 의 순서 그대로. 빈 검색어는 빈 목록 */
export function search<T extends Pick<SettingsItem, "title" | "keywords">>(items: readonly T[], query: string): T[] {
  void items;
  void query;
  return [];
}
