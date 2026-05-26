use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;

use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::error::{PerchError, Result};
use crate::model::{ProcessInfo, Protocol, RawListener};
use crate::platform::{is_local_bind, PlatformScanner};
use crate::scan::project::infer_project_path;

pub struct LinuxScanner;

impl LinuxScanner {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformScanner for LinuxScanner {
    fn scan_listeners(&self) -> Result<Vec<RawListener>> {
        let mut listeners = Vec::new();
        parse_proc_net("/proc/net/tcp", Protocol::Tcp, &mut listeners)?;
        parse_proc_net("/proc/net/tcp6", Protocol::Tcp, &mut listeners)?;
        parse_proc_net("/proc/net/udp", Protocol::Udp, &mut listeners)?;
        parse_proc_net("/proc/net/udp6", Protocol::Udp, &mut listeners)?;
        Ok(listeners)
    }

    fn hydrate_process(&self, pid: u32) -> Result<ProcessInfo> {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);
        let mut info = hydrate_procfs(pid).unwrap_or_default();

        if let Some(process) = system.process(Pid::from_u32(pid)) {
            if info.executable.is_empty() {
                info.executable = process
                    .exe()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| format!("pid:{pid}"));
            }
            if info.cmdline.is_empty() {
                info.cmdline = process
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ");
            }
            if info.cwd.is_empty() {
                info.cwd = process
                    .cwd()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
            }
            info.memory_rss_bytes = process.memory();
            info.uptime_secs = process.run_time();
            info.cpu_percent = process.cpu_usage();
        }

        if info.cwd.is_empty() {
            if let Some(dir) = infer_project_path(&info.cmdline, &info.executable) {
                info.cwd = dir;
            }
        }

        Ok(info)
    }
}

fn parse_proc_net(path: &str, protocol: Protocol, out: &mut Vec<RawListener>) -> Result<()> {
    let contents = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    for line in contents.lines().skip(1) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 10 {
            continue;
        }
        let local = cols[1];
        let state = cols[3];
        let inode = cols[9];

        if protocol == Protocol::Tcp && state != "0A" {
            continue;
        }

        let Some((addr, port)) = parse_proc_addr(local) else {
            continue;
        };

        if !is_local_bind(&addr) {
            continue;
        }

        let pid = find_pid_for_inode(inode)?;
        if pid == 0 {
            continue;
        }

        out.push(RawListener {
            port,
            protocol,
            bind_address: addr,
            pid,
        });
    }
    Ok(())
}

fn parse_proc_addr(field: &str) -> Option<(String, u16)> {
    let (addr_hex, port_hex) = field.split_once(':')?;
    let port = u16::from_str_radix(port_hex, 16).ok()?;

    if addr_hex.len() == 8 {
        let ip = u32::from_str_radix(addr_hex, 16).ok()?;
        let addr = Ipv4Addr::from(ip.to_be()).to_string();
        Some((addr, port))
    } else if addr_hex.len() == 32 {
        let mut bytes = [0u8; 16];
        for (i, chunk) in addr_hex.as_bytes().chunks(8).enumerate() {
            let s = std::str::from_utf8(chunk).ok()?;
            let word = u32::from_str_radix(s, 16).ok()?;
            let be = word.to_be_bytes();
            bytes[i * 4..(i + 1) * 4].copy_from_slice(&be);
        }
        let addr = Ipv6Addr::from(bytes).to_string();
        Some((addr, port))
    } else {
        None
    }
}

fn find_pid_for_inode(inode: &str) -> Result<u32> {
    for entry in fs::read_dir("/proc").map_err(PerchError::Io)? {
        let entry = entry.map_err(PerchError::Io)?;
        let file_name = entry.file_name();
        let Ok(pid) = file_name.to_string_lossy().parse::<u32>() else {
            continue;
        };
        let fd_dir = entry.path().join("fd");
        let Ok(fds) = fs::read_dir(fd_dir) else {
            continue;
        };
        for fd in fds.flatten() {
            let link = fd.path();
            if let Ok(target) = fs::read_link(&link) {
                let target = target.to_string_lossy();
                if target.contains("socket:[") && target.contains(inode) {
                    return Ok(pid);
                }
            }
        }
    }
    Ok(0)
}

fn hydrate_procfs(pid: u32) -> Result<ProcessInfo> {
    let base = PathBuf::from(format!("/proc/{pid}"));
    let cmdline_path = base.join("cmdline");
    let cwd_path = base.join("cwd");
    let exe_path = base.join("exe");

    let cmdline = fs::read(&cmdline_path)
        .map(|bytes| {
            bytes
                .split(|&b| b == 0)
                .filter(|part| !part.is_empty())
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();

    let cwd = fs::read_link(&cwd_path)
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    let executable = fs::read_link(&exe_path)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| format!("pid:{pid}"));

    Ok(ProcessInfo {
        executable,
        cwd,
        cmdline,
        ..Default::default()
    })
}
