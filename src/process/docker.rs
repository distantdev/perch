use std::collections::HashMap;
use std::process::{Command, Stdio};

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
}
