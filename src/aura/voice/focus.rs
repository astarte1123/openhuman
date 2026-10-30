//! Foreground-application detection for dictation delivery routing.
//!
//! The voice server needs to know whether the Aura desktop app itself holds
//! keyboard focus when a dictation hotkey fires: if it does, the transcript
//! is delivered over Socket.IO (`dictation:transcription`) so the frontend
//! inserts it into the active chat input; if an external app holds focus,
//! the transcript is pasted via the OS clipboard + synthetic Ctrl/Cmd+V.
//!
//! - **macOS**: `capture_expected_app_name` in `server.rs` already queries
//!   the real frontmost app name via the accessibility APIs.
//! - **Windows**: we compare the foreground window's PID with our own. The
//!   core normally runs in-process inside the Tauri host (PR #1061), so an
//!   equal PID means "Aura is focused". Hand-rolled `user32`/`kernel32` FFI —
//!   no extra crate dependencies.
//! - **Linux**: not implemented — falls back to "external", i.e. paste
//!   delivery (the historical behavior on this platform).

/// Returns `true` when the current process owns the foreground window.
///
/// Used by the voice server to route the transcript into the app (Socket.IO)
/// instead of pasting into whatever external window happens to be focused.
/// Returns `false` on platforms without an implementation — the caller then
/// keeps the paste-external behavior, which is safe (no text is lost, it is
/// pasted into the focused window as before).
pub fn foreground_is_own_process() -> bool {
    imp::foreground_is_own_process().unwrap_or(false)
}

#[cfg(target_os = "windows")]
mod imp {
    type HWND = *mut core::ffi::c_void;
    type DWORD = u32;

    #[link(name = "user32")]
    extern "system" {
        fn GetForegroundWindow() -> HWND;
        fn GetWindowThreadProcessId(hwnd: HWND, pid: *mut DWORD) -> DWORD;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcessId() -> DWORD;
    }

    pub(super) fn foreground_is_own_process() -> Option<bool> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                // No window is focused (e.g. desktop) — treat as external.
                return Some(false);
            }
            let mut fg_pid: DWORD = 0;
            // lpdwProcessId must not be NULL per API contract.
            GetWindowThreadProcessId(hwnd, &mut fg_pid as *mut DWORD);
            let own_pid = GetCurrentProcessId();
            Some(fg_pid == own_pid)
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub(super) fn foreground_is_own_process() -> Option<bool> {
        // macOS uses the accessibility-based capture in server.rs; other
        // platforms fall back to paste-external delivery.
        None
    }
}

#[cfg(test)]
mod tests {
    use super::foreground_is_own_process;

    #[test]
    fn returns_boolean_without_panicking() {
        // On Windows this exercises the real FFI against the actual desktop
        // session (CI agents may have no foreground window — false is fine).
        // Elsewhere it is the documented None → false fallback.
        let _ = foreground_is_own_process();
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn non_windows_falls_back_to_external() {
        assert!(!foreground_is_own_process());
    }
}
