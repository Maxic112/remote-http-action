//! Client address resolution and network filters.

use std::net::IpAddr;

use ipnet::IpNet;

/// Real client IP. `X-Forwarded-For` is honoured only when the direct peer is a trusted proxy;
/// the header is walked right-to-left, skipping trusted proxies.
pub fn resolve_client_ip(peer: IpAddr, forwarded_for: Option<&str>, trusted: &[IpNet]) -> IpAddr {
    let peer = peer.to_canonical();
    if !contains(trusted, peer) {
        return peer;
    }
    let Some(header) = forwarded_for else {
        return peer;
    };
    let mut client = peer;
    for part in header.rsplit(',') {
        let Ok(ip) = part.trim().parse::<IpAddr>() else {
            break;
        };
        let ip = ip.to_canonical();
        client = ip;
        if !contains(trusted, ip) {
            break;
        }
    }
    client
}

pub fn contains(nets: &[IpNet], ip: IpAddr) -> bool {
    nets.iter().any(|n| n.contains(&ip))
}

/// Empty allow-list means everyone is allowed.
pub fn ip_allowed(ip: IpAddr, allowed: &[IpNet]) -> bool {
    allowed.is_empty() || contains(allowed, ip)
}

/// Strips the port from a `Host` header value (`host:port`, `[v6]:port`).
pub fn host_without_port(host: &str) -> &str {
    let host = host.trim();
    if let Some(rest) = host.strip_prefix('[') {
        return rest.split(']').next().unwrap_or(rest);
    }
    match host.rsplit_once(':') {
        Some((h, port)) if port.chars().all(|c| c.is_ascii_digit()) && !h.contains(':') => h,
        _ => host,
    }
}

/// Matches a `Host` header against the allow-list (`example.com`, `*.example.com`). Empty = any.
pub fn host_allowed(host: Option<&str>, allowed: &[String]) -> bool {
    if allowed.is_empty() {
        return true;
    }
    let Some(host) = host else { return false };
    let host = host_without_port(host).to_ascii_lowercase();
    allowed.iter().any(|pattern| {
        let p = pattern.trim().to_ascii_lowercase();
        match p.strip_prefix("*.") {
            Some(suffix) => host.ends_with(&format!(".{suffix}")),
            None => host == p,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::parse_nets;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn xff_is_ignored_from_untrusted_peers() {
        let trusted = parse_nets(&["10.0.0.1".into()]).unwrap();
        assert_eq!(
            resolve_client_ip(ip("1.2.3.4"), Some("9.9.9.9"), &trusted),
            ip("1.2.3.4")
        );
    }

    #[test]
    fn xff_is_used_from_trusted_proxy() {
        let trusted = parse_nets(&["10.0.0.0/8".into()]).unwrap();
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), Some("9.9.9.9"), &trusted),
            ip("9.9.9.9")
        );
        // Spoofed left-most entries are ignored: the right-most untrusted address wins.
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), Some("6.6.6.6, 9.9.9.9, 10.0.0.2"), &trusted),
            ip("9.9.9.9")
        );
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), None, &trusted),
            ip("10.0.0.1")
        );
        assert_eq!(
            resolve_client_ip(ip("10.0.0.1"), Some("garbage"), &trusted),
            ip("10.0.0.1")
        );
    }

    #[test]
    fn ipv4_mapped_addresses_are_canonicalised() {
        assert_eq!(
            resolve_client_ip(ip("::ffff:192.168.1.5"), None, &[]),
            ip("192.168.1.5")
        );
    }

    #[test]
    fn ip_allow_list() {
        let allowed = parse_nets(&["192.168.1.0/24".into(), "10.0.0.7".into()]).unwrap();
        assert!(ip_allowed(ip("192.168.1.200"), &allowed));
        assert!(ip_allowed(ip("10.0.0.7"), &allowed));
        assert!(!ip_allowed(ip("10.0.0.8"), &allowed));
        assert!(ip_allowed(ip("8.8.8.8"), &[]));
    }

    #[test]
    fn host_allow_list() {
        let allowed = vec!["pc.local".to_string(), "*.example.com".to_string()];
        assert!(host_allowed(Some("pc.local:8765"), &allowed));
        assert!(host_allowed(Some("PC.LOCAL"), &allowed));
        assert!(host_allowed(Some("a.example.com"), &allowed));
        assert!(!host_allowed(Some("example.com"), &allowed));
        assert!(!host_allowed(Some("evil.com"), &allowed));
        assert!(!host_allowed(None, &allowed));
        assert!(host_allowed(None, &[]));
        assert_eq!(host_without_port("[::1]:80"), "::1");
        assert_eq!(host_without_port("::1"), "::1");
    }
}
