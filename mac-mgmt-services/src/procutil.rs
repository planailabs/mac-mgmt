//! Cross-platform process primitives used by the supervisor: liveness check,
//! signal/terminate, and reap. Unix uses `libc` (kill/waitpid); Windows uses
//! `windows-sys` (OpenProcess/TerminateProcess/WaitForSingleObject). Windows has
//! no graceful per-signal termination, so any signal maps to TerminateProcess.

/// Portable signal numbers (match unix values; on windows both terminate).
pub const SIGTERM: i32 = 15;
pub const SIGKILL: i32 = 9;

#[cfg(unix)]
pub use unix_imp::*;
#[cfg(windows)]
pub use windows_imp::*;

#[cfg(unix)]
mod unix_imp {
    /// Is the process still alive? (signal 0 probe)
    pub fn is_alive(pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    /// Send `signal` to the process.
    pub fn send_signal(pid: u32, signal: i32) -> std::io::Result<()> {
        if unsafe { libc::kill(pid as i32, signal) } == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    /// Non-blocking: reap the child if it has exited. Returns true once it has
    /// exited (and been reaped). Uses waitpid so zombies are detected + reaped.
    pub fn reap_if_exited(pid: u32) -> bool {
        // >0: reaped; ==0: still running; <0: not our child / already gone.
        unsafe { libc::waitpid(pid as i32, std::ptr::null_mut(), libc::WNOHANG) != 0 }
    }

    /// Block until the child is reaped (after a kill).
    pub fn reap_blocking(pid: u32) {
        unsafe {
            libc::waitpid(pid as i32, std::ptr::null_mut(), 0);
        }
    }
}

#[cfg(windows)]
mod windows_imp {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, TerminateProcess, WaitForSingleObject,
    };

    const PROCESS_TERMINATE: u32 = 0x0001;
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
    const STILL_ACTIVE: u32 = 259;
    const INFINITE: u32 = 0xFFFF_FFFF;

    fn open(pid: u32, access: u32) -> Option<*mut core::ffi::c_void> {
        let h = unsafe { OpenProcess(access, 0, pid) };
        if h.is_null() { None } else { Some(h) }
    }

    pub fn is_alive(pid: u32) -> bool {
        let Some(h) = open(pid, PROCESS_QUERY_LIMITED_INFORMATION) else { return false };
        let mut code: u32 = 0;
        let alive = unsafe { GetExitCodeProcess(h, &mut code) != 0 } && code == STILL_ACTIVE;
        unsafe { CloseHandle(h) };
        alive
    }

    pub fn send_signal(pid: u32, _signal: i32) -> std::io::Result<()> {
        // No per-signal semantics on windows — terminate the process.
        let Some(h) = open(pid, PROCESS_TERMINATE) else {
            return Err(std::io::Error::last_os_error());
        };
        let ok = unsafe { TerminateProcess(h, 1) != 0 };
        unsafe { CloseHandle(h) };
        if ok { Ok(()) } else { Err(std::io::Error::last_os_error()) }
    }

    /// Windows has no zombies; "reaped" == no longer alive.
    pub fn reap_if_exited(pid: u32) -> bool {
        !is_alive(pid)
    }

    pub fn reap_blocking(pid: u32) {
        if let Some(h) = open(pid, PROCESS_SYNCHRONIZE) {
            unsafe { WaitForSingleObject(h, INFINITE) };
            unsafe { CloseHandle(h) };
        }
    }
}
