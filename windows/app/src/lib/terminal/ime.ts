// 한글 IME 키 처리기(xterm.js #6065). WebView2 에서 IME 가 켜져 있으면 일부 키가 keyCode 229 로만 와서
// xterm 이 알아보지 못하거나, 조합이 끝나기 전에 눌린 Enter·방향키가 사라진다. xterm 소스는 고치지 않고
// 키 처리기 앞단에서 아래 두 가지만 바로잡는다.
//  1) 조합 중이 아닌데 229 로 온 Escape·Ctrl 조합은 ev.code 로 읽어 바로 보낸다
//  2) 조합 중에 눌린 Enter·Escape·방향키·Home/End·Ctrl 조합은 붙들었다가 compositionend 다음 틱에 보낸다

export interface ImeKeys {
  /** 키 이벤트를 먼저 거른다. false 면 이쪽에서 처리를 끝냈으니 xterm 이 다루지 않는다 */
  filter(ev: KeyboardEvent): boolean;
  dispose(): void;
}

const MODIFIER_CODE = /^(Control|Shift|Alt|Meta)(Left|Right)$/;
const ARROW_FINAL: Record<string, string> = { ArrowUp: "A", ArrowDown: "B", ArrowRight: "C", ArrowLeft: "D", Home: "H", End: "F" };

const isEnter = (ev: KeyboardEvent) => ev.code === "Enter" || ev.code === "NumpadEnter";

/** Escape · Ctrl+A..Z · Ctrl+Space. 조합 중이 아니어도 229 로 올 수 있는 키 */
function controlKey(ev: KeyboardEvent): string | null {
  if (ev.code === "Escape") return "\x1b";
  if (!ev.ctrlKey || ev.altKey || ev.metaKey) return null;
  if (ev.code === "Space") return "\x00";
  const letter = /^Key([A-Z])$/.exec(ev.code);
  return letter ? String.fromCharCode(letter[1].charCodeAt(0) - 64) : null;
}

/** 조합 중에 붙들 키를 셸로 보낼 문자열로 바꾼다. 붙들지 않을 키는 null */
function heldKey(ev: KeyboardEvent, applicationCursor: boolean): string | null {
  if (MODIFIER_CODE.test(ev.code)) return null;
  if (isEnter(ev)) return ev.shiftKey ? "\n" : "\r";
  const control = controlKey(ev);
  if (control !== null) return control;
  const final = ARROW_FINAL[ev.code];
  if (!final) return null;
  const modifier = 1 + (ev.shiftKey ? 1 : 0) + (ev.altKey ? 2 : 0) + (ev.ctrlKey ? 4 : 0);
  if (modifier > 1) return `\x1b[1;${modifier}${final}`;
  return `${applicationCursor ? "\x1bO" : "\x1b["}${final}`;
}

export function createImeKeys(
  textarea: HTMLTextAreaElement,
  send: (data: string) => void,
  applicationCursor: () => boolean,
): ImeKeys {
  let composing = false;
  // 붙들어 둔 키. 각각 (Enter 인지, 보낼 문자열)
  let held: { enter: boolean; data: string }[] = [];
  let flushTimer: ReturnType<typeof setTimeout> | undefined;

  const flush = () => {
    clearTimeout(flushTimer);
    const keys = held;
    held = [];
    for (const key of keys) send(key.data);
  };
  const start = () => {
    composing = true;
    // 다음 조합이 바로 시작되면 앞서 붙들린 키부터 보낸다
    flush();
  };
  const end = () => {
    composing = false;
    clearTimeout(flushTimer);
    // 같은 틱에 다시 쏘아지는 키가 있으면 거를 수 있게 한 틱 뒤에 보낸다
    flushTimer = setTimeout(flush, 0);
  };
  textarea.addEventListener("compositionstart", start);
  textarea.addEventListener("compositionend", end);

  return {
    filter(ev) {
      if (ev.type !== "keydown") return true;
      const duringComposition = composing || ev.isComposing;
      if (duringComposition) {
        const data = heldKey(ev, applicationCursor());
        if (data === null) return true;
        ev.preventDefault();
        // 이미 Enter 가 대기 중이면 다시 쏘아진 Enter 는 버린다
        if (!(isEnter(ev) && held.some((key) => key.enter))) held.push({ enter: isEnter(ev), data });
        return false;
      }
      if (held.length > 0) {
        // compositionend 직후 같은 틱에 입력기가 다시 쏘는 같은 키(Enter, Escape, 방향키 ...)는 이미 붙들어 둔 것과 같으므로 버린다.
        // 조합 중 Escape 는 한 번인데 ESC 가 두 번 셸에 가면 PSReadLine 이 줄을 지우지 않고 ^[ 를 글자로 넣는다
        const again = heldKey(ev, applicationCursor());
        if ((isEnter(ev) && held.some((key) => key.enter)) || (again !== null && held.some((key) => key.data === again))) {
          ev.preventDefault();
          return false;
        }
      }
      if (ev.keyCode === 229) {
        const data = controlKey(ev);
        if (data === null) return true;
        ev.preventDefault();
        send(data);
        return false;
      }
      return true;
    },
    dispose() {
      textarea.removeEventListener("compositionstart", start);
      textarea.removeEventListener("compositionend", end);
      clearTimeout(flushTimer);
    },
  };
}
