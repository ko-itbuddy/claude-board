//! 알림 패널을 분리된 프로세스로 띄우고 기다리지 않는다 (SPEC §3.6).

use std::path::Path;
use std::process::{Command, Stdio};

/// `app args…` 를 띄우고 곧바로 돌아온다. 표준 입출력은 닫는다.
///
/// Windows: `CREATE_BREAKAWAY_FROM_JOB | DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP`(breakaway 가 거부되면 그 플래그 없이 다시).
/// 그 밖: 새 세션(setsid)으로 띄운다.
pub fn detached(app: &Path, args: &[String]) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{
            CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
        };
        no_inherit_std_handles();
        let base = DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP;
        for flags in [base | CREATE_BREAKAWAY_FROM_JOB, base] {
            let mut cmd = Command::new(app);
            cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).creation_flags(flags);
            if cmd.spawn().is_ok() {
                return true;
            }
        }
        false
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let mut cmd = Command::new(app);
        cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        // SAFETY: fork 뒤 exec 전에 setsid 만 부른다(비동기 신호 안전)
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
        cmd.spawn().is_ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (app, args);
        false
    }
}

/// 우리 표준 입출력(Claude Code 가 준 파이프)이 패널 프로세스로 상속되지 않게 한다.
/// std 는 bInheritHandles 로 프로세스를 만들므로, 상속되면 Claude Code 가 패널이 닫힐 때까지 hook 출력을 기다린다.
#[cfg(windows)]
fn no_inherit_std_handles() {
    use windows_sys::Win32::Foundation::{SetHandleInformation, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE};
    for which in [STD_INPUT_HANDLE, STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
        // SAFETY: 우리 표준 핸들의 상속 표시만 끈다
        unsafe {
            let h = GetStdHandle(which);
            if !h.is_null() && h != INVALID_HANDLE_VALUE {
                SetHandleInformation(h, HANDLE_FLAG_INHERIT, 0);
            }
        }
    }
}
