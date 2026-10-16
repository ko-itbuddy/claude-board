// 개발할 때만 여는 캐릭터 확인 화면 — 배포 빌드에서는 없는 페이지로 둔다
import { dev } from "$app/environment";
import { error } from "@sveltejs/kit";

export const load = () => {
  if (!dev) error(404);
};
