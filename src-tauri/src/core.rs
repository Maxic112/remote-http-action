//! Application core: one set of operations used by the UI, the CLI and the HTTP API.
//!
//! Every user-facing operation is a [`Request`] variant. The UI sends requests through the
//! `api` Tauri command, the CLI through the IPC channel, so both always have the same features.

use std::net::IpAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::actions::{self, ACTION_TYPES};
use crate::db::Db;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::{AppEvent, EventSink};
use crate::http::HttpServer;
use crate::http::guard::BruteForceGuard;
use crate::logs::{LogCategory, Logger};
use crate::model::*;
use crate::paths::AppPaths;
use crate::platform::Platform;
use crate::runner::{RunRequest, Runner};
use crate::secrets;
use crate::settings::{CLI_PROTECTED_PATHS, Settings};

/// Who sent a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caller {
    Ui,
    Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    KeysList,
    KeyCreate {
        name: String,
        #[serde(default)]
        group_ids: Vec<Id>,
    },
    KeyUpdate {
        id: Id,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        enabled: Option<bool>,
        #[serde(default)]
        group_ids: Option<Vec<Id>>,
    },
    KeyRotate {
        id: Id,
    },
    KeyDelete {
        id: Id,
    },
    GroupsList,
    GroupCreate {
        name: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        key_ids: Vec<Id>,
        #[serde(default)]
        flow_ids: Vec<Id>,
    },
    GroupUpdate {
        id: Id,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        key_ids: Option<Vec<Id>>,
        #[serde(default)]
        flow_ids: Option<Vec<Id>>,
    },
    GroupDelete {
        id: Id,
    },
    FlowsList,
    /// `flow` is an id or a path.
    FlowGet {
        flow: String,
    },
    FlowCreate {
        flow: FlowInput,
    },
    FlowUpdate {
        id: Id,
        flow: FlowInput,
    },
    FlowSetEnabled {
        flow: String,
        enabled: bool,
    },
    FlowDelete {
        flow: String,
    },
    FlowRun {
        flow: String,
        #[serde(default)]
        wait: bool,
    },
    RunsList {
        #[serde(default)]
        flow_id: Option<Id>,
        #[serde(default)]
        limit: Option<u32>,
    },
    RunGet {
        id: String,
    },
    RunCancel {
        id: String,
    },
    SettingsGet {
        #[serde(default)]
        path: Option<String>,
    },
    SettingsUpdate {
        settings: Settings,
    },
    SettingsSet {
        path: String,
        value: Value,
    },
    ServerStatus,
    ServerStart,
    ServerStop,
    BansList,
    BanRemove {
        ip: String,
    },
    SystemInfo,
    ActionTypes,
    AppShow,
    AppQuit,
    AppRestartElevated,
    LogsInfo,
    LogsRead {
        category: LogCategory,
        #[serde(default)]
        limit: Option<usize>,
        #[serde(default)]
        query: Option<String>,
    },
    LogsClear {
        category: LogCategory,
    },
    LogsOpenFolder,
    AutostartStatus,
    AutostartInstall,
    AutostartRemove,
    TlsInfo,
    /// Replaces the self-signed certificate and restarts an HTTPS server.
    TlsRegenerate,
}

/// UI-level operations implemented by the GUI shell.
pub trait AppControl: Send + Sync {
    fn show_window(&self);
    fn quit(&self);
}

pub struct NoopControl;

impl AppControl for NoopControl {
    fn show_window(&self) {}
    fn quit(&self) {}
}

pub struct Core {
    pub db: Arc<Db>,
    settings: RwLock<Settings>,
    pub platform: Arc<dyn Platform>,
    pub runner: Arc<Runner>,
    pub http: HttpServer,
    pub guard: BruteForceGuard,
    pub events: Arc<dyn EventSink>,
    pub control: Arc<dyn AppControl>,
    pub paths: AppPaths,
    pub logs: Arc<Logger>,
    started: Instant,
}

const MAX_NAME_LEN: usize = 200;
/// Seven days: enough for flows that wait for a long-running program.
pub const MAX_FLOW_TIMEOUT_SEC: u64 = 7 * 24 * 3600;

fn to_value<T: Serialize>(v: T) -> AppResult<Value> {
    serde_json::to_value(v).map_err(|e| AppError::internal(e.to_string()))
}

