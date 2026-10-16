//! 콘솔 도구 (Windows) — 다른 프로세스의 콘솔에 붙어 제목 바꾸기·입력 넣기·터미널 창 찾기
//! (SPEC §3.2 제목, §3.4 headless, §5.7 터미널 제어). Windows 실기 확인은 `tests/windows_console.rs`(CI 의 windows-latest 러너).
//!
//! 한 프로세스는 콘솔 하나에만 붙을 수 있으므로 모든 함수가 전역 잠금 아래에서
//! `FreeConsole → AttachConsole(pid) → 일 → FreeConsole` 로 붙었다 뗀다(한 번에 한 세션씩).
//! 붙어 있는 동안은 Ctrl 처리 함수를 등록해 그 콘솔의 Ctrl+C·Ctrl+Break 가 우리 프로세스를 끝내지 않게 한다
//! (`SetConsoleCtrlHandler(NULL, TRUE)` 는 그사이 띄운 자식 프로세스에 상속되고 Ctrl+Break 는 막지 못해 쓰지 않는다).
//! `AttachConsole` 은 표준 핸들을 그 콘솔 핸들로 바꾸고 `FreeConsole` 은 그 핸들을 닫기만 하므로, 뗀 뒤 원래 표준 핸들을
//! 되돌린다 — 그대로 두면 보드가 나중에 띄우는 프로세스가 닫힌 핸들을 물려받으려다 실패한다.
//! Windows 가 아닌 곳에서는 아무것도 하지 않고 false/None 을 돌려준다.

use std::time::Duration;

/// 콘솔에 넣을 입력 하나.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input<'a> {
    /// Esc 키(VK_ESCAPE, 0x1B)
    Escape,
    /// Enter 키(VK_RETURN, `\r`)
    Enter,
    /// 글자들 — UTF-16 단위마다 가상 키 0 에 그 문자
    Text(&'a str),
    /// 붙은 채로 잠깐 기다린다(예: Esc 뒤 0.5초)
    Pause(Duration),
}

/// 그 프로세스의 콘솔 제목을 바꾼다.
pub fn set_title(pid: u32, title: &str) -> bool {
    sys::set_title(pid, title)
}

/// 그 프로세스의 콘솔 제목(Windows Terminal 탭 찾기에서 제목을 잠깐 바꿨다 되돌릴 때).
pub fn title(pid: u32) -> Option<String> {
    sys::title(pid)
}

/// 그 프로세스의 콘솔에 붙을 수 있는지 — 보드의 `canDrive` 조건 하나.
pub fn can_attach(pid: u32) -> bool {
    sys::can_attach(pid)
}

/// 그 세션의 터미널 창(HWND 값). 콘솔 창이 보이면 그 창, 아니면 그 창의 소유 창(ConPTY 는 터미널 창을 소유자로 둔다),
/// 아니면 가장 가까운 조상 프로세스의 보이는 최상위 창(`explorer.exe` 에서 멈춘다). 콘솔에 붙지 못하면 None.
pub fn terminal_window(pid: u32) -> Option<isize> {
    sys::terminal_window(pid)
}

/// headless 판별(SPEC §3.4) — 실행 인자에 `-p`/`--print` 가 있거나, 콘솔에 붙지 못하거나, 보이는 터미널 창이 없으면 참.
pub fn is_headless(pid: u32, argv: &[String]) -> bool {
    print_mode(argv) || sys::terminal_window(pid).is_none()
}

/// 실행 인자(argv[0] 제외)에 `-p`/`--print` 가 있는지.
pub fn print_mode(argv: &[String]) -> bool {
    argv.iter().skip(1).any(|a| a == "-p" || a == "--print")
}

/// 그 프로세스의 콘솔 입력 버퍼에 키 입력을 차례로 넣는다(`WriteConsoleInputW`, 누름·뗌 한 쌍씩).
/// 예: 일시중지 `[Escape]`, 자동 전환 이어 가기 `[Escape, Pause(0.5s), Text(문구), Pause(0.2s), Enter]`.
pub fn send_input(pid: u32, inputs: &[Input]) -> bool {
    sys::send_input(pid, inputs)
}

