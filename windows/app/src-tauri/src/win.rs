//! Windows 전용 도구 — 창 앞으로 가져오기·비활성 표시·패널 창 스타일·소리·DPAPI·Windows Terminal 탭 고르기.
//! **모두 Windows 실기 미확인.** 다른 OS 에서는 아무것도 하지 않는다(금고만 평문 그대로 — 개발용).

/// 계정 금고에 넣을 바이트를 암호화한다(Windows: DPAPI 현재 사용자, 그 밖: 그대로).
pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
    sys::protect(plain)
}

pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
    sys::unprotect(sealed)
}

pub use sys::{bring_to_front, play_sound, process_window, select_wt_tab, window_pid};
// 패널 스타일·비활성 표시는 Windows 에서만 부른다
#[cfg(windows)]
pub use sys::{show_no_activate, style_notify_panel};

#[cfg(windows)]
mod sys {
    use std::ptr::{null, null_mut};

    use windows_sys::Win32::Foundation::{LocalFree, HWND, LPARAM, TRUE};
    use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ALIAS, SND_ASYNC, SND_NODEFAULT};
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindow, GetWindowLongPtrW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, BringWindowToTop,
        GWL_EXSTYLE, GW_OWNER, HWND_TOPMOST, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_RESTORE,
        SW_SHOWNOACTIVATE, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn blob(data: &[u8]) -> CRYPT_INTEGER_BLOB {
        CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 }
    }

    /// DPAPI 결과를 복사하고 LocalFree 로 놓는다.
    unsafe fn take(out: CRYPT_INTEGER_BLOB) -> Vec<u8> {
        // SAFETY: DPAPI 가 채운 버퍼와 길이
        let v = unsafe { std::slice::from_raw_parts(out.pbData, out.cbData as usize) }.to_vec();
        unsafe { LocalFree(out.pbData.cast()) };
        v
    }

    pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
        let input = blob(plain);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
        // SAFETY: 입력 버퍼는 이 함수 동안 살아 있고, 출력은 take 가 놓는다
        let ok = unsafe { CryptProtectData(&input, null(), null(), null(), null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) } != 0;
        ok.then(|| unsafe { take(out) })
    }

    pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
        let input = blob(sealed);
        let mut out = CRYPT_INTEGER_BLOB { cbData: 0, pbData: null_mut() };
        // SAFETY: 위와 같다
        let ok = unsafe { CryptUnprotectData(&input, null_mut(), null(), null(), null(), CRYPTPROTECT_UI_FORBIDDEN, &mut out) } != 0;
        ok.then(|| unsafe { take(out) })
    }

    /// 창을 앞으로 — 최소화면 복원하고, 포그라운드 잠금에 막히면 포그라운드 스레드에 입력을 붙여 다시 시도한다.
    pub fn bring_to_front(hwnd: isize) {
        let h = hwnd as HWND;
        // SAFETY: 창 핸들 조작만 한다(잘못된 핸들이면 실패할 뿐이다)
        unsafe {
            if IsIconic(h) != 0 {
                ShowWindow(h, SW_RESTORE);
            }
            if SetForegroundWindow(h) != 0 {
                return;
            }
            let fg = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg, null_mut());
            let me = GetCurrentThreadId();
            if fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, TRUE) != 0 {
                BringWindowToTop(h);
                SetForegroundWindow(h);
                AttachThreadInput(me, fg_thread, 0);
            }
        }
    }

    /// 포커스를 가져가지 않고 보인다.
    pub fn show_no_activate(hwnd: isize) {
        // SAFETY: 창 표시만 바꾼다
        unsafe { ShowWindow(hwnd as HWND, SW_SHOWNOACTIVATE) };
    }

    /// 알림 패널: 눌러도 활성화되지 않고(WS_EX_NOACTIVATE) 작업 표시줄·Alt+Tab 에 나타나지 않으며(WS_EX_TOOLWINDOW) 늘 위.
    pub fn style_notify_panel(hwnd: isize) {
        let h = hwnd as HWND;
        // SAFETY: 우리 창의 확장 스타일만 바꾼다
        unsafe {
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
            let ex = (ex | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST) & !WS_EX_APPWINDOW;
            SetWindowLongPtrW(h, GWL_EXSTYLE, ex as isize);
            SetWindowPos(h, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_FRAMECHANGED);
        }
    }

    /// 시스템 소리 별칭을 비동기로 — done = 알림음(없으면 별표), wait = 경고음. 실패는 무시.
    pub fn play_sound(kind: &str) {
        let aliases: &[&str] = match kind {
            "done" => &["SystemNotification", "SystemAsterisk"],
            "wait" => &["SystemExclamation"],
            _ => &[],
        };
        for a in aliases {
            let w = wide(a);
            // SAFETY: NUL 로 끝나는 별칭 이름
            if unsafe { PlaySoundW(w.as_ptr(), null_mut(), SND_ALIAS | SND_ASYNC | SND_NODEFAULT) } != 0 {
                return;
            }
        }
    }

    pub fn window_pid(hwnd: isize) -> u32 {
        let mut pid = 0u32;
        // SAFETY: 창 핸들 조회
        unsafe { GetWindowThreadProcessId(hwnd as HWND, &mut pid) };
        pid
    }

    unsafe extern "system" fn collect(hwnd: HWND, lparam: LPARAM) -> i32 {
        // SAFETY: EnumWindows 에 넘긴 Vec
        let list = unsafe { &mut *(lparam as *mut Vec<(u32, isize)>) };
        if unsafe { IsWindowVisible(hwnd) } != 0 && unsafe { GetWindow(hwnd, GW_OWNER) }.is_null() {
            list.push((window_pid(hwnd as isize), hwnd as isize));
        }
        TRUE
    }

    /// 그 프로세스들 가운데 하나의 보이는 소유자 없는 최상위 창.
    pub fn process_window(pids: &[u32]) -> Option<isize> {
        let mut list: Vec<(u32, isize)> = Vec::new();
        // SAFETY: 콜백은 EnumWindows 안에서만 list 를 쓴다
        unsafe { EnumWindows(Some(collect), &mut list as *mut _ as LPARAM) };
        list.into_iter().find(|(p, _)| pids.contains(p)).map(|(_, h)| h)
    }

    /// Windows Terminal 창 안에서 이름이 `marker` 인 탭을 UI Automation 으로 찾아 고른다.
    pub fn select_wt_tab(hwnd: isize, marker: &str) -> bool {
        use windows::Win32::Foundation::HWND as WHWND;
        use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
        use windows::Win32::UI::Accessibility::{
            CUIAutomation, IUIAutomation, IUIAutomationSelectionItemPattern, TreeScope_Descendants, UIA_SelectionItemPatternId,
            UIA_TabItemControlTypeId,
        };
        // 탭 항목을 모두 훑어 이름이 표지인 것을 고른다(속성 조건은 VARIANT 를 만들어야 해 직접 거른다)
        let find = || -> windows::core::Result<bool> {
            // SAFETY: COM 호출 — 이 스레드에서 COM 을 열고(이미 열려 있으면 그대로) 받은 인터페이스만 쓴다
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let uia: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
                let root = uia.ElementFromHandle(WHWND(hwnd as *mut _))?;
                let all = root.FindAll(TreeScope_Descendants, &uia.CreateTrueCondition()?)?;
                for i in 0..all.Length()? {
                    let e = all.GetElement(i)?;
                    if e.CurrentControlType()? == UIA_TabItemControlTypeId && e.CurrentName()? == marker {
                        let p: IUIAutomationSelectionItemPattern = e.GetCurrentPatternAs(UIA_SelectionItemPatternId)?;
                        p.Select()?;
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        };
        find().unwrap_or(false)
    }
}

#[cfg(not(windows))]
mod sys {
    pub fn protect(plain: &[u8]) -> Option<Vec<u8>> {
        Some(plain.to_vec())
    }
    pub fn unprotect(sealed: &[u8]) -> Option<Vec<u8>> {
        Some(sealed.to_vec())
    }
    pub fn bring_to_front(_hwnd: isize) {}
    pub fn play_sound(_kind: &str) {}
    pub fn window_pid(_hwnd: isize) -> u32 {
        0
    }
    pub fn process_window(_pids: &[u32]) -> Option<isize> {
        None
    }
    pub fn select_wt_tab(_hwnd: isize, _marker: &str) -> bool {
        false
    }
}