fn clean_name(name: &str, what: &str) -> AppResult<String> {
    let n = name.trim();
    if n.is_empty() {
        return Err(AppError::validation(format!(
            "{what} name must not be empty"
        )));
    }
    if n.chars().count() > MAX_NAME_LEN {
        return Err(AppError::validation(format!("{what} name is too long")));
    }
    Ok(n.to_string())
}

/// Normalises a flow path (`/Sleep/` → `sleep`) and checks its characters.
pub fn normalize_flow_path(path: &str) -> AppResult<String> {
    let p = path.trim().trim_matches('/').to_ascii_lowercase();
    if p.is_empty() {
        return Err(AppError::validation("flow path must not be empty"));
    }
    if p.len() > MAX_NAME_LEN {
        return Err(AppError::validation("flow path is too long"));
    }
    if p.contains("//")
        || !p
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '/'))
    {
        return Err(AppError::validation(
            "flow path may contain only a-z, 0-9, '-', '_' and '/'",
        ));
    }
    Ok(p)
}

pub fn validate_flow(mut f: FlowInput) -> AppResult<FlowInput> {
    f.name = clean_name(&f.name, "flow")?;
    f.description = f.description.trim().to_string();
    f.path = normalize_flow_path(&f.path)?;
    if !(1..=MAX_FLOW_TIMEOUT_SEC).contains(&f.timeout_sec) {
        return Err(AppError::validation(format!(
            "flow timeout must be between 1 and {MAX_FLOW_TIMEOUT_SEC} seconds"
        )));
    }
    if f.start_delay_sec > 3_600 {
        return Err(AppError::validation(
            "start delay must not exceed 3600 seconds",
        ));
    }
    if f.cooldown_sec > 86_400 {
        return Err(AppError::validation(
            "cooldown must not exceed 86400 seconds",
        ));
    }
    if f.actions.is_empty() {
        return Err(AppError::validation(
            "flow must contain at least one action",
        ));
    }
    for (i, a) in f.actions.iter().enumerate() {
        actions::parse(a)
            .map_err(|e| AppError::validation(format!("action {}: {}", i + 1, e.message)))?;
    }
    Ok(f)
}

