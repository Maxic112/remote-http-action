//! Win32 helpers: elevation, relaunch, console attach, current user SID.

use std::ffi::c_void;

use windows::Win32::Foundation::{CloseHandle, HANDLE, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TOKEN_USER, TokenElevation, TokenUser,
};
use windows::Win32::System::Console::{
    ATTACH_PARENT_PROCESS, AttachConsole, GetStdHandle, STD_OUTPUT_HANDLE,
};
use windows::Win32::System::Threading::{
    GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_SYNCHRONIZE, WaitForSingleObject,
};
use windows::Win32::UI::Shell::{SEE_MASK_NOASYNC, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

use crate::error::{AppError, AppResult};

fn with_token<T>(f: impl FnOnce(HANDLE) -> Option<T>) -> Option<T> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;
        let result = f(token);
        let _ = CloseHandle(token);
        result
    }
}

pub fn is_elevated() -> bool {
    with_token(|token| unsafe {
        let mut elevation = TOKEN_ELEVATION::default();
        let mut len = 0u32;
        GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut c_void),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut len,
        )
        .ok()?;
        Some(elevation.TokenIsElevated != 0)
    })
    .unwrap_or(false)
}

/// String SID of the user owning the current process, e.g. `S-1-5-21-...`.
pub fn current_user_sid() -> Option<String> {
    with_token(|token| unsafe {
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        GetTokenInformation(
            token,
            TokenUser,
            Some(buf.as_mut_ptr() as *mut c_void),
            len,
            &mut len,
        )
        .ok()?;
        let user = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut sid_str = PWSTR::null();
        ConvertSidToStringSidW(user.User.Sid, &mut sid_str).ok()?;
        let s = sid_str.to_string().ok();
        let _ = LocalFree(Some(HLOCAL(sid_str.0 as *mut c_void)));
        s
    })
}

pub fn relaunch_elevated() -> AppResult<()> {
    let exe = std::env::current_exe().map_err(|e| AppError::internal(e.to_string()))?;
    let file = HSTRING::from(exe.as_os_str());
    let params = HSTRING::from(format!("--wait-pid {}", std::process::id()));
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOASYNC,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_SHOWNORMAL.0,
        ..Default::default()
    };
    unsafe { ShellExecuteExW(&mut info) }.map_err(|e| {
        AppError::new(
            crate::error::ErrorCode::AdminRequired,
            format!("elevation was not granted: {e}"),
        )
    })
}

/// Waits (up to `timeout_ms`) for another process to exit. Used after an elevated relaunch.
pub fn wait_for_pid(pid: u32, timeout_ms: u32) {
    unsafe {
        if let Ok(h) = OpenProcess(PROCESS_SYNCHRONIZE, false, pid) {
            let _ = WaitForSingleObject(h, timeout_ms);
            let _ = CloseHandle(h);
        }
    }
}

/// Attaches to the parent console when the process has no usable stdout
/// (GUI-subsystem release builds started from a terminal).
pub fn attach_parent_console() {
    unsafe {
        let has_stdout = GetStdHandle(STD_OUTPUT_HANDLE)
            .map(|h| !h.is_invalid() && !h.0.is_null())
            .unwrap_or(false);
        if !has_stdout {
            let _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}
