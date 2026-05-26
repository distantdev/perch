use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{PerchError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub scan: ScanConfig,
    #[serde(default)]
    pub filter: FilterConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanConfig {
    #[serde(default = "default_true")]
    pub loopback_only: bool,
    #[serde(default = "default_true")]
    pub tcp_only: bool,
    #[serde(default = "default_true")]
    pub dev_only: bool,
    #[serde(default = "default_false")]
    pub include_wildcard_bind: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilterConfig {
    #[serde(default = "default_ignore_ports")]
    pub ignore_ports: Vec<u16>,
    #[serde(default = "default_ignore_executables")]
    pub ignore_executables: Vec<String>,
    #[serde(default)]
    pub ignore_cmdline_patterns: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_ignore_ports() -> Vec<u16> {
    vec![
        22, 53, 135, 139, 445, 1900, 2869, 3702, 5040, 5050, 5353, 5357, 7680,
    ]
}

fn default_ignore_executables() -> Vec<String> {
    vec![
        "chrome".into(),
        "firefox".into(),
        "msedge".into(),
        "Code Helper".into(),
        "Code.exe".into(),
        "svchost".into(),
        "System".into(),
        "lsass".into(),
        "csrss".into(),
        "wininit".into(),
        "services".into(),
        "SearchHost".into(),
        "RuntimeBroker".into(),
        "dllhost".into(),
        "spoolsv".into(),
        "WmiPrvSE".into(),
        "MsMpEng".into(),
        "SecurityHealthService".into(),
        "mDNSResponder".into(),
        "OneDrive".into(),
    ]
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            loopback_only: true,
            tcp_only: true,
            dev_only: true,
            include_wildcard_bind: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scan: ScanConfig::default(),
            filter: FilterConfig {
                ignore_ports: default_ignore_ports(),
                ignore_executables: default_ignore_executables(),
                ignore_cmdline_patterns: vec!["devtools".into()],
            },
        }
    }
}

impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = path.map(PathBuf::from).unwrap_or_else(default_config_path);

        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let example = include_str!("../config.toml.example");
            let _ = fs::write(&path, example);
            return Ok(Config::default());
        }

        let contents = fs::read_to_string(&path)
            .map_err(|e| PerchError::Config(format!("read {}: {e}", path.display())))?;
        toml::from_str(&contents)
            .map_err(|e| PerchError::Config(format!("parse {}: {e}", path.display())))
    }

    pub fn with_show_all(mut self) -> Self {
        self.scan.loopback_only = false;
        self.scan.tcp_only = false;
        self.scan.dev_only = false;
        self.scan.include_wildcard_bind = true;
        self
    }
}

pub fn default_config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("perch")
        .join("config.toml")
}
