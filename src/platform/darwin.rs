use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use sysinfo::{Pid, ProcessesToUpdate, System};

use crate::error::Result;
use crate::model::{ProcessInfo, Protocol, RawListener};
use crate::platform::{is_local_bind, PlatformScanner};
use crate::scan::project::infer_project_path;

pub struct DarwinScanner;

impl DarwinScanner {
    pub fn new() -> Self {
        Self
    }
}

impl PlatformScanner for DarwinScanner {
    fn scan_listeners(&self) -> Result<Vec<RawListener>> {
        scan_via_lsof()
    }

    fn hydrate_process(&self, pid: u32) -> Result<ProcessInfo> {
        let mut system = System::new();
        system.refresh_processes(ProcessesToUpdate::All, true);

        let mut info = hydrate_proc_path(pid).unwrap_or_default();

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

fn scan_via_lsof() -> Result<Vec<RawListener>> {
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcn"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();

    let Ok(output) = output else {
        return Ok(Vec::new());
    };

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let reader = BufReader::new(output.stdout.as_slice());
    let mut listeners = Vec::new();
    let mut pid: u32 = 0;
    let mut port: Option<u16> = None;
    let mut bind = String::new();

    for line in reader.lines().map_while(|line| line.ok()) {
        if line.is_empty() {
            if pid > 0 {
                if let Some(p) = port {
                    if is_local_bind(&bind) || bind.is_empty() {
                        listeners.push(RawListener {
                            port: p,
                            protocol: Protocol::Tcp,
                            bind_address: if bind.is_empty() {
                                "127.0.0.1".into()
                            } else {
                                bind.clone()
                            },
                            pid,
                        });
                    }
                }
            }
            pid = 0;
            port = None;
            bind.clear();
            continue;
        }

        let (tag, value) = line.split_at(1);
        match tag {
            "p" => pid = value.parse().unwrap_or(0),
            "n" => {
                if let Some((host, p)) = parse_lsof_name(value) {
                    bind = host;
                    port = Some(p);
                }
            }
            _ => {}
        }
    }

    Ok(listeners)
}

fn parse_lsof_name(value: &str) -> Option<(String, u16)> {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('[') {
        if let Some((host, port_str)) = rest.split_once("]:") {
            let port = port_str.parse().ok()?;
            return Some((host.to_string(), port));
        }
    }
    if let Some((host, port_str)) = value.rsplit_once(':') {
        let port = port_str.parse().ok()?;
        let host = if host == "*" {
            "0.0.0.0".to_string()
        } else {
            host.to_string()
        };
        return Some((host, port));
    }
    None
}

fn hydrate_proc_path(pid: u32) -> Result<ProcessInfo> {
    let mut info = ProcessInfo::default();

    #[cfg(target_os = "macos")]
    {
        use std::os::raw::c_char;

        extern "C" {
            fn proc_pidpath(pid: i32, buffer: *mut c_char, buffer_size: u32) -> i32;
        }

        let mut buf = [0i8; 4096];
        let ret = unsafe { proc_pidpath(pid as i32, buf.as_mut_ptr(), buf.len() as u32) };
        if ret > 0 {
            let path = unsafe {
                std::ffi::CStr::from_ptr(buf.as_ptr())
                    .to_string_lossy()
                    .into_owned()
            };
            let path_obj = std::path::Path::new(&path);
            info.cwd = path_obj
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            info.executable = path_obj
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(path);
        }
    }

    Ok(info)
}
