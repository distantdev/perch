use std::net::{Ipv4Addr, Ipv6Addr};

use sysinfo::{Pid, ProcessesToUpdate, System};
use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GetExtendedTcpTable, GetExtendedUdpTable, MIB_TCP6TABLE_OWNER_PID, MIB_TCPTABLE_OWNER_PID,
    MIB_TCP_STATE_LISTEN, MIB_UDP6TABLE_OWNER_PID, MIB_UDPTABLE_OWNER_PID,
    TCP_TABLE_OWNER_PID_LISTENER, UDP_TABLE_OWNER_PID,
};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6};

use crate::error::{PerchError, Result};
use crate::model::{ProcessInfo, Protocol, RawListener};
use crate::platform::windows_cwd;
use crate::platform::{is_local_bind, PlatformScanner};
use crate::scan::project::infer_project_path;

fn win32_ok(status: u32) -> bool {
    status == NO_ERROR.0
}

fn win32_buffer_too_small(status: u32) -> bool {
    status == ERROR_INSUFFICIENT_BUFFER.0
}

pub struct WindowsScanner;

impl WindowsScanner {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformScanner for WindowsScanner {
    fn scan_listeners(&self) -> Result<Vec<RawListener>> {
        let mut listeners = Vec::new();
        listeners.extend(scan_tcp_v4()?);
        listeners.extend(scan_tcp_v6()?);
        listeners.extend(scan_udp_v4()?);
        listeners.extend(scan_udp_v6()?);
        dedupe_listeners(&mut listeners);
        Ok(listeners)
    }

    fn hydrate_process(&self, pid: u32) -> Result<ProcessInfo> {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        hydrate_from_sysinfo(&system, pid)
    }
}

fn dedupe_listeners(listeners: &mut Vec<RawListener>) {
    listeners.sort_by_key(|l| (l.port, l.protocol as u8, l.pid));
    listeners.dedup_by_key(|l| (l.port, l.protocol as u8, l.pid));
}

fn scan_tcp_v4() -> Result<Vec<RawListener>> {
    let mut size: u32 = 0;
    unsafe {
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET.0.into(),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }

    let mut buf = vec![0u8; size as usize];
    loop {
        let status = unsafe {
            GetExtendedTcpTable(
                Some(buf.as_mut_ptr().cast()),
                &mut size,
                false,
                AF_INET.0.into(),
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if win32_ok(status) {
            break;
        }
        if !win32_buffer_too_small(status) {
            return Err(PerchError::Scan(format!(
                "GetExtendedTcpTable IPv4 failed: {status:?}"
            )));
        }
        buf.resize(size as usize, 0);
    }

    let table = unsafe { &*(buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID) };
    let rows =
        unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };

    let mut out = Vec::new();
    for row in rows {
        if row.dwState != MIB_TCP_STATE_LISTEN.0 as u32 {
            continue;
        }
        let port = u16::from_be(row.dwLocalPort as u16);
        let addr = Ipv4Addr::from(u32::from_be(row.dwLocalAddr)).to_string();
        if !is_local_bind(&addr) && addr != "0.0.0.0" {
            continue;
        }
        let pid = row.dwOwningPid;
        if pid == 0 {
            continue;
        }
        out.push(RawListener {
            port,
            protocol: Protocol::Tcp,
            bind_address: addr,
            pid,
        });
    }
    Ok(out)
}

fn scan_tcp_v6() -> Result<Vec<RawListener>> {
    let mut size: u32 = 0;
    unsafe {
        let _ = GetExtendedTcpTable(
            None,
            &mut size,
            false,
            AF_INET6.0.into(),
            TCP_TABLE_OWNER_PID_LISTENER,
            0,
        );
    }

    let mut buf = vec![0u8; size as usize];
    loop {
        let status = unsafe {
            GetExtendedTcpTable(
                Some(buf.as_mut_ptr().cast()),
                &mut size,
                false,
                AF_INET6.0.into(),
                TCP_TABLE_OWNER_PID_LISTENER,
                0,
            )
        };
        if win32_ok(status) {
            break;
        }
        if !win32_buffer_too_small(status) {
            return Err(PerchError::Scan(format!(
                "GetExtendedTcpTable IPv6 failed: {status:?}"
            )));
        }
        buf.resize(size as usize, 0);
    }

    let table = unsafe { &*(buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID) };
    let rows =
        unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };

    let mut out = Vec::new();
    for row in rows {
        if row.dwState != MIB_TCP_STATE_LISTEN.0 as u32 {
            continue;
        }
        let port = u16::from_be(row.dwLocalPort as u16);
        let addr = format_ipv6(&row.ucLocalAddr);
        if !is_local_bind(&addr) {
            continue;
        }
        let pid = row.dwOwningPid;
        if pid == 0 {
            continue;
        }
        out.push(RawListener {
            port,
            protocol: Protocol::Tcp,
            bind_address: addr,
            pid,
        });
    }
    Ok(out)
}

fn format_ipv6(bytes: &[u8; 16]) -> String {
    if bytes.iter().take(12).all(|&b| b == 0) && bytes[12] == 0 && bytes[13] == 0 {
        if bytes[14] == 0 && bytes[15] == 1 {
            return "::1".into();
        }
        if bytes[14] == 0 && bytes[15] == 0 {
            return "::".into();
        }
    }
    Ipv6Addr::from(*bytes).to_string()
}

fn scan_udp_v4() -> Result<Vec<RawListener>> {
    let mut size: u32 = 0;
    unsafe {
        let _ = GetExtendedUdpTable(
            None,
            &mut size,
            false,
            AF_INET.0.into(),
            UDP_TABLE_OWNER_PID,
            0,
        );
    }
    let mut buf = vec![0u8; size as usize];
    loop {
        let status = unsafe {
            GetExtendedUdpTable(
                Some(buf.as_mut_ptr().cast()),
                &mut size,
                false,
                AF_INET.0.into(),
                UDP_TABLE_OWNER_PID,
                0,
            )
        };
        if win32_ok(status) {
            break;
        }
        if !win32_buffer_too_small(status) {
            return Err(PerchError::Scan(format!(
                "GetExtendedUdpTable IPv4 failed: {status:?}"
            )));
        }
        buf.resize(size as usize, 0);
    }

    let table = unsafe { &*(buf.as_ptr() as *const MIB_UDPTABLE_OWNER_PID) };
    let rows =
        unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };

