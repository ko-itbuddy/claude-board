//! 콘솔 도구 (Windows) — 다른 프로세스의 콘솔에 붙어 제목 바꾸기·입력 넣기·터미널 창 찾기
//! (SPEC §3.2 제목, §3.4 headless, §5.7 터미널 제어). **모두 Windows 실기 미확인.**
//!
//! 한 프로세스는 콘솔 하나에만 붙을 수 있으므로 모든 함수가 전역 잠금 아래에서
//! `FreeConsole → AttachConsole(pid) → 일 → FreeConsole` 로 붙었다 뗀다(한 번에 한 세션씩).
//! 붙어 있는 동안은 `SetConsoleCtrlHandler(NULL, TRUE)` 로 그 콘솔의 Ctrl+C 가 우리 프로세스를 끝내지 않게 한다.
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

#[cfg(windows)]
mod sys {
    use super::Input;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{
        CloseHandle, FALSE, GENERIC_READ, GENERIC_WRITE, HWND, INVALID_HANDLE_VALUE, LPARAM, TRUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::Console::{
        AttachConsole, FreeConsole, GetConsoleTitleW, GetConsoleWindow, SetConsoleCtrlHandler, SetConsoleTitleW,
        WriteConsoleInputW, INPUT_RECORD, INPUT_RECORD_0, KEY_EVENT, KEY_EVENT_RECORD, KEY_EVENT_RECORD_0,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsWindowVisible, GW_OWNER,
    };

    static LOCK: Mutex<()> = Mutex::new(());

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// 그 pid 의 콘솔에 붙은 동안 `f` 를 실행한다. 붙지 못하면 None.
    fn with_console<T>(pid: u32, f: impl FnOnce() -> T) -> Option<T> {
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: 콘솔 붙이기·떼기와 Ctrl 처리 설정만 한다
        unsafe {
            FreeConsole();
            if AttachConsole(pid) == 0 {
                return None;
            }
            SetConsoleCtrlHandler(None, TRUE);
        }
        let out = f();
        unsafe {
            SetConsoleCtrlHandler(None, FALSE);
            FreeConsole();
        }
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
            String::from_utf16_lossy(&buf[..n.min(buf.len())])
        })
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
        if visible(hwnd) {
            return Some(console);
        }
        if !hwnd.is_null() {
            // SAFETY: 창 핸들 조회
            let owner = unsafe { GetWindow(hwnd, GW_OWNER) };
            if visible(owner) {
                return Some(owner as isize);
            }
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

    fn key(vk: u16, scan: u16, ch: u16, down: bool) -> INPUT_RECORD {
        INPUT_RECORD {
            EventType: KEY_EVENT as u16,
            Event: INPUT_RECORD_0 {
                KeyEvent: KEY_EVENT_RECORD {
                    bKeyDown: if down { TRUE } else { FALSE },
                    wRepeatCount: 1,
                    wVirtualKeyCode: vk,
                    wVirtualScanCode: scan,
                    uChar: KEY_EVENT_RECORD_0 { UnicodeChar: ch },
                    dwControlKeyState: 0,
                },
            },
        }
    }

    fn records(input: &Input) -> Vec<INPUT_RECORD> {
        let press = |vk, scan, ch| [key(vk, scan, ch, true), key(vk, scan, ch, false)];
        match input {
            Input::Escape => press(0x1B, 0x01, 0x1B).to_vec(),
            Input::Enter => press(0x0D, 0x1C, u16::from(b'\r')).to_vec(),
            Input::Text(s) => s.encode_utf16().flat_map(|u| press(0, 0, u)).collect(),
            Input::Pause(_) => Vec::new(),
        }
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
            }
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

    #[cfg(not(windows))]
    #[test]
    fn non_windows_is_noop() {
        assert!(!set_title(1, "x"));
        assert!(!send_input(1, &[Input::Escape]));
        assert!(!can_attach(1));
        assert_eq!(title(1), None);
    }
}
