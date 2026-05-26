use crate::config::Config;
use crate::error::Result;
use crate::model::{DevServer, ScanResult, ServerType};
use crate::platform::PlatformScanner;
use crate::process::docker;
use crate::process::state::detect_runtime_state;
use crate::scan::classify::{classify_server, is_docker_proxy, is_likely_dev_server};
use crate::scan::filter::apply_filters;
use crate::scan::scope::filter_listeners;

pub fn run_scan(scanner: &dyn PlatformScanner, config: &Config) -> Result<ScanResult> {
    let listeners = scanner.scan_listeners()?;
    let listeners = filter_listeners(listeners, &config.scan);
    let docker_map = docker::fetch_docker_port_map();
    let paused_containers = docker::fetch_paused_containers();
    let mut servers = Vec::new();
    let mut errors = Vec::new();

    for listener in listeners {
        match scanner.hydrate_process(listener.pid) {
            Ok(info) => {
                let mut server_type = classify_server(&info.executable, &info.cmdline);
                let mut docker_container = None;
                let mut warnings = Vec::new();

                if is_docker_proxy(&info.executable) {
                    server_type = ServerType::Docker;
                    if let Some(ref map) = docker_map {
                        if let Some(name) = map.get(&listener.port) {
                            docker_container = Some(name.clone());
                        } else {
                            warnings.push("docker-proxy: no container mapping".into());
                        }
                    } else {
                        warnings.push("docker-proxy: docker CLI unavailable".into());
                    }
                }

                let runtime_state = detect_runtime_state(
                    listener.pid,
                    docker_container.as_deref(),
                    paused_containers.as_ref(),
                );

                servers.push(DevServer {
                    port: listener.port,
                    protocol: listener.protocol,
                    bind_address: listener.bind_address,
                    pid: listener.pid,
                    executable: info.executable,
                    cwd: info.cwd,
                    cmdline: info.cmdline,
                    cpu_percent: info.cpu_percent,
                    memory_rss_bytes: info.memory_rss_bytes,
                    uptime_secs: info.uptime_secs,
                    server_type,
                    runtime_state,
                    docker_container,
                    warnings,
                });
            }
            Err(e) => {
                errors.push(format!("pid {}: {e}", listener.pid));
            }
        }
    }

    servers.sort_by_key(|s| (s.port, s.pid));
    let servers = apply_filters(servers, config);
    let servers = if config.scan.dev_only {
        servers
            .into_iter()
            .filter(|s| is_likely_dev_server(s.server_type, &s.executable, &s.cmdline))
            .collect()
    } else {
        servers
    };
    Ok(ScanResult::new(servers, errors))
}

pub fn sort_servers(servers: &mut [DevServer], mode: crate::model::SortMode) {
    use crate::model::SortMode;
    servers.sort_by(|a, b| match mode {
        SortMode::Port => a.port.cmp(&b.port),
        SortMode::Type => a
            .server_type
            .as_str()
            .cmp(b.server_type.as_str())
            .then(a.port.cmp(&b.port)),
        SortMode::Pid => a.pid.cmp(&b.pid),
        SortMode::Cpu => b
            .cpu_percent
            .partial_cmp(&a.cpu_percent)
            .unwrap_or(std::cmp::Ordering::Equal),
        SortMode::Memory => b.memory_rss_bytes.cmp(&a.memory_rss_bytes),
        SortMode::Uptime => b.uptime_secs.cmp(&a.uptime_secs),
    });
}

pub fn filter_servers(servers: &[DevServer], query: &str) -> Vec<DevServer> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return servers.to_vec();
    }
    servers
        .iter()
        .filter(|s| {
            s.port.to_string().contains(&q)
                || s.executable.to_ascii_lowercase().contains(&q)
                || s.cmdline.to_ascii_lowercase().contains(&q)
                || s.cwd.to_ascii_lowercase().contains(&q)
                || s.server_type.as_str().to_ascii_lowercase().contains(&q)
                || s.docker_container
                    .as_ref()
                    .is_some_and(|c| c.to_ascii_lowercase().contains(&q))
        })
        .cloned()
        .collect()
}
