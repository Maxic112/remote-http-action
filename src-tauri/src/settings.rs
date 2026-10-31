//! Application settings: defaults, validation and dotted-path updates.

use std::net::IpAddr;

use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Settings {
    pub server: ServerSettings,
    pub cli: CliSettings,
    pub app: AppSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ServerSettings {
    pub enabled: bool,
    pub bind_address: String,
    pub port: u16,
    /// Client IPs / CIDRs allowed to connect. Empty = everyone.
    pub allowed_ips: Vec<String>,
    /// Allowed values of the `Host` header (`example.com`, `*.example.com`). Empty = any.
    pub allowed_hosts: Vec<String>,
    /// Reverse proxies whose `X-Forwarded-For` header is trusted.
    pub trusted_proxies: Vec<String>,
    pub flow_prefix: String,
    pub allowed_path: String,
    pub status_path: String,
    /// Answer `/status/` without a key (minimal payload).
    pub public_status: bool,
    /// Answer 404 instead of 403 for flows the key has no access to.
    pub hide_forbidden_flows: bool,
    pub brute_force: BruteForceSettings,
}

impl Default for ServerSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            bind_address: "0.0.0.0".into(),
            port: 8765,
            allowed_ips: Vec::new(),
            allowed_hosts: Vec::new(),
            trusted_proxies: Vec::new(),
            flow_prefix: "/flow/".into(),
            allowed_path: "/get-allowed/".into(),
            status_path: "/status/".into(),
            public_status: true,
            hide_forbidden_flows: false,
            brute_force: BruteForceSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BruteForceSettings {
    pub enabled: bool,
    pub max_failures: u32,
    pub window_sec: u64,
    pub ban_sec: u64,
}

impl Default for BruteForceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_failures: 10,
            window_sec: 300,
            ban_sec: 900,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CliSettings {
    pub enabled: bool,
}

impl Default for CliSettings {
    fn default() -> Self {
        // Enabled in development builds, disabled in release builds.
        Self {
            enabled: cfg!(debug_assertions),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppSettings {
    /// Closing the window hides it to the tray instead of quitting.
    pub close_to_tray: bool,
    /// `system`, `ru` or `en`.
    pub language: String,
    pub run_history_limit: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            close_to_tray: true,
            language: "system".into(),
            run_history_limit: 200,
        }
    }
}

/// Paths that must never be changed through the CLI.
pub const CLI_PROTECTED_PATHS: &[&str] = &["cli", "cli.enabled"];

pub fn parse_net(entry: &str) -> AppResult<IpNet> {
    let entry = entry.trim();
    if let Ok(net) = entry.parse::<IpNet>() {
        return Ok(net.trunc());
    }
    entry
        .parse::<IpAddr>()
        .map(IpNet::from)
        .map_err(|_| AppError::validation(format!("invalid IP address or CIDR: {entry}")))
}

pub fn parse_nets(entries: &[String]) -> AppResult<Vec<IpNet>> {
    entries
        .iter()
        .filter(|e| !e.trim().is_empty())
        .map(|e| parse_net(e))
        .collect()
}

/// Normalises a URL prefix to the `/segment/` form.
pub fn normalize_prefix(p: &str) -> String {
    let trimmed = p.trim().trim_matches('/');
    if trimmed.is_empty() {
        "/".into()
    } else {
        format!("/{trimmed}/")
    }
}

impl Settings {
    /// Normalises prefixes and checks every field. Returns the normalised copy.
    pub fn validated(mut self) -> AppResult<Self> {
        let s = &mut self.server;
        s.bind_address = s.bind_address.trim().to_string();
        s.bind_address.parse::<IpAddr>().map_err(|_| {
            AppError::validation(format!("invalid bind address: {}", s.bind_address))
        })?;
        if s.port == 0 {
            return Err(AppError::validation("port must be between 1 and 65535"));
        }
        parse_nets(&s.allowed_ips)?;
        parse_nets(&s.trusted_proxies)?;
        for h in &s.allowed_hosts {
            let h = h.trim();
            if h.is_empty() || h.contains('/') || h.contains(' ') {
                return Err(AppError::validation(format!("invalid host: {h}")));
            }
        }
        s.flow_prefix = normalize_prefix(&s.flow_prefix);
        s.allowed_path = normalize_prefix(&s.allowed_path);
        s.status_path = normalize_prefix(&s.status_path);
        let prefixes = [
            &s.flow_prefix,
            &s.allowed_path,
            &s.status_path,
            &"/runs/".to_string(),
        ];
        for (i, a) in prefixes.iter().enumerate() {
            if a.as_str() == "/" {
                return Err(AppError::validation("URL prefixes must not be empty"));
            }
            for b in prefixes.iter().skip(i + 1) {
                if a.starts_with(b.as_str()) || b.starts_with(a.as_str()) {
                    return Err(AppError::validation(format!(
                        "URL prefixes overlap: {a} and {b}"
                    )));
                }
            }
        }
        if s.brute_force.max_failures == 0 || s.brute_force.window_sec == 0 {
            return Err(AppError::validation("brute force limits must be positive"));
        }
        if !["system", "ru", "en"].contains(&self.app.language.as_str()) {
            return Err(AppError::validation(
                "language must be one of: system, ru, en",
            ));
        }
        if self.app.run_history_limit == 0 {
            return Err(AppError::validation("run history limit must be positive"));
        }
        Ok(self)
    }

    /// Returns the value at a dotted path, e.g. `server.port`.
    pub fn get_path(&self, path: &str) -> AppResult<Value> {
        let root = serde_json::to_value(self).map_err(|e| AppError::internal(e.to_string()))?;
        if path.is_empty() {
            return Ok(root);
        }
        let mut cur = &root;
        for part in path.split('.') {
            cur = cur
                .get(part)
                .ok_or_else(|| AppError::not_found(format!("unknown setting: {path}")))?;
        }
        Ok(cur.clone())
    }

    /// Replaces the value at a dotted path and validates the result.
    pub fn with_path(&self, path: &str, value: Value) -> AppResult<Self> {
        let mut root = serde_json::to_value(self).map_err(|e| AppError::internal(e.to_string()))?;
        let mut cur = &mut root;
        let parts: Vec<&str> = path.split('.').collect();
        for part in &parts {
            cur = cur
                .get_mut(*part)
                .ok_or_else(|| AppError::not_found(format!("unknown setting: {path}")))?;
        }
        if std::mem::discriminant(cur) != std::mem::discriminant(&value) && !cur.is_null() {
            return Err(AppError::validation(format!("wrong value type for {path}")));
        }
        *cur = value;
        let parsed: Settings = serde_json::from_value(root)?;
        parsed.validated()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_are_valid() {
        let s = Settings::default().validated().unwrap();
        assert_eq!(s.server.flow_prefix, "/flow/");
        assert_eq!(s.server.port, 8765);
        assert!(s.server.enabled);
        assert_eq!(s.cli.enabled, cfg!(debug_assertions));
        assert!(s.app.close_to_tray);
    }

    #[test]
    fn prefixes_are_normalised() {
        let mut s = Settings::default();
        s.server.flow_prefix = "run".into();
        s.server.status_path = "/health".into();
        let s = s.validated().unwrap();
        assert_eq!(s.server.flow_prefix, "/run/");
        assert_eq!(s.server.status_path, "/health/");
    }

    #[test]
    fn overlapping_prefixes_are_rejected() {
        let mut s = Settings::default();
        s.server.allowed_path = "/flow/allowed".into();
        assert!(s.validated().is_err());
        let mut s = Settings::default();
        s.server.status_path = "/runs".into();
        assert!(s.validated().is_err());
    }

    #[test]
    fn invalid_networks_are_rejected() {
        let mut s = Settings::default();
        s.server.allowed_ips = vec!["192.168.1.0/24".into(), "10.0.0.5".into(), "::1".into()];
        assert!(s.clone().validated().is_ok());
        s.server.allowed_ips.push("nope".into());
        assert!(s.validated().is_err());
    }

    #[test]
    fn invalid_bind_address_and_port_are_rejected() {
        let mut s = Settings::default();
        s.server.bind_address = "localhost".into();
        assert!(s.validated().is_err());
        let mut s = Settings::default();
        s.server.port = 0;
        assert!(s.validated().is_err());
    }

    #[test]
    fn get_and_set_by_path() {
        let s = Settings::default();
        assert_eq!(s.get_path("server.port").unwrap(), json!(8765));
        let s2 = s.with_path("server.port", json!(9000)).unwrap();
        assert_eq!(s2.server.port, 9000);
        assert!(s.with_path("server.port", json!("x")).is_err());
        assert!(s.with_path("server.nope", json!(1)).is_err());
        assert!(s.with_path("server.port", json!(70000)).is_err());
    }

    #[test]
    fn missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_value(json!({"server": {"port": 1234}})).unwrap();
        assert_eq!(s.server.port, 1234);
        assert_eq!(s.server.flow_prefix, "/flow/");
    }
}
