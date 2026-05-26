use std::collections::HashMap;
use std::process::{Command, Stdio};

use crate::error::{PerchError, Result};

pub fn container_for_port(port: u16) -> Option<String> {
    fetch_docker_port_map()?.get(&port).cloned()
}

pub fn docker_stop(container: &str) -> Result<()> {
    run_docker(["stop", container], "stop")
}

pub fn docker_pause(container: &str) -> Result<()> {
    run_docker(["pause", container], "pause")
}

pub fn docker_unpause(container: &str) -> Result<()> {
    run_docker(["unpause", container], "unpause")
}

fn run_docker<const N: usize>(args: [&str; N], verb: &str) -> Result<()> {
    let output = Command::new("docker")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| {
            PerchError::ProcessControl(format!(
                "docker {verb}: {e} (is Docker running?)"
            ))
        })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr.trim();
    let message = if detail.is_empty() {
        format!("docker {verb} exited with {}", output.status)
    } else {
        format!("docker {verb}: {detail}")
    };
    Err(PerchError::ProcessControl(message))
}

pub fn fetch_docker_port_map() -> Option<HashMap<u16, String>> {
    let output = run_docker_ps()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut map = HashMap::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 3 {
            continue;
        }
        let name = parts[1].to_string();
        let ports_field = parts[2];
        for host_port in parse_docker_ports(ports_field) {
            map.insert(host_port, name.clone());
        }
    }

    Some(map)
}

fn run_docker_ps() -> Option<std::process::Output> {
    Command::new("docker")
        .args(["ps", "--format", "{{.ID}}\t{{.Names}}\t{{.Ports}}"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
}

fn parse_docker_ports(ports: &str) -> Vec<u16> {
    let mut out = Vec::new();
    for segment in ports.split(',') {
        let segment = segment.trim();
        if let Some(host_part) = segment.split("->").next() {
            if let Some(port_str) = host_part.rsplit(':').next() {
                if let Ok(port) = port_str.parse::<u16>() {
                    out.push(port);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_docker_ports_field() {
        let ports = "0.0.0.0:3000->3000/tcp, [::]:8080->80/tcp";
        let parsed = parse_docker_ports(ports);
        assert!(parsed.contains(&3000));
        assert!(parsed.contains(&8080));
    }

    #[test]
    fn parses_empty_ports() {
        assert!(parse_docker_ports("").is_empty());
    }

    #[test]
    fn container_for_port_missing_when_docker_unavailable() {
        assert!(container_for_port(59999).is_none());
    }
}
