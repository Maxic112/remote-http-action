//! Domain types shared by the database, services, HTTP API, CLI and UI.

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type Id = i64;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiKey {
    pub id: Id,
    pub name: String,
    pub key_prefix: String,
    pub enabled: bool,
    pub group_ids: Vec<Id>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

/// Returned once, when a key is created or rotated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssuedKey {
    pub key: ApiKey,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccessGroup {
    pub id: Id,
    pub name: String,
    pub description: String,
    pub key_ids: Vec<Id>,
    pub flow_ids: Vec<Id>,
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ResponseMode {
    #[default]
    Sync,
    Async,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Concurrency {
    Allow,
    #[default]
    RejectIfRunning,
    Queue,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnError {
    #[default]
    Stop,
    Continue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ClientMeta {
    pub icon: Option<String>,
    pub color: Option<String>,
    pub category: Option<String>,
    pub tags: Vec<String>,
    pub confirm_on_client: bool,
}

/// One step of a flow. `params` depend on `action_type`, see [`crate::actions`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionDef {
    #[serde(rename = "type")]
    pub action_type: String,
    #[serde(default = "empty_object")]
    pub params: Value,
    #[serde(default)]
    pub on_error: OnError,
    #[serde(default)]
    pub timeout_sec: Option<u64>,
    #[serde(default)]
    pub run_as_admin: bool,
    #[serde(default)]
    pub capture_output: bool,
}

fn empty_object() -> Value {
    Value::Object(Default::default())
}

/// Editable part of a flow (what UI/CLI send on create/update).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct FlowInput {
    pub name: String,
    pub description: String,
    pub path: String,
    pub enabled: bool,
    pub response_mode: ResponseMode,
    pub timeout_sec: u64,
    pub concurrency: Concurrency,
    pub cooldown_sec: u64,
    pub start_delay_sec: u64,
    pub cancelable: bool,
    pub client_meta: ClientMeta,
    /// Reserved for request parameters; not used yet.
    pub input_schema: Option<Value>,
    pub actions: Vec<ActionDef>,
}

impl Default for FlowInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            path: String::new(),
            enabled: true,
            response_mode: ResponseMode::Sync,
            timeout_sec: 60,
            concurrency: Concurrency::RejectIfRunning,
            cooldown_sec: 0,
            start_delay_sec: 0,
            cancelable: true,
            client_meta: ClientMeta::default(),
            input_schema: None,
            actions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Flow {
    pub id: Id,
    #[serde(flatten)]
    pub input: FlowInput,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Pending,
    Delayed,
    Running,
    Success,
    Failed,
    Canceled,
    Timeout,
}

impl RunStatus {
    pub fn is_finished(self) -> bool {
        matches!(
            self,
            RunStatus::Success | RunStatus::Failed | RunStatus::Canceled | RunStatus::Timeout
        )
    }
    pub fn as_str(self) -> &'static str {
        match self {
            RunStatus::Pending => "pending",
            RunStatus::Delayed => "delayed",
            RunStatus::Running => "running",
            RunStatus::Success => "success",
            RunStatus::Failed => "failed",
            RunStatus::Canceled => "canceled",
            RunStatus::Timeout => "timeout",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "pending" => RunStatus::Pending,
            "delayed" => RunStatus::Delayed,
            "running" => RunStatus::Running,
            "success" => RunStatus::Success,
            "canceled" => RunStatus::Canceled,
            "timeout" => RunStatus::Timeout,
            _ => RunStatus::Failed,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Success,
    Failed,
    Skipped,
    Canceled,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StepResult {
    pub index: usize,
    #[serde(rename = "type")]
    pub action_type: String,
    pub status: StepStatus,
    pub started_at: String,
    pub finished_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Whether stdout/stderr may be returned to HTTP clients.
    #[serde(default)]
    pub capture_output: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunSource {
    Http,
    Cli,
    Ui,
}

impl RunSource {
    pub fn as_str(self) -> &'static str {
        match self {
            RunSource::Http => "http",
            RunSource::Cli => "cli",
            RunSource::Ui => "ui",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "http" => RunSource::Http,
            "cli" => RunSource::Cli,
            _ => RunSource::Ui,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Run {
    pub id: String,
    pub flow_id: Option<Id>,
    pub flow_name: String,
    pub flow_path: String,
    pub source: RunSource,
    pub api_key_id: Option<Id>,
    pub api_key_name: Option<String>,
    pub client_ip: Option<String>,
    pub status: RunStatus,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
    pub steps: Vec<StepResult>,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
