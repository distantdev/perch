use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::config::Config;
use crate::error::{PerchError, Result};
use crate::model::DevServer;
use crate::platform::PlatformScanner;
use crate::process::docker;
use crate::scan::classify::is_docker_proxy;
use crate::scan::pipeline::run_scan;

#[derive(Debug, Clone, Copy)]
pub enum KillMode {
    Graceful,
    Force,
}

pub struct ProcessControl;

impl ProcessControl {
    pub fn resolve_server(
        target: &str,
        scanner: &dyn PlatformScanner,
        config: &Config,
    ) -> Result<DevServer> {
        let scan = run_scan(scanner, config)?;

        if let Ok(port) = target.parse::<u16>() {
            return scan
                .servers
                .into_iter()
                .find(|s| s.port == port)
                .ok_or(PerchError::PortNotFound(port));
        }

        if let Ok(pid) = target.parse::<u32>() {
            if let Some(server) = scan.servers.into_iter().find(|s| s.pid == pid) {
                return Ok(server);
            }
            return Ok(DevServer {
                port: 0,
                protocol: crate::model::Protocol::Tcp,
                bind_address: String::new(),
                pid,
                executable: String::new(),
                cwd: String::new(),
                cmdline: String::new(),
                cpu_percent: 0.0,
                memory_rss_bytes: 0,
                uptime_secs: 0,
                server_type: crate::model::ServerType::Other,
                runtime_state: crate::model::ServerRuntimeState::Running,
                docker_container: None,
                warnings: Vec::new(),
            });
        }

        Err(PerchError::InvalidTarget(target.to_string()))
    }

    pub fn kill(
        target: &str,
        force: bool,
        scanner: &dyn PlatformScanner,
        config: &Config,
    ) -> Result<()> {
        let server = Self::resolve_server(target, scanner, config)?;
        let mode = if force {
            KillMode::Force
        } else {
            KillMode::Graceful
        };
        Self::kill_server(&server, mode)
    }

    pub fn pause(target: &str, scanner: &dyn PlatformScanner, config: &Config) -> Result<()> {
        let server = Self::resolve_server(target, scanner, config)?;
        Self::pause_server(&server)
    }

    pub fn resume(target: &str, scanner: &dyn PlatformScanner, config: &Config) -> Result<()> {
        let server = Self::resolve_server(target, scanner, config)?;
        Self::resume_server(&server)
    }

    pub fn kill_server(server: &DevServer, mode: KillMode) -> Result<()> {
        if let Some(container) = docker_container_name(server) {
            let _ = mode;
            return docker::docker_stop(&container);
        }
        ensure_can_kill(server.pid)?;
        match mode {
            KillMode::Graceful => platform_kill_graceful(server.pid),
            KillMode::Force => platform_kill_force(server.pid),
        }
    }

    pub fn pause_server(server: &DevServer) -> Result<()> {
        if let Some(container) = docker_container_name(server) {
            return docker::docker_pause(&container);
        }
        ensure_can_pause(server.pid)?;
        platform_pause(server.pid)
    }

    pub fn resume_server(server: &DevServer) -> Result<()> {
        if let Some(container) = docker_container_name(server) {
            return docker::docker_unpause(&container);
        }
        ensure_can_pause(server.pid)?;
        platform_resume(server.pid)
    }
}

fn docker_container_name(server: &DevServer) -> Option<String> {
    if let Some(name) = &server.docker_container {
        return Some(name.clone());
    }
    if is_docker_proxy(&server.executable) {
        return docker::container_for_port(server.port);
    }
    None
}

fn ensure_process_exists(pid: u32) -> Result<()> {
    let mut system = System::new();
    system.refresh_processes(ProcessesToUpdate::All, true);
    if system.process(Pid::from_u32(pid)).is_none() {
        return Err(PerchError::ProcessNotFound(pid));
    }
    Ok(())
}

fn ensure_can_kill(pid: u32) -> Result<()> {
    ensure_process_exists(pid)?;
    ensure_process_owner(pid)?;

    #[cfg(windows)]
    if !can_terminate_process(pid) {
        return Err(PerchError::PermissionDenied {
            pid,
            reason: "unable to terminate process; try running as administrator".into(),
        });
    }

    Ok(())
}

fn ensure_can_pause(pid: u32) -> Result<()> {
    ensure_process_exists(pid)?;
    ensure_process_owner(pid)?;

    #[cfg(windows)]
    if !can_suspend_process(pid) {
        return Err(PerchError::PermissionDenied {
            pid,
            reason: "unable to suspend process; try running as administrator".into(),
        });
    }

    Ok(())
}