impl Core {
    pub fn new(
        db: Arc<Db>,
        platform: Arc<dyn Platform>,
        events: Arc<dyn EventSink>,
        control: Arc<dyn AppControl>,
        paths: AppPaths,
    ) -> AppResult<Arc<Self>> {
        let settings = db.load_settings()?;
        db.fail_interrupted_runs()?;
        let logs = Arc::new(Logger::new(
            paths.data_dir.join("logs"),
            settings.logging.clone(),
        ));
        let runner = Arc::new(Runner::new(
            db.clone(),
            platform.clone(),
            events.clone(),
            logs.clone(),
            settings.app.run_history_limit,
        ));
        Ok(Arc::new(Self {
            db,
            settings: RwLock::new(settings),
            platform,
            runner,
            http: HttpServer::default(),
            guard: BruteForceGuard::default(),
            events,
            control,
            paths,
            logs,
            started: Instant::now(),
        }))
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    /// Starts the HTTP server if it is enabled in settings.
    pub async fn start_server_if_enabled(self: &Arc<Self>) {
        if self.settings().server.enabled {
            self.http.start(self.clone()).await;
        }
    }

    /// Validates, stores and applies new settings.
    pub async fn apply_settings(self: &Arc<Self>, new: Settings) -> AppResult<Settings> {
        let new = new.validated()?;
        let old = self.settings();
        self.db.save_settings(&new)?;
        *self.settings.write().unwrap() = new.clone();
        self.runner.set_history_limit(new.app.run_history_limit);
        self.logs.set_settings(new.logging.clone());
        let (o, n) = (&old.server, &new.server);
        if !n.enabled {
            if o.enabled {
                self.http.stop(self).await;
            }
        } else if !o.enabled
            || o.bind_address != n.bind_address
            || o.port != n.port
            || o.tls != n.tls
        {
            self.http.start(self.clone()).await;
        }
        self.changed("settings");
        Ok(new)
    }

    fn changed(&self, entity: &'static str) {
        self.events.emit(AppEvent::DataChanged { entity });
    }

    fn find_flow(&self, flow: &str) -> AppResult<Flow> {
        if let Ok(id) = flow.trim().parse::<Id>() {
            return self.db.get_flow(id);
        }
        let path = normalize_flow_path(flow)?;
        self.db
            .find_flow_by_path(&path)?
            .ok_or_else(|| AppError::not_found(format!("flow \"{path}\" not found")))
    }

    fn issue_key(&self, id: Id) -> AppResult<Value> {
        let secret = secrets::generate_key();
        self.db.replace_key_secret(
            id,
            &secrets::display_prefix(&secret),
            &secrets::hash_key(&secret),
        )?;
        to_value(IssuedKey {
            key: self.db.get_key(id)?,
            secret,
        })
    }

    pub fn system_info(&self) -> Value {
        let interfaces: Vec<Value> = if_addrs::get_if_addrs()
            .map(|list| {
                list.into_iter()
                    .filter(|i| i.ip().is_ipv4())
                    .map(|i| json!({ "name": i.name, "ip": i.ip().to_string(), "loopback": i.is_loopback() }))
                    .collect()
            })
            .unwrap_or_default();
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "build": if cfg!(debug_assertions) { "dev" } else { "release" },
            "elevated": self.platform.is_elevated(),
            "pid": std::process::id(),
            "paths": self.paths,
            "ipc_endpoint": self.paths.ipc_endpoint(),
            "shells": self.platform.shells(),
            "interfaces": interfaces,
        })
    }

    /// Executes a request on behalf of the UI or the CLI.
    pub async fn handle(self: &Arc<Self>, caller: Caller, req: Request) -> AppResult<Value> {
        use Request::*;
        match req {
            KeysList => to_value(self.db.list_keys()?),
            KeyCreate { name, group_ids } => {
                let name = clean_name(&name, "API key")?;
                let secret = secrets::generate_key();
                let id = self.db.insert_key(
                    &name,
                    &secrets::display_prefix(&secret),
                    &secrets::hash_key(&secret),
                    &group_ids,
                )?;
                self.changed("keys");
                to_value(IssuedKey {
                    key: self.db.get_key(id)?,
                    secret,
                })
            }
            KeyUpdate {
                id,
                name,
                enabled,
                group_ids,
            } => {
                let name = name.map(|n| clean_name(&n, "API key")).transpose()?;
                self.db
                    .update_key(id, name.as_deref(), enabled, group_ids.as_deref())?;
                self.changed("keys");
                to_value(self.db.get_key(id)?)
            }
            KeyRotate { id } => {
                let v = self.issue_key(id)?;
                self.changed("keys");
                Ok(v)
            }
            KeyDelete { id } => {
                self.db.delete_key(id)?;
                self.changed("keys");
                Ok(json!({ "deleted": id }))
            }

            GroupsList => to_value(self.db.list_groups()?),
            GroupCreate {
                name,
                description,
                key_ids,
                flow_ids,
            } => {
                let name = clean_name(&name, "group")?;
                let id = self
                    .db
                    .insert_group(&name, description.trim(), &key_ids, &flow_ids)?;
                self.changed("groups");
                to_value(self.db.get_group(id)?)
            }
            GroupUpdate {
                id,
                name,
                description,
                key_ids,
                flow_ids,
            } => {
                let name = name.map(|n| clean_name(&n, "group")).transpose()?;
                let description = description.map(|d| d.trim().to_string());
                self.db.update_group(
                    id,
                    name.as_deref(),
                    description.as_deref(),
                    key_ids.as_deref(),
                    flow_ids.as_deref(),
                )?;
                self.changed("groups");
                to_value(self.db.get_group(id)?)
            }
            GroupDelete { id } => {
                self.db.delete_group(id)?;
                self.changed("groups");
                Ok(json!({ "deleted": id }))
            }

            FlowsList => to_value(self.db.list_flows()?),
            FlowGet { flow } => to_value(self.find_flow(&flow)?),
            FlowCreate { flow } => {
                let input = validate_flow(flow)?;
                let id = self.db.insert_flow(&input)?;
                self.changed("flows");
                to_value(self.db.get_flow(id)?)
            }
            FlowUpdate { id, flow } => {
                let input = validate_flow(flow)?;
                self.db.update_flow(id, &input)?;
                self.changed("flows");
                to_value(self.db.get_flow(id)?)
            }
            FlowSetEnabled { flow, enabled } => {
                let mut f = self.find_flow(&flow)?;
                f.input.enabled = enabled;
                self.db.update_flow(f.id, &f.input)?;
                self.changed("flows");
                to_value(self.db.get_flow(f.id)?)
            }
            FlowDelete { flow } => {
                let f = self.find_flow(&flow)?;
                self.db.delete_flow(f.id)?;
                self.changed("flows");
                Ok(json!({ "deleted": f.id }))
            }
            FlowRun { flow, wait } => {
                let f = self.find_flow(&flow)?;
                let timeout =
                    Duration::from_secs(f.input.start_delay_sec + f.input.timeout_sec + 5);
                let source = if caller == Caller::Cli {
                    RunSource::Cli
                } else {
                    RunSource::Ui
                };
                let handle = self.runner.start(RunRequest {
                    flow: f,
                    source,
                    api_key: None,
                    client_ip: None,
                })?;
                let initial = handle.run.clone();
                if wait {
                    to_value(handle.wait(timeout).await.unwrap_or(initial))
                } else {
                    to_value(initial)
                }
            }

            RunsList { flow_id, limit } => {
                to_value(self.db.list_runs(flow_id, limit.unwrap_or(100).min(1000))?)
            }
            RunGet { id } => to_value(self.db.get_run(&id)?),
            RunCancel { id } => {
                // A person at the computer may cancel any run, including non-cancelable ones.
                self.runner.cancel(&id, caller == Caller::Ui)?;
                Ok(json!({ "run_id": id, "status": "canceling" }))
            }

            SettingsGet { path } => self.settings().get_path(path.as_deref().unwrap_or("")),
            SettingsUpdate { mut settings } => {
                if caller == Caller::Cli {
                    settings.cli = self.settings().cli;
                }
                to_value(self.apply_settings(settings).await?)
            }
            SettingsSet { path, value } => {
                let path = path.trim().to_string();
                if caller == Caller::Cli
                    && CLI_PROTECTED_PATHS
                        .iter()
                        .any(|p| path == *p || path.starts_with(&format!("{p}.")))
                {
                    return Err(AppError::new(
                        ErrorCode::Forbidden,
                        "CLI access can only be changed in the application window",
                    ));
                }
                let new = self.settings().with_path(&path, value)?;
                to_value(self.apply_settings(new).await?)
            }

            ServerStatus => to_value(self.http.status()),
            ServerStart => {
                let mut s = self.settings();
                s.server.enabled = true;
                self.apply_settings(s).await?;
                if self.http.status().state != crate::http::ServerState::Running {
                    self.http.start(self.clone()).await;
                }
                to_value(self.http.status())
            }
            ServerStop => {
                let mut s = self.settings();
                s.server.enabled = false;
                self.apply_settings(s).await?;
                to_value(self.http.status())
            }

            BansList => to_value(self.guard.list(Instant::now())),
            BanRemove { ip } => {
                let addr: IpAddr = ip
                    .trim()
                    .parse()
                    .map_err(|_| AppError::validation(format!("invalid IP address: {ip}")))?;
                if !self.guard.unban(addr) {
                    return Err(AppError::not_found(format!("{ip} is not banned")));
                }
                Ok(json!({ "unbanned": ip }))
            }

            SystemInfo => Ok(self.system_info()),
            ActionTypes => to_value(ACTION_TYPES),
            AppShow => {
                self.control.show_window();
                Ok(json!({ "ok": true }))
            }
            AppQuit => {
                self.control.quit();
                Ok(json!({ "ok": true }))
            }
            AppRestartElevated => {
                if self.platform.is_elevated() {
                    return Err(AppError::conflict(
                        "the application already runs as administrator",
                    ));
                }
                self.platform.relaunch_elevated()?;
                self.control.quit();
                Ok(json!({ "ok": true }))
            }

            LogsInfo => Ok(json!({
                "dir": self.logs.dir(),
                "categories": self.logs.info(),
            })),
            LogsRead {
                category,
                limit,
                query,
            } => {
                let limit = limit.unwrap_or(200).clamp(1, 5000);
                to_value(self.logs.read(category, limit, query.as_deref())?)
            }
            LogsClear { category } => {
                self.logs.clear(category)?;
                Ok(json!({ "cleared": category }))
            }
            LogsOpenFolder => {
                self.platform.open_folder(self.logs.dir())?;
                Ok(json!({ "ok": true }))
            }

            AutostartStatus => to_value(self.platform.autostart_status()?),
            AutostartInstall => {
                self.platform.autostart_install()?;
                to_value(self.platform.autostart_status()?)
            }
            AutostartRemove => {
                self.platform.autostart_remove()?;
                to_value(self.platform.autostart_status()?)
            }

            TlsInfo => to_value(crate::tls::info(
                &self.settings().server.tls,
                &self.paths.data_dir,
            )),
            TlsRegenerate => {
                let s = self.settings().server;
                if s.tls.mode != crate::settings::TlsMode::SelfSigned {
                    return Err(AppError::validation(
                        "only the self-signed certificate can be regenerated",
                    ));
                }
                crate::tls::generate_self_signed(&self.paths.data_dir)?;
                if s.enabled && s.tls.enabled {
                    self.http.start(self.clone()).await;
                }
                to_value(crate::tls::info(&s.tls, &self.paths.data_dir))
            }
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::events::NoopSink;
    use crate::platform::mock::MockPlatform;

    pub fn core_with(platform: MockPlatform) -> Arc<Core> {
        let db = Arc::new(Db::open_in_memory().unwrap());
        // Each test core gets its own data folder (logs, certificates).
        let dir = tempfile::tempdir().unwrap().keep();
        let paths = AppPaths::at(&dir, false);
        Core::new(
            db,
            Arc::new(platform),
            Arc::new(NoopSink),
            Arc::new(NoopControl),
            paths,
        )
        .unwrap()
    }

    pub fn sample_flow(path: &str) -> FlowInput {
        FlowInput {
            name: format!("Flow {path}"),
            path: path.into(),
            actions: vec![ActionDef {
                action_type: "delay".into(),
                params: json!({"seconds": 0}),
                on_error: OnError::Stop,
                timeout_sec: None,
                run_as_admin: false,
                capture_output: false,
            }],
            ..Default::default()
        }
    }

    #[test]
    fn flow_paths_are_normalised_and_checked() {
        assert_eq!(normalize_flow_path("/Sleep/").unwrap(), "sleep");
        assert_eq!(
            normalize_flow_path("pc/power-off_now").unwrap(),
            "pc/power-off_now"
        );
        assert!(normalize_flow_path("").is_err());
        assert!(normalize_flow_path("a//b").is_err());
        assert!(normalize_flow_path("a b").is_err());
        assert!(normalize_flow_path("../etc").is_err());
    }

    #[test]
    fn flows_are_validated() {
        assert!(validate_flow(sample_flow("ok")).is_ok());
        let mut f = sample_flow("x");
        f.actions.clear();
        assert!(validate_flow(f).is_err());
        let mut f = sample_flow("x");
        f.timeout_sec = 0;
        assert!(validate_flow(f).is_err());
        let mut f = sample_flow("x");
        f.name = "  ".into();
        assert!(validate_flow(f).is_err());
    }

    #[tokio::test]
    async fn key_lifecycle_through_requests() {
        let core = core_with(MockPlatform::default());
        let issued = core
            .handle(
                Caller::Ui,
                Request::KeyCreate {
                    name: " phone ".into(),
                    group_ids: vec![],
                },
            )
            .await
            .unwrap();
        let secret = issued["secret"].as_str().unwrap().to_string();
        let id = issued["key"]["id"].as_i64().unwrap();
        assert_eq!(issued["key"]["name"], "phone");
        assert!(secret.starts_with("rha_"));
        // The secret is never returned by list operations.
        let list = core.handle(Caller::Ui, Request::KeysList).await.unwrap();
        assert!(!list.to_string().contains(&secret));

        let rotated = core
            .handle(Caller::Ui, Request::KeyRotate { id })
            .await
            .unwrap();
        assert_ne!(rotated["secret"].as_str().unwrap(), secret);
        assert!(
            core.db
                .find_key_by_hash(&secrets::hash_key(&secret))
                .unwrap()
                .is_none()
        );

        core.handle(
            Caller::Ui,
            Request::KeyUpdate {
                id,
                name: None,
                enabled: Some(false),
                group_ids: None,
            },
        )
        .await
        .unwrap();
        assert!(!core.db.get_key(id).unwrap().enabled);
        core.handle(Caller::Ui, Request::KeyDelete { id })
            .await
            .unwrap();
        assert!(
            core.handle(Caller::Ui, Request::KeyDelete { id })
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn flows_and_groups_through_requests() {
        let core = core_with(MockPlatform::default());
        let flow = core
            .handle(
                Caller::Ui,
                Request::FlowCreate {
                    flow: sample_flow("/Sleep/"),
                },
            )
            .await
            .unwrap();
        assert_eq!(flow["path"], "sleep");
        let flow_id = flow["id"].as_i64().unwrap();
        let group = core
            .handle(
                Caller::Ui,
                Request::GroupCreate {
                    name: "ops".into(),
                    description: String::new(),
                    key_ids: vec![],
                    flow_ids: vec![flow_id],
                },
            )
            .await
            .unwrap();
        assert_eq!(group["flow_ids"], json!([flow_id]));

        let disabled = core
            .handle(
                Caller::Cli,
                Request::FlowSetEnabled {
                    flow: "sleep".into(),
                    enabled: false,
                },
            )
            .await
            .unwrap();
        assert_eq!(disabled["enabled"], false);
        assert!(
            core.handle(
                Caller::Cli,
                Request::FlowRun {
                    flow: "sleep".into(),
                    wait: true
                }
            )
            .await
            .is_err()
        );

        core.handle(
            Caller::Cli,
            Request::FlowSetEnabled {
                flow: flow_id.to_string(),
                enabled: true,
            },
        )
        .await
        .unwrap();
        let run = core
            .handle(
                Caller::Cli,
                Request::FlowRun {
                    flow: "sleep".into(),
                    wait: true,
                },
            )
            .await
            .unwrap();
        assert_eq!(run["status"], "success");
        assert_eq!(run["source"], "cli");
    }

    #[tokio::test]
    async fn cli_cannot_change_cli_access() {
        let core = core_with(MockPlatform::default());
        let before = core.settings().cli.enabled;
        let err = core
            .handle(
                Caller::Cli,
                Request::SettingsSet {
                    path: "cli.enabled".into(),
                    value: json!(!before),
                },
            )
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Forbidden);

        let mut s = core.settings();
        s.cli.enabled = !before;
        s.app.run_history_limit = 50;
        core.handle(
            Caller::Cli,
            Request::SettingsUpdate {
                settings: s.clone(),
            },
        )
        .await
        .unwrap();
        assert_eq!(core.settings().cli.enabled, before);
        assert_eq!(core.settings().app.run_history_limit, 50);

        core.handle(Caller::Ui, Request::SettingsUpdate { settings: s })
            .await
            .unwrap();
        assert_eq!(core.settings().cli.enabled, !before);
    }

    #[tokio::test]
    async fn settings_set_by_path_and_persisted() {
        let core = core_with(MockPlatform::default());
        let mut s = core.settings();
        s.server.enabled = false;
        core.apply_settings(s).await.unwrap();
        core.handle(
            Caller::Cli,
            Request::SettingsSet {
                path: "server.port".into(),
                value: json!(9100),
            },
        )
        .await
        .unwrap();
        assert_eq!(core.settings().server.port, 9100);
        assert_eq!(core.db.load_settings().unwrap().server.port, 9100);
        let v = core
            .handle(
                Caller::Cli,
                Request::SettingsGet {
                    path: Some("server.port".into()),
                },
            )
            .await
            .unwrap();
        assert_eq!(v, json!(9100));
    }

    #[tokio::test]
    async fn restart_elevated_is_refused_when_already_elevated() {
        let core = core_with(MockPlatform {
            elevated: true,
            ..Default::default()
        });
        let err = core
            .handle(Caller::Ui, Request::AppRestartElevated)
            .await
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::Conflict);
    }

    #[test]
    fn request_json_shape_is_stable() {
        let r: Request = serde_json::from_value(json!({"op": "key_create", "name": "a"})).unwrap();
        assert_eq!(
            r,
            Request::KeyCreate {
                name: "a".into(),
                group_ids: vec![]
            }
        );
        let r: Request =
            serde_json::from_value(json!({"op": "flow_run", "flow": "sleep"})).unwrap();
        assert_eq!(
            r,
            Request::FlowRun {
                flow: "sleep".into(),
                wait: false
            }
        );
    }
}
