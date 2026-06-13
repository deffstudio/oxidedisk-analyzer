//! On-demand UAC elevation.
//!
//! The app runs unprivileged. Elevation is only requested when the user asks to
//! clean protected locations (e.g. `C:\Windows\Temp`, the Update cache, Prefetch)
//! and the current process lacks the rights. We relaunch *the same binary* with
//! the `runas` verb (triggers the UAC prompt) and pass a flag so the elevated
//! instance jumps straight to the temp-cleanup view. The unprivileged instance
//! then exits.

/// Command-line flag the elevated instance is launched with so it opens the
/// cleanup view immediately.
pub const CLEANUP_FLAG: &str = "--cleanup";

#[cfg(windows)]
mod imp {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

    /// Is the current process running with an elevated (administrator) token?
    pub fn is_elevated() -> bool {
        unsafe {
            let mut token: HANDLE = ptr::null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return false;
            }

            let mut elevation = TOKEN_ELEVATION {
                TokenIsElevated: 0,
            };
            let mut size = 0u32;
            let ok = GetTokenInformation(
                token,
                TokenElevation,
                &mut elevation as *mut _ as *mut _,
                std::mem::size_of::<TOKEN_ELEVATION>() as u32,
                &mut size,
            );
            CloseHandle(token);

            ok != 0 && elevation.TokenIsElevated != 0
        }
    }

    /// Relaunch the current executable elevated, forwarding `args`. Triggers a
    /// UAC prompt; returns `Err` if the user declines or the launch fails.
    pub fn relaunch_as_admin(args: &[&str]) -> Result<(), String> {
        let exe = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;

        let verb = to_wide("runas");
        let file = to_wide(&exe.to_string_lossy());
        let params = to_wide(&args.join(" "));

        let result = unsafe {
            ShellExecuteW(
                ptr::null_mut(),
                verb.as_ptr(),
                file.as_ptr(),
                params.as_ptr(),
                ptr::null(),
                SW_SHOWNORMAL,
            )
        };

        // ShellExecuteW returns a value > 32 on success.
        if (result as isize) > 32 {
            Ok(())
        } else {
            Err("Elevation was cancelled or failed.".to_string())
        }
    }

    fn to_wide(s: &str) -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn is_elevated() -> bool {
        false
    }

    pub fn relaunch_as_admin(_args: &[&str]) -> Result<(), String> {
        Err("Elevation is only supported on Windows.".to_string())
    }
}

pub use imp::{is_elevated, relaunch_as_admin};