fn ensure_process_owner(pid: u32) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        let uid = unsafe { libc::getuid() };
        if let Ok(status) = std::fs::read_to_string(format!("/proc/{pid}/status")) {
            for line in status.lines() {
                if let Some(val) = line.strip_prefix("Uid:\t") {
                    let owner: u32 = val
                        .split_whitespace()
                        .next()
                        .unwrap_or("0")
                        .parse()
                        .unwrap_or(0);
                    if owner != uid {
                        return Err(PerchError::PermissionDenied {
                            pid,
                            reason: "process owned by another user".into(),
                        });
                    }
                    break;
                }
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
    }

    Ok(())
}

#[cfg(windows)]
fn can_terminate_process(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE};

    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_TERMINATE, false, pid) else {
            return false;
        };
        let _ = CloseHandle(handle);
        true
    }
}

#[cfg(windows)]
fn can_suspend_process(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SUSPEND_RESUME};

    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_SUSPEND_RESUME, false, pid) else {
            return false;
        };
        let _ = CloseHandle(handle);
        true
    }
}

#[cfg(windows)]
fn platform_kill_graceful(pid: u32) -> Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, TerminateProcess, PROCESS_TERMINATE};

    unsafe {
        let handle =
            OpenProcess(PROCESS_TERMINATE, false, pid).map_err(|_| permission_error(pid))?;
        let ok = TerminateProcess(handle, 1).is_ok();
        let _ = CloseHandle(handle);
        if ok {
            Ok(())
        } else {
            Err(PerchError::ProcessControl(format!(
                "TerminateProcess failed for pid {pid}"
            )))
        }
    }
}

#[cfg(windows)]
fn platform_kill_force(pid: u32) -> Result<()> {
    platform_kill_graceful(pid)
}

#[cfg(windows)]
fn platform_pause(pid: u32) -> Result<()> {
    nt_suspend_resume(pid, true)
}

#[cfg(windows)]
fn platform_resume(pid: u32) -> Result<()> {
    nt_suspend_resume(pid, false)
}

#[cfg(windows)]
fn nt_suspend_resume(pid: u32, suspend: bool) -> Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_SUSPEND_RESUME};

    #[link(name = "ntdll")]
    extern "system" {
        fn NtSuspendProcess(process: isize) -> i32;
        fn NtResumeProcess(process: isize) -> i32;
    }

    unsafe {
        let handle =
            OpenProcess(PROCESS_SUSPEND_RESUME, false, pid).map_err(|_| permission_error(pid))?;
        let raw = handle.0 as isize;
        let status = if suspend {
            NtSuspendProcess(raw)
        } else {
            NtResumeProcess(raw)
        };
        let _ = CloseHandle(handle);
        if status == 0 {
            return Ok(());
        }
        Err(PerchError::ProcessControl(format!(
            "NtSuspend/Resume failed for pid {pid} (status {status})"
        )))
    }
}

#[cfg(unix)]
fn platform_kill_graceful(pid: u32) -> Result<()> {
    send_signal(pid, nix::sys::signal::Signal::SIGTERM)
}

#[cfg(unix)]
fn platform_kill_force(pid: u32) -> Result<()> {
    send_signal(pid, nix::sys::signal::Signal::SIGKILL)
}

#[cfg(unix)]
fn platform_pause(pid: u32) -> Result<()> {
    send_signal(pid, nix::sys::signal::Signal::SIGSTOP)
}

#[cfg(unix)]
fn platform_resume(pid: u32) -> Result<()> {
    send_signal(pid, nix::sys::signal::Signal::SIGCONT)
}

#[cfg(unix)]
fn send_signal(pid: u32, signal: nix::sys::signal::Signal) -> Result<()> {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid as NixPid;

    let sig: Signal = signal;
    kill(NixPid::from_raw(pid as i32), sig).map_err(|e| {
        if e == nix::errno::Errno::EPERM {
            PerchError::PermissionDenied {
                pid,
                reason: format!("{e}"),
            }
        } else if e == nix::errno::Errno::ESRCH {
            PerchError::ProcessNotFound(pid)
        } else {
            PerchError::ProcessControl(format!("signal {sig:?} to {pid}: {e}"))
        }
    })
}

#[cfg(windows)]
fn permission_error(pid: u32) -> PerchError {
    PerchError::PermissionDenied {
        pid,
        reason: "access denied".into(),
    }
}
