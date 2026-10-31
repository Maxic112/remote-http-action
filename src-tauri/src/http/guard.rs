//! Temporary IP bans after repeated authentication failures.

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::settings::BruteForceSettings;

#[derive(Default)]
struct Entry {
    failures: VecDeque<Instant>,
    banned_until: Option<Instant>,
}

#[derive(Default)]
pub struct BruteForceGuard {
    entries: Mutex<HashMap<IpAddr, Entry>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Ban {
    pub ip: String,
    pub seconds_left: u64,
}

impl BruteForceGuard {
    /// Remaining ban time, if the address is banned.
    pub fn banned(&self, ip: IpAddr, now: Instant) -> Option<Duration> {
        let entries = self.entries.lock().unwrap();
        entries
            .get(&ip)?
            .banned_until
            .filter(|u| *u > now)
            .map(|u| u - now)
    }

    /// Records a failed authentication. Returns true if the address got banned.
    pub fn record_failure(&self, ip: IpAddr, cfg: &BruteForceSettings, now: Instant) -> bool {
        if !cfg.enabled {
            return false;
        }
        let mut entries = self.entries.lock().unwrap();
        let e = entries.entry(ip).or_default();
        let window = Duration::from_secs(cfg.window_sec);
        while e
            .failures
            .front()
            .is_some_and(|t| now.duration_since(*t) > window)
        {
            e.failures.pop_front();
        }
        e.failures.push_back(now);
        if e.failures.len() as u32 >= cfg.max_failures {
            e.failures.clear();
            e.banned_until = Some(now + Duration::from_secs(cfg.ban_sec));
            return true;
        }
        false
    }

    pub fn list(&self, now: Instant) -> Vec<Ban> {
        let mut entries = self.entries.lock().unwrap();
        entries.retain(|_, e| !e.failures.is_empty() || e.banned_until.is_some_and(|u| u > now));
        let mut bans: Vec<Ban> = entries
            .iter()
            .filter_map(|(ip, e)| {
                e.banned_until.filter(|u| *u > now).map(|u| Ban {
                    ip: ip.to_string(),
                    seconds_left: (u - now).as_secs(),
                })
            })
            .collect();
        bans.sort_by(|a, b| a.ip.cmp(&b.ip));
        bans
    }

    pub fn unban(&self, ip: IpAddr) -> bool {
        self.entries.lock().unwrap().remove(&ip).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> BruteForceSettings {
        BruteForceSettings {
            enabled: true,
            max_failures: 3,
            window_sec: 60,
            ban_sec: 600,
        }
    }

    #[test]
    fn bans_after_max_failures_within_window() {
        let g = BruteForceGuard::default();
        let ip: IpAddr = "1.1.1.1".parse().unwrap();
        let t0 = Instant::now();
        assert!(!g.record_failure(ip, &cfg(), t0));
        assert!(!g.record_failure(ip, &cfg(), t0));
        assert!(g.banned(ip, t0).is_none());
        assert!(g.record_failure(ip, &cfg(), t0));
        assert!(g.banned(ip, t0 + Duration::from_secs(10)).is_some());
        assert!(g.banned(ip, t0 + Duration::from_secs(601)).is_none());
        assert_eq!(g.list(t0).len(), 1);
        assert!(g.unban(ip));
        assert!(g.banned(ip, t0).is_none());
    }

    #[test]
    fn old_failures_expire() {
        let g = BruteForceGuard::default();
        let ip: IpAddr = "1.1.1.1".parse().unwrap();
        let t0 = Instant::now();
        g.record_failure(ip, &cfg(), t0);
        g.record_failure(ip, &cfg(), t0);
        assert!(!g.record_failure(ip, &cfg(), t0 + Duration::from_secs(120)));
        assert!(g.banned(ip, t0 + Duration::from_secs(120)).is_none());
    }

    #[test]
    fn disabled_guard_never_bans() {
        let g = BruteForceGuard::default();
        let ip: IpAddr = "1.1.1.1".parse().unwrap();
        let c = BruteForceSettings {
            enabled: false,
            ..cfg()
        };
        for _ in 0..10 {
            assert!(!g.record_failure(ip, &c, Instant::now()));
        }
    }
}
