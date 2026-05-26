use crate::config::Config;
use crate::model::DevServer;

pub fn apply_filters(servers: Vec<DevServer>, config: &Config) -> Vec<DevServer> {
    servers
        .into_iter()
        .filter(|s| !should_ignore(s, config))
        .collect()
}

fn should_ignore(server: &DevServer, config: &Config) -> bool {
    let filter = &config.filter;

    if filter.ignore_ports.contains(&server.port) {
        return true;
    }

    let exe_lower = server.executable.to_ascii_lowercase();
    for ignored in &filter.ignore_executables {
        if exe_lower.contains(&ignored.to_ascii_lowercase()) {
            return true;
        }
    }

    let cmd_lower = server.cmdline.to_ascii_lowercase();
    for pattern in &filter.ignore_cmdline_patterns {
        if cmd_lower.contains(&pattern.to_ascii_lowercase()) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::model::{DevServer, Protocol, ServerType};

    fn sample(port: u16, exe: &str, cmd: &str) -> DevServer {
        DevServer {
            port,
            protocol: Protocol::Tcp,
            bind_address: "127.0.0.1".into(),
            pid: 1,
            executable: exe.into(),
            cwd: "/tmp".into(),
            cmdline: cmd.into(),
            cpu_percent: 0.0,
            memory_rss_bytes: 0,
            uptime_secs: 0,
            server_type: ServerType::Other,
            docker_container: None,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn ignores_configured_port() {
        let mut config = Config::default();
        config.filter.ignore_ports = vec![3000];
        let servers = vec![sample(3000, "node", ""), sample(4000, "node", "")];
        let filtered = apply_filters(servers, &config);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].port, 4000);
    }

    #[test]
    fn ignores_executable_substring() {
        let mut config = Config::default();
        config.filter.ignore_executables = vec!["chrome".into()];
        let servers = vec![sample(9222, "Google Chrome", ""), sample(3000, "node", "")];
        let filtered = apply_filters(servers, &config);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].port, 3000);
    }

    #[test]
    fn ignores_cmdline_pattern() {
        let mut config = Config::default();
        config.filter.ignore_cmdline_patterns = vec!["devtools".into()];
        let servers = vec![
            sample(9222, "chrome", "chrome --devtools"),
            sample(3000, "node", "vite"),
        ];
        let filtered = apply_filters(servers, &config);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].port, 3000);
    }
}
