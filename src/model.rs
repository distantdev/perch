use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ServerType {
    Node,
    DotNet,
    Python,
    Docker,
    Other,
}

impl ServerType {
    pub fn as_str(self) -> &'static str {
        match self {
            ServerType::Node => "Node",
            ServerType::DotNet => ".NET",
            ServerType::Python => "Python",
            ServerType::Docker => "Docker",
            ServerType::Other => "Other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DevServer {
    pub port: u16,
    pub protocol: Protocol,
    pub bind_address: String,
    pub pid: u32,
    pub executable: String,
    pub cwd: String,
    pub cmdline: String,
    pub cpu_percent: f32,
    pub memory_rss_bytes: u64,
    pub uptime_secs: u64,
    pub server_type: ServerType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docker_container: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RawListener {
    pub port: u16,
    pub protocol: Protocol,
    pub bind_address: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Default)]
pub struct ProcessInfo {
    pub executable: String,
    pub cwd: String,
    pub cmdline: String,
    pub cpu_percent: f32,
    pub memory_rss_bytes: u64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub version: String,
    pub scanned_at: DateTime<Utc>,
    pub servers: Vec<DevServer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl ScanResult {
    pub fn new(servers: Vec<DevServer>, errors: Vec<String>) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            scanned_at: Utc::now(),
            servers,
            errors,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Port,
    Type,
    Pid,
    Cpu,
    Memory,
    Uptime,
}

impl SortMode {
    pub fn cycle(self) -> Self {
        match self {
            SortMode::Port => SortMode::Type,
            SortMode::Type => SortMode::Pid,
            SortMode::Pid => SortMode::Cpu,
            SortMode::Cpu => SortMode::Memory,
            SortMode::Memory => SortMode::Uptime,
            SortMode::Uptime => SortMode::Port,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Port => "Port",
            SortMode::Type => "Type",
            SortMode::Pid => "PID",
            SortMode::Cpu => "CPU",
            SortMode::Memory => "MEM",
            SortMode::Uptime => "UPTIME",
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.0}MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0}KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes}B")
    }
}

pub fn format_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    let seconds = secs % 60;
    if days > 0 {
        format!("{days}d {hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    }
}