/// 콘솔 키 입력 하나(`KEY_EVENT_RECORD` 의 값들). 반복 수는 늘 1.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyEvent {
    vk: u16,
    scan: u16,
    ch: u16,
    down: bool,
}

/// 입력 하나를 누름·뗌 쌍의 키 이벤트로 바꾼다. Esc = VK_ESCAPE(스캔 0x01, 문자 0x1B), Enter = VK_RETURN(스캔 0x1C, `\r`),
/// 글자는 UTF-16 단위마다 가상 키 0 — 서로게이트 쌍은 높은 쪽·낮은 쪽 순서로 각각 누름·뗌(libuv 가 둘을 이어 붙인다).
#[cfg_attr(not(windows), allow(dead_code))]
fn key_events(input: &Input) -> Vec<KeyEvent> {
    let press = |vk, scan, ch| [KeyEvent { vk, scan, ch, down: true }, KeyEvent { vk, scan, ch, down: false }];
    match input {
        Input::Escape => press(0x1B, 0x01, 0x1B).to_vec(),
        Input::Enter => press(0x0D, 0x1C, u16::from(b'\r')).to_vec(),
        Input::Text(s) => s.encode_utf16().flat_map(|u| press(0, 0, u)).collect(),
        Input::Pause(_) => Vec::new(),
    }
}

