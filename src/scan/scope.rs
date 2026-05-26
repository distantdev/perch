use crate::config::ScanConfig;
use crate::model::{Protocol, RawListener};
use crate::platform::{is_local_bind, is_loopback_bind, is_wildcard_bind};

pub fn filter_listeners(listeners: Vec<RawListener>, scan: &ScanConfig) -> Vec<RawListener> {
    listeners
        .into_iter()
        .filter(|l| listener_in_scope(l, scan))
        .collect()
}

pub fn listener_in_scope(listener: &RawListener, scan: &ScanConfig) -> bool {
    if scan.tcp_only && listener.protocol == Protocol::Udp {
        return false;
    }

    let bind = listener.bind_address.as_str();
    if scan.loopback_only {
        if is_loopback_bind(bind) {
            return true;
        }
        return scan.include_wildcard_bind && is_wildcard_bind(bind);
    }

    is_local_bind(bind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ScanConfig;
    use crate::model::Protocol;

    fn listener(addr: &str, protocol: Protocol) -> RawListener {
        RawListener {
            port: 3000,
            protocol,
            bind_address: addr.into(),
            pid: 1,
        }
    }

    #[test]
    fn default_scope_is_loopback_tcp_only() {
        let scan = ScanConfig::default();
        assert!(listener_in_scope(
            &listener("127.0.0.1", Protocol::Tcp),
            &scan
        ));
        assert!(!listener_in_scope(
            &listener("0.0.0.0", Protocol::Tcp),
            &scan
        ));
        assert!(!listener_in_scope(
            &listener("127.0.0.1", Protocol::Udp),
            &scan
        ));
    }

    #[test]
    fn wildcard_included_when_configured() {
        let scan = ScanConfig {
            include_wildcard_bind: true,
            ..ScanConfig::default()
        };
        assert!(listener_in_scope(
            &listener("0.0.0.0", Protocol::Tcp),
            &scan
        ));
    }
}
