// 세션 줄이 부르는 동작 — 메뉴 항목과 클릭이 내는 동작 이름을 한 곳에서 정한다. 이름은 mac performAction 의 어휘
// (pause·resume·stop·reopen·focus)와 Windows 전용 activate 뿐이다(Rust session_action.rs 의 SessionAction::parse 와 같은 집합).
// 외부를 import 하지 않아(타입만) node:test 로 시험한다.
import type { Session, SessionAction } from "../types";

export interface SessionCall {
  action: SessionAction;
  /** reopen 에만 쓴다 — true 면 "이어서 진행" 문구를 첫 요청으로 넘긴다 */
  carryOn?: boolean;
}

export type SessionMenuEntry = { separator: true } | ({ label: string; enabled?: boolean } & SessionCall);

type MenuSession = Pick<Session, "state" | "paused" | "canDrive">;

// 끝난 세션은 이어 열기, 일시중지는 재개·중단, 그 밖은 일시중지·중단.
// reopen 의 carryOn 은 아직 mac 처럼 s.paused 로 정하지 않고 false 로 고정한다(문구 없이 연다)
export function sessionMenuEntries(s: MenuSession): SessionMenuEntry[] {
  if (s.state === "ended") {
    return [{ label: s.paused ? "세션 재개 (이어서 진행)" : "세션 다시 열기", action: "reopen", carryOn: false }];
  }
  if (s.state === "paused") {
    return [{ label: "세션 재개", action: "resume" }, { separator: true }, { label: "세션 중단", action: "stop" }];
  }
  return [
    { label: "일시중지", enabled: s.state !== "idle" && s.canDrive, action: "pause" },
    { separator: true },
    { label: "세션 중단", action: "stop" },
  ];
}

// 한 번 클릭 = 그 세션의 터미널 앱만 앞으로, 두 번 클릭 = 종료 세션은 다시 열기, 아니면 그 세션 창으로 이동
export function rowClickCall(s: Pick<Session, "state">, detail: number): SessionCall {
  if (detail < 2) return { action: "activate" };
  return s.state === "ended" ? { action: "reopen", carryOn: false } : { action: "focus" };
}
