use std::io::{self, Write};

use crate::config::Config;
use crate::error::Result;
use crate::model::{format_bytes, format_uptime, ScanResult};
use crate::platform::PlatformScanner;
use crate::scan::pipeline::run_scan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListFormat {
    Table,
    Json,
}

pub fn run_list(
    scanner: &dyn PlatformScanner,
    config: &Config,
    format: ListFormat,
    verbose: bool,
) -> Result<()> {
    let result = run_scan(scanner, config)?;

    if verbose {
        for err in &result.errors {
            eprintln!("warning: {err}");
        }
    }

    match format {
        ListFormat::Json => {
            let json = serde_json::to_string_pretty(&result)
                .map_err(|e| crate::error::PerchError::Scan(e.to_string()))?;
            println!("{json}");
        }
        ListFormat::Table => {
            print_table(&result);
        }
    }

    Ok(())
}

fn print_table(result: &ScanResult) {
    if result.servers.is_empty() {
        println!("No servers found.");
        println!("Tip: start a dev server, or run `perch list --all` for every local listener.");
        return;
    }

    println!(
        "{:<6} {:<10} {:<8} {:<40} {:<16} {:<8} {:<8} {:<12}",
        "PORT", "TYPE", "PID", "PATH", "EXE", "CPU", "MEM", "UPTIME"
    );
    println!("{}", "-".repeat(110));

    for s in &result.servers {
        let dir = truncate(&s.cwd, 40);
        let typ = s
            .docker_container
            .as_deref()
            .map(|c| format!("{} ({})", s.server_type.as_str(), c))
            .unwrap_or_else(|| s.server_type.as_str().to_string());

        println!(
            "{:<6} {:<10} {:<8} {:<40} {:<16} {:<7.1}% {:<8} {:<12}",
            s.port,
            truncate(&typ, 10),
            s.pid,
            dir,
            truncate(&s.executable, 16),
            s.cpu_percent,
            format_bytes(s.memory_rss_bytes),
            format_uptime(s.uptime_secs),
        );
    }

    let _ = io::stdout().flush();
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let end: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{end}...")
    }
}