#[cfg(windows)]
mod sys {
    use super::{key_events, Input, KeyEvent};
    use std::sync::Mutex;
    use windows_sys::core::BOOL;
    use windows_sys::Win32::Foundation::{
        CloseHandle, FALSE, GENERIC_READ, GENERIC_WRITE, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM, TRUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::Console::{
        AttachConsole, FreeConsole, GetConsoleTitleW, GetConsoleWindow, GetStdHandle, SetConsoleCtrlHandler,
        SetConsoleTitleW, SetStdHandle, WriteConsoleInputW, CTRL_BREAK_EVENT, CTRL_C_EVENT, INPUT_RECORD,
        INPUT_RECORD_0, KEY_EVENT, KEY_EVENT_RECORD, KEY_EVENT_RECORD_0, STD_ERROR_HANDLE, STD_HANDLE,
        STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    static LOCK: Mutex<()> = Mutex::new(());

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    const STD: [STD_HANDLE; 3] = [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE];

    /// 붙어 있는 동안의 Ctrl+C·Ctrl+Break 는 처리한 것으로 하고 무시한다. 창 닫기·로그오프는 기본 처리에 맡긴다.
    unsafe extern "system" fn ignore_ctrl(ctrl: u32) -> BOOL {
        if ctrl == CTRL_C_EVENT || ctrl == CTRL_BREAK_EVENT {
            TRUE
        } else {
            FALSE
        }
    }

    /// 붙은 상태 — 놓을 때(패닉 포함) 콘솔에서 떼고 붙기 전 표준 핸들을 되돌린다.
    struct Attached {
        saved: [HANDLE; 3],
    }

    impl Drop for Attached {
        fn drop(&mut self) {
            // SAFETY: 우리가 등록한 처리 함수를 빼고 콘솔에서 뗀 뒤, 붙기 전에 읽어 둔 표준 핸들 값을 다시 넣는다.
            // FreeConsole 이 AttachConsole 이 연 핸들을 닫으므로 그 핸들은 직접 닫지 않는다(두 번 닫힘 방지)
            unsafe {
                SetConsoleCtrlHandler(Some(ignore_ctrl), FALSE);
                FreeConsole();
                for (which, h) in STD.iter().zip(self.saved) {
                    SetStdHandle(*which, h);
                }
            }
        }
    }

    /// 그 pid 의 콘솔에 붙은 동안 `f` 를 실행한다. 붙지 못하면 None.
    fn with_console<T>(pid: u32, f: impl FnOnce() -> T) -> Option<T> {
        let _lock = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: 표준 핸들 값 읽기, 콘솔 붙이기·떼기, Ctrl 처리 함수 등록만 한다(처리 함수 목록은 붙일 때 초기화되므로 붙인 뒤 등록)
        let attached = unsafe {
            let saved = STD.map(|which| GetStdHandle(which));
            FreeConsole();
            let ok = AttachConsole(pid) != 0;
            let attached = Attached { saved };
            if !ok {
                return None;
            }
            SetConsoleCtrlHandler(Some(ignore_ctrl), TRUE);
            attached
        };
        let out = f();
        drop(attached);
        Some(out)
    }

    pub fn set_title(pid: u32, title: &str) -> bool {
        let w = wide(title);
        // SAFETY: NUL 로 끝나는 문자열
        with_console(pid, || unsafe { SetConsoleTitleW(w.as_ptr()) != 0 }).unwrap_or(false)
    }

    pub fn title(pid: u32) -> Option<String> {
        with_console(pid, || {
            let mut buf = vec![0u16; 4096];
            // SAFETY: 버퍼 크기를 정확히 넘긴다
            let n = unsafe { GetConsoleTitleW(buf.as_mut_ptr(), buf.len() as u32) } as usize;
            // 0 = 실패이거나 빈 제목 — 되돌릴 값으로 쓰지 않는다(부르는 쪽이 프로젝트 이름으로 되돌린다)
            (n > 0).then(|| String::from_utf16_lossy(&buf[..n.min(buf.len())]))
        })
        .flatten()
    }

    pub fn can_attach(pid: u32) -> bool {
        with_console(pid, || ()).is_some()
    }

    fn visible(h: HWND) -> bool {
        // SAFETY: 창 핸들 조회만 한다
        !h.is_null() && unsafe { IsWindowVisible(h) } != 0
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: EnumWindows 에 넘긴 Vec 의 포인터
        let list = unsafe { &mut *(lparam as *mut Vec<(u32, isize)>) };
        if visible(hwnd) && unsafe { GetWindow(hwnd, GW_OWNER) }.is_null() {
            let mut pid = 0u32;
            unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
            list.push((pid, hwnd as isize));
        }
        TRUE
    }

    /// 보이는 소유자 없는 최상위 창들(pid, HWND).
    fn top_windows() -> Vec<(u32, isize)> {
        let mut list: Vec<(u32, isize)> = Vec::new();
        // SAFETY: 콜백은 EnumWindows 안에서만 list 를 쓴다
        unsafe { EnumWindows(Some(collect), &mut list as *mut _ as LPARAM) };
        list
    }

    pub fn terminal_window(pid: u32) -> Option<isize> {
        // SAFETY: 붙은 동안 콘솔 창 핸들만 읽는다
        let console = with_console(pid, || unsafe { GetConsoleWindow() } as isize)?;
        let hwnd = console as HWND;
        // 소유 창을 먼저 본다 — ConPTY 의 가짜 창은 터미널 창을 소유자로 두고, 터미널이 그 창의 보이기 상태를
        // 따라 맞추기도 하므로 가짜 창이 보인다고 그것을 고르면 앞으로 가져오기·탭 찾기가 터미널 창에 닿지 않는다.
        // 일반 conhost 창은 소유자가 없어 다음 단계에서 그 창을 고른다
        if !hwnd.is_null() {
            // SAFETY: 창 핸들 조회
            let owner = unsafe { GetWindow(hwnd, GW_OWNER) };
            if visible(owner) {
                return Some(owner as isize);
            }
        }
        if visible(hwnd) {
            return Some(console);
        }
        // 조상 가운데 보이는 최상위 창 — explorer.exe 는 늘 창이 있어 판별에 쓸 수 없으므로 거기서 멈춘다
        let table = crate::proc::ProcessTable::capture();
        let windows = top_windows();
        for anc in table.ancestors(pid, 10) {
            if table.name(anc).is_some_and(|n| n.eq_ignore_ascii_case("explorer.exe")) {
                break;
            }
            if let Some(&(_, w)) = windows.iter().find(|(p, _)| *p == anc) {
                return Some(w);
            }
        }
        None
    }

    fn record(k: KeyEvent) -> INPUT_RECORD {
        INPUT_RECORD {
            EventType: KEY_EVENT as u16,
            Event: INPUT_RECORD_0 {
                KeyEvent: KEY_EVENT_RECORD {
                    bKeyDown: if k.down { TRUE } else { FALSE },
                    wRepeatCount: 1,
                    wVirtualKeyCode: k.vk,
                    wVirtualScanCode: k.scan,
                    uChar: KEY_EVENT_RECORD_0 { UnicodeChar: k.ch },
                    dwControlKeyState: 0,
                },
            },
        }
    }

    fn records(input: &Input) -> Vec<INPUT_RECORD> {
        key_events(input).into_iter().map(record).collect()
    }

    pub fn send_input(pid: u32, inputs: &[Input]) -> bool {
        with_console(pid, || {
            let name = wide("CONIN$");
            // SAFETY: 붙은 콘솔의 입력 버퍼를 열고 닫는다
            let h = unsafe {
                CreateFileW(
                    name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            if h == INVALID_HANDLE_VALUE || h.is_null() {
                return false;
            }
            let mut ok = true;
            for input in inputs {
                if let Input::Pause(d) = input {
                    std::thread::sleep(*d);
                    continue;
                }
                let recs = records(input);
                let mut done = 0usize;
                while ok && done < recs.len() {
                    let mut written = 0u32;
                    // SAFETY: 남은 레코드 수를 정확히 넘긴다
                    let r = unsafe {
                        WriteConsoleInputW(h, recs[done..].as_ptr(), (recs.len() - done) as u32, &mut written)
                    };
                    ok = r != 0 && written > 0;
                    done += written as usize;
                }
                // 못 넣었으면 남은 입력(Enter 등)과 기다림을 건너뛴다
                if !ok {
                    break;
                }
            }
            // SAFETY: 위에서 연 CONIN$ 핸들을 한 번 닫는다
            unsafe { CloseHandle(h) };
            ok
        })
        .unwrap_or(false)
    }
}

#[cfg(not(windows))]
mod sys {
    use super::Input;

    pub fn set_title(_pid: u32, _title: &str) -> bool {
        false
    }
    pub fn title(_pid: u32) -> Option<String> {
        None
    }
    pub fn can_attach(_pid: u32) -> bool {
        false
    }
    pub fn terminal_window(_pid: u32) -> Option<isize> {
        None
    }
    pub fn send_input(_pid: u32, _inputs: &[Input]) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_mode_skips_argv0() {
        let v = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(print_mode(&v(&["claude", "-p", "hi"])));
        assert!(print_mode(&v(&["claude", "--print"])));
        assert!(!print_mode(&v(&["-p"])));
        assert!(!print_mode(&v(&["claude", "--permission-mode", "plan"])));
    }

    #[test]
    fn key_events_pairs() {
        let k = |vk, scan, ch, down| KeyEvent { vk, scan, ch, down };
        assert_eq!(key_events(&Input::Escape), [k(0x1B, 0x01, 0x1B, true), k(0x1B, 0x01, 0x1B, false)]);
        assert_eq!(key_events(&Input::Enter), [k(0x0D, 0x1C, 0x0D, true), k(0x0D, 0x1C, 0x0D, false)]);
        assert!(key_events(&Input::Pause(std::time::Duration::from_millis(1))).is_empty());
        // 한글은 UTF-16 한 단위, 이모지는 서로게이트 쌍 — 단위마다 누름·뗌, 높은 쪽이 먼저
        assert_eq!(key_events(&Input::Text("가")), [k(0, 0, 0xAC00, true), k(0, 0, 0xAC00, false)]);
        let e = key_events(&Input::Text("a😀"));
        let chars: Vec<(u16, bool)> = e.iter().map(|x| (x.ch, x.down)).collect();
        assert_eq!(chars, [(0x61, true), (0x61, false), (0xD83D, true), (0xD83D, false), (0xDE00, true), (0xDE00, false)]);
        assert!(e.iter().all(|x| x.vk == 0 && x.scan == 0));
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_is_noop() {
        assert!(!set_title(1, "x"));
        assert!(!send_input(1, &[Input::Escape]));
        assert!(!can_attach(1));
        assert_eq!(title(1), None);
    }
}
