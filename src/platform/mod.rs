#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod windows_cwd;

use crate::error::Result;
use crate::model::{ProcessInfo, RawListener};

pub trait PlatformScanner: Send + Sync {
    fn scan_listeners(&self) -> Result<Vec<RawListener>>;
    fn hydrate_process(&self, pid: u32) -> Result<ProcessInfo>;
}

pub fn default_scanner() -> Box<dyn PlatformScanner> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsScanner::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxScanner::new())
    }
    #[cfg(target_os = "macos")]
    {
        Box::new(darwin::DarwinScanner::new())
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        struct StubScanner;
        impl PlatformScanner for StubScanner {
            fn scan_listeners(&self) -> Result<Vec<RawListener>> {
                Ok(Vec::new())
            }
            fn hydrate_process(&self, _pid: u32) -> Result<ProcessInfo> {
                Ok(ProcessInfo::default())
            }
        }
        Box::new(StubScanner)
    }
}

pub fn is_loopback_bind(addr: &str) -> bool {
    let addr = addr.trim();
    addr == "127.0.0.1" || addr == "::1" || addr.starts_with("127.") || addr == "[::1]"
}

pub fn is_wildcard_bind(addr: &str) -> bool {
    let addr = addr.trim();
    addr == "0.0.0.0" || addr == "::" || addr == "::0" || addr == "*" || addr == "[::]"
}

pub fn is_local_bind(addr: &str) -> bool {
    is_loopback_bind(addr) || is_wildcard_bind(addr)
}