    let mut out = Vec::new();
    for row in rows {
        let port = u16::from_be(row.dwLocalPort as u16);
        let addr = Ipv4Addr::from(u32::from_be(row.dwLocalAddr)).to_string();
        if !is_local_bind(&addr) {
            continue;
        }
        let pid = row.dwOwningPid;
        if pid == 0 {
            continue;
        }
        out.push(RawListener {
            port,
            protocol: Protocol::Udp,
            bind_address: addr,
            pid,
        });
    }
    Ok(out)
}

fn scan_udp_v6() -> Result<Vec<RawListener>> {
    let mut size: u32 = 0;
    unsafe {
        let _ = GetExtendedUdpTable(
            None,
            &mut size,
            false,
            AF_INET6.0.into(),
            UDP_TABLE_OWNER_PID,
            0,
        );
    }
    let mut buf = vec![0u8; size as usize];
    loop {
        let status = unsafe {
            GetExtendedUdpTable(
                Some(buf.as_mut_ptr().cast()),
                &mut size,
                false,
                AF_INET6.0.into(),
                UDP_TABLE_OWNER_PID,
                0,
            )
        };
        if win32_ok(status) {
            break;
        }
        if !win32_buffer_too_small(status) {
            return Err(PerchError::Scan(format!(
                "GetExtendedUdpTable IPv6 failed: {status:?}"
            )));
        }
        buf.resize(size as usize, 0);
    }

    let table = unsafe { &*(buf.as_ptr() as *const MIB_UDP6TABLE_OWNER_PID) };
    let rows =
        unsafe { std::slice::from_raw_parts(table.table.as_ptr(), table.dwNumEntries as usize) };

    let mut out = Vec::new();
    for row in rows {
        let port = u16::from_be(row.dwLocalPort as u16);
        let addr = format_ipv6(&row.ucLocalAddr);
        if !is_local_bind(&addr) {
            continue;
        }
        let pid = row.dwOwningPid;
        if pid == 0 {
            continue;
        }
        out.push(RawListener {
            port,
            protocol: Protocol::Udp,
            bind_address: addr,
            pid,
        });
    }
    Ok(out)
}

fn hydrate_from_sysinfo(system: &System, pid: u32) -> Result<ProcessInfo> {
    let Some(process) = system.process(Pid::from_u32(pid)) else {
        return Ok(ProcessInfo {
            executable: format!("pid:{pid}"),
            ..Default::default()
        });
    };

    let executable = process
        .exe()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| format!("pid:{pid}"));

    let cmdline = process
        .cmd()
        .iter()
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");

    let cwd = process
        .cwd()
        .map(|p| p.display().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| windows_cwd::read_process_cwd(pid))
        .or_else(|| infer_project_path(&cmdline, &executable))
        .unwrap_or_default();

    let memory_rss_bytes = process.memory();
    let uptime_secs = process.run_time();
    let cpu_percent = process.cpu_usage();

    Ok(ProcessInfo {
        executable,
        cwd,
        cmdline,
        cpu_percent,
        memory_rss_bytes,
        uptime_secs,
    })
}
