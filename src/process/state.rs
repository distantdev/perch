use std::collections::HashSet;

#[cfg(unix)]
use sysinfo::{Pid, ProcessStatus, ProcessesToUpdate, System};

use crate::model::ServerRuntimeState;

pub fn detect_runtime_state(
    pid: u32,
    docker_container: Option<&str>,
    paused_containers: Option<&HashSet<String>>,
) -> ServerRuntimeState {
    if let Some(name) = docker_container {
        if paused_containers.is_some_and(|paused| paused.contains(name)) {
            return ServerRuntimeState::Paused;
        }
    }

    if is_process_paused(pid) {
        ServerRuntimeState::Paused
    } else {
        ServerRuntimeState::Running
    }
}

fn is_process_paused(pid: u32) -> bool {
    #[cfg(windows)]
    {
        windows_process_paused(pid)
    }

    #[cfg(target_os = "linux")]
    {
        linux_process_paused(pid)
    }

    #[cfg(all(unix, not(target_os = "linux")))]
    {
        unix_process_paused(pid)
    }
}

#[cfg(windows)]
fn windows_process_paused(pid: u32) -> bool {
    windows_process_paused_via_kernel32(pid) || windows_process_paused_via_threads(pid)
}

#[cfg(windows)]
fn windows_process_paused_via_kernel32(pid: u32) -> bool {
    use std::ffi::c_void;
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::LibraryLoader::{GetModuleHandleA, GetProcAddress};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    use windows::core::s;

    type IsProcessSuspendedFn = unsafe extern "system" fn(HANDLE) -> windows::Win32::Foundation::BOOL;

    unsafe {
        let Ok(process) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };

        let Ok(kernel32) = GetModuleHandleA(s!("kernel32.dll")) else {
            let _ = CloseHandle(process);
            return false;
        };

        let Some(proc) = GetProcAddress(kernel32, s!("IsProcessSuspended")) else {
            let _ = CloseHandle(process);
            return false;
        };

        let is_process_suspended: IsProcessSuspendedFn =
            std::mem::transmute(proc as *const c_void);
        let paused = is_process_suspended(process).as_bool();
        let _ = CloseHandle(process);
        paused
    }
}

#[cfg(windows)]
fn windows_process_paused_via_threads(pid: u32) -> bool {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::Threading::{OpenThread, THREAD_QUERY_LIMITED_INFORMATION};

    #[link(name = "ntdll")]
    extern "system" {
        fn NtQueryInformationThread(
            thread_handle: isize,
            thread_information_class: u32,
            thread_information: *mut core::ffi::c_void,
            thread_information_length: u32,
            return_length: *mut u32,
        ) -> i32;
    }

    const THREAD_SUSPEND_COUNT: u32 = 35;

    unsafe fn thread_suspend_count(handle: HANDLE) -> Option<u32> {
        let mut count: u32 = 0;
        let status = NtQueryInformationThread(
            handle.0 as isize,
            THREAD_SUSPEND_COUNT,
            (&mut count as *mut u32).cast(),
            std::mem::size_of::<u32>() as u32,
            std::ptr::null_mut(),
        );
        if status == 0 {
            Some(count)
        } else {
            None
        }
    }

    unsafe {
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) else {
            return false;
        };
        let _snapshot_guard = SnapshotGuard(snapshot);

        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };

        let mut queried = 0u32;
        let mut any_running = false;
        let mut any_suspended = false;

        if Thread32First(snapshot, &mut entry).is_err() {
            return false;
        }

        loop {
            if entry.th32OwnerProcessID == pid {
                if let Ok(thread) =
                    OpenThread(THREAD_QUERY_LIMITED_INFORMATION, false, entry.th32ThreadID)
                {
                    queried += 1;
                    match thread_suspend_count(thread) {
                        Some(0) => any_running = true,
                        Some(_) => any_suspended = true,
                        None => {}
                    }
                    let _ = CloseHandle(thread);
                }
            }

            if Thread32Next(snapshot, &mut entry).is_err() {
                break;
            }
        }

        queried > 0 && any_suspended && !any_running
    }
}

#[cfg(windows)]
struct SnapshotGuard(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

#[cfg(target_os = "linux")]
fn linux_process_paused(pid: u32) -> bool {
    let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) else {
        return false;
    };
    linux_state_is_paused(&status)
}

#[cfg(all(unix, not(target_os = "linux")))]
fn unix_process_paused(pid: u32) -> bool {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    system
        .process(Pid::from_u32(pid))
        .is_some_and(|process| process.status() == ProcessStatus::Stop)
}

#[cfg(any(target_os = "linux", test))]
fn linux_state_is_paused(status: &str) -> bool {
    for line in status.lines() {
        if let Some(state) = line.strip_prefix("State:") {
            return state.trim().starts_with('T');
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_status_detects_stopped_process() {
        let status = "Name:\tnode\nState:\tT (stopped)\n";
        assert!(linux_state_is_paused(status));
    }

    #[test]
    fn linux_status_detects_running_process() {
        let status = "Name:\tnode\nState:\tS (sleeping)\n";
        assert!(!linux_state_is_paused(status));
    }

    #[test]
    fn docker_container_pause_wins() {
        let paused = HashSet::from(["api".to_string()]);
        let state = detect_runtime_state(123, Some("api"), Some(&paused));
        assert_eq!(state, ServerRuntimeState::Paused);
    }
}
