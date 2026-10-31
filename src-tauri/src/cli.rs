//! Command-line interface. Arguments are parsed locally and sent to the running instance.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use serde_json::Value;

use crate::core::Request;
use crate::error::{AppError, ErrorCode};
use crate::ipc::{self, IpcRequest};
use crate::model::{FlowInput, Id};
use crate::paths::AppPaths;

#[derive(Debug, Parser)]
#[command(
    name = "remote-http-action",
    version,
    about = "Controls a running remote-http-action instance",
    after_help = "Run without arguments to start the application."
)]
pub struct Cli {
    /// Print raw JSON instead of tables
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Manage API keys
    #[command(subcommand)]
    Keys(KeysCmd),
    /// Manage access groups
    #[command(subcommand)]
    Groups(GroupsCmd),
    /// Manage and run flows
    #[command(subcommand)]
    Flows(FlowsCmd),
    /// Inspect and cancel flow runs
    #[command(subcommand)]
    Runs(RunsCmd),
    /// Read and change settings
    #[command(subcommand)]
    Settings(SettingsCmd),
    /// Control the HTTP server
    #[command(subcommand)]
    Server(ServerCmd),
    /// Manage banned client addresses
    #[command(subcommand)]
    Bans(BansCmd),
    /// System information
    #[command(subcommand)]
    System(SystemCmd),
    /// Control the application window and process
    #[command(subcommand)]
    App(AppCmd),
}

#[derive(Debug, Subcommand)]
pub enum KeysCmd {
    /// List API keys
    List,
    /// Create a key; the secret is printed once
    Create {
        name: String,
        /// Group ids, comma separated
        #[arg(long, value_delimiter = ',')]
        groups: Vec<Id>,
    },
    /// Change name, state or groups of a key
    Update(KeyUpdateArgs),
    /// Issue a new secret for a key; the old one stops working
    Rotate { id: Id },
    /// Delete a key
    Delete { id: Id },
}

#[derive(Debug, Args)]
pub struct KeyUpdateArgs {
    pub id: Id,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long, conflicts_with = "disable")]
    pub enable: bool,
    #[arg(long)]
    pub disable: bool,
    /// Replace groups (comma separated ids)
    #[arg(long, value_delimiter = ',', conflicts_with = "no_groups")]
    pub groups: Option<Vec<Id>>,
    /// Remove the key from all groups
    #[arg(long)]
    pub no_groups: bool,
}

#[derive(Debug, Subcommand)]
pub enum GroupsCmd {
    /// List access groups
    List,
    /// Create a group
    Create {
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        /// Key ids, comma separated
        #[arg(long, value_delimiter = ',')]
        keys: Vec<Id>,
        /// Flow ids, comma separated
        #[arg(long, value_delimiter = ',')]
        flows: Vec<Id>,
    },
    /// Change a group
    Update(GroupUpdateArgs),
    /// Delete a group
    Delete { id: Id },
}

#[derive(Debug, Args)]
pub struct GroupUpdateArgs {
    pub id: Id,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long, value_delimiter = ',', conflicts_with = "no_keys")]
    pub keys: Option<Vec<Id>>,
    #[arg(long)]
    pub no_keys: bool,
    #[arg(long, value_delimiter = ',', conflicts_with = "no_flows")]
    pub flows: Option<Vec<Id>>,
    #[arg(long)]
    pub no_flows: bool,
}

#[derive(Debug, Subcommand)]
pub enum FlowsCmd {
    /// List flows
    List,
    /// Show a flow (by id or path) as JSON
    Get { flow: String },
    /// Create a flow from a JSON file
    Create {
        #[arg(long)]
        file: PathBuf,
    },
    /// Replace a flow with the contents of a JSON file
    Update {
        id: Id,
        #[arg(long)]
        file: PathBuf,
    },
    /// Enable a flow
    Enable { flow: String },
    /// Disable a flow
    Disable { flow: String },
    /// Delete a flow
    Delete { flow: String },
    /// Run a flow
    Run {
        flow: String,
        /// Wait for the run to finish
        #[arg(long)]
        wait: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum RunsCmd {
    /// List recent runs
    List {
        #[arg(long)]
        flow: Option<Id>,
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },
    /// Show a run
    Get { id: String },
    /// Cancel a pending or running run
    Cancel { id: String },
}

#[derive(Debug, Subcommand)]
pub enum SettingsCmd {
    /// Show all settings or one value (dotted path, e.g. server.port)
    Get { path: Option<String> },
    /// Set a value; VALUE is parsed as JSON, otherwise taken as a string
    Set { path: String, value: String },
}

#[derive(Debug, Subcommand)]
pub enum ServerCmd {
    /// Show HTTP server state
    Status,
    /// Enable and start the HTTP server
    Start,
    /// Stop and disable the HTTP server
    Stop,
}

#[derive(Debug, Subcommand)]
pub enum BansCmd {
    /// List banned addresses
    List,
    /// Lift a ban
    Remove { ip: String },
}

#[derive(Debug, Subcommand)]
pub enum SystemCmd {
    /// Version, rights, paths, shells and network interfaces
    Info,
    /// Supported action types
    ActionTypes,
}

#[derive(Debug, Subcommand)]
pub enum AppCmd {
    /// Show the application window
    Show,
    /// Quit the application
    Quit,
    /// Restart the application with administrator rights (UAC prompt on the desktop)
    RestartElevated,
}

fn read_flow(path: &PathBuf) -> Result<FlowInput, AppError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppError::validation(format!("cannot read {}: {e}", path.display())))?;
    serde_json::from_str(&text)
        .map_err(|e| AppError::validation(format!("invalid flow JSON in {}: {e}", path.display())))
}

fn ids_option(ids: Option<Vec<Id>>, clear: bool) -> Option<Vec<Id>> {
    if clear { Some(Vec::new()) } else { ids }
}

/// Converts parsed arguments into a core request.
pub fn to_request(cmd: Command) -> Result<Request, AppError> {
    Ok(match cmd {
        Command::Keys(c) => match c {
            KeysCmd::List => Request::KeysList,
            KeysCmd::Create { name, groups } => Request::KeyCreate {
                name,
                group_ids: groups,
            },
            KeysCmd::Update(a) => Request::KeyUpdate {
                id: a.id,
                name: a.name,
                enabled: if a.enable {
                    Some(true)
                } else if a.disable {
                    Some(false)
                } else {
                    None
                },
                group_ids: ids_option(a.groups, a.no_groups),
            },
            KeysCmd::Rotate { id } => Request::KeyRotate { id },
            KeysCmd::Delete { id } => Request::KeyDelete { id },
        },
        Command::Groups(c) => match c {
            GroupsCmd::List => Request::GroupsList,
            GroupsCmd::Create {
                name,
                description,
                keys,
                flows,
            } => Request::GroupCreate {
                name,
                description,
                key_ids: keys,
                flow_ids: flows,
            },
            GroupsCmd::Update(a) => Request::GroupUpdate {
                id: a.id,
                name: a.name,
                description: a.description,
                key_ids: ids_option(a.keys, a.no_keys),
                flow_ids: ids_option(a.flows, a.no_flows),
            },
            GroupsCmd::Delete { id } => Request::GroupDelete { id },
        },
        Command::Flows(c) => match c {
            FlowsCmd::List => Request::FlowsList,
            FlowsCmd::Get { flow } => Request::FlowGet { flow },
            FlowsCmd::Create { file } => Request::FlowCreate {
                flow: read_flow(&file)?,
            },
            FlowsCmd::Update { id, file } => Request::FlowUpdate {
                id,
                flow: read_flow(&file)?,
            },
            FlowsCmd::Enable { flow } => Request::FlowSetEnabled {
                flow,
                enabled: true,
            },
            FlowsCmd::Disable { flow } => Request::FlowSetEnabled {
                flow,
                enabled: false,
            },
            FlowsCmd::Delete { flow } => Request::FlowDelete { flow },
            FlowsCmd::Run { flow, wait } => Request::FlowRun { flow, wait },
        },
        Command::Runs(c) => match c {
            RunsCmd::List { flow, limit } => Request::RunsList {
                flow_id: flow,
                limit: Some(limit),
            },
            RunsCmd::Get { id } => Request::RunGet { id },
            RunsCmd::Cancel { id } => Request::RunCancel { id },
        },
        Command::Settings(c) => match c {
            SettingsCmd::Get { path } => Request::SettingsGet { path },
            SettingsCmd::Set { path, value } => {
                let value = serde_json::from_str(&value).unwrap_or(Value::String(value));
                Request::SettingsSet { path, value }
            }
        },
        Command::Server(c) => match c {
            ServerCmd::Status => Request::ServerStatus,
            ServerCmd::Start => Request::ServerStart,
            ServerCmd::Stop => Request::ServerStop,
        },
        Command::Bans(c) => match c {
            BansCmd::List => Request::BansList,
            BansCmd::Remove { ip } => Request::BanRemove { ip },
        },
        Command::System(c) => match c {
            SystemCmd::Info => Request::SystemInfo,
            SystemCmd::ActionTypes => Request::ActionTypes,
        },
        Command::App(c) => match c {
            AppCmd::Show => Request::AppShow,
            AppCmd::Quit => Request::AppQuit,
            AppCmd::RestartElevated => Request::AppRestartElevated,
        },
    })
}

/// Process exit codes, see the CLI documentation.
pub fn exit_code(code: ErrorCode) -> i32 {
    match code {
        ErrorCode::NotRunning => 3,
        ErrorCode::CliDisabled => 4,
        ErrorCode::Unauthorized | ErrorCode::Forbidden | ErrorCode::AdminRequired => 5,
        ErrorCode::NotFound => 6,
        ErrorCode::Validation => 7,
        ErrorCode::Conflict | ErrorCode::TooManyRequests => 8,
        ErrorCode::Internal => 1,
    }
}

fn cell(v: &Value) -> String {
    match v {
        Value::Null => "-".into(),
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(cell).collect::<Vec<_>>().join(","),
        other => other.to_string(),
    }
}

/// Renders an array of objects as an aligned text table.
pub fn table(rows: &[Value], columns: &[&str]) -> String {
    let data: Vec<Vec<String>> = rows
        .iter()
        .map(|r| columns.iter().map(|c| cell(&r[*c])).collect())
        .collect();
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            data.iter()
                .map(|r| r[i].chars().count())
                .chain([c.len()])
                .max()
                .unwrap_or(0)
        })
        .collect();
    let fmt = |cells: Vec<String>| {
        cells
            .iter()
            .zip(&widths)
            .map(|(c, w)| format!("{c:<w$}"))
            .collect::<Vec<_>>()
            .join("  ")
            .trim_end()
            .to_string()
    };
    let mut out = vec![fmt(columns.iter().map(|c| c.to_uppercase()).collect())];
    out.extend(data.into_iter().map(fmt));
    out.join("\n")
}

fn columns_for(req: &Request) -> Option<&'static [&'static str]> {
    Some(match req {
        Request::KeysList => &[
            "id",
            "name",
            "key_prefix",
            "enabled",
            "group_ids",
            "last_used_at",
        ],
        Request::GroupsList => &["id", "name", "key_ids", "flow_ids", "description"],
        Request::FlowsList => &["id", "path", "name", "enabled", "response_mode"],
        Request::RunsList { .. } => &["id", "flow_path", "source", "status", "created_at", "error"],
        Request::BansList => &["ip", "seconds_left"],
        Request::ActionTypes => &["id", "admin_hint"],
        _ => return None,
    })
}

/// Human-readable rendering of a successful response.
pub fn render(req: &Request, data: &Value) -> String {
    if let (Some(cols), Some(rows)) = (columns_for(req), data.as_array()) {
        return if rows.is_empty() {
            "(empty)".into()
        } else {
            table(rows, cols)
        };
    }
    if let Some(secret) = data.get("secret").and_then(Value::as_str) {
        let name = data["key"]["name"].as_str().unwrap_or("");
        return format!(
            "API key \"{name}\" (id {}).\nSecret (shown only once, store it now):\n\n  {secret}\n",
            data["key"]["id"]
        );
    }
    match data {
        Value::String(s) => s.clone(),
        Value::Number(_) | Value::Bool(_) => data.to_string(),
        _ => serde_json::to_string_pretty(data).unwrap_or_default(),
    }
}

/// CLI entry point. Returns the process exit code.
pub fn run(args: Vec<String>) -> i32 {
    let cli = match Cli::try_parse_from(args) {
        Ok(c) => c,
        Err(e) => {
            let _ = e.print();
            return if e.use_stderr() { 2 } else { 0 };
        }
    };
    let json_out = cli.json;
    let request = match to_request(cli.command) {
        Ok(r) => r,
        Err(e) => return report_error(&e, json_out),
    };
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    let endpoint = AppPaths::resolve().ipc_endpoint();
    let result = rt.block_on(ipc::send(
        &endpoint,
        &IpcRequest {
            request: request.clone(),
        },
    ));
    match result {
        Ok(resp) => match resp.error {
            Some(e) => report_error(&e, json_out),
            None => {
                if json_out {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&resp.data).unwrap_or_default()
                    );
                } else {
                    println!("{}", render(&request, &resp.data));
                }
                0
            }
        },
        Err(e) => report_error(&e, json_out),
    }
}

fn report_error(e: &AppError, json_out: bool) -> i32 {
    if json_out {
        println!("{}", serde_json::json!({ "error": e }));
    } else {
        eprintln!("error: {}", e.message);
    }
    exit_code(e.code)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(args: &[&str]) -> Request {
        let mut full = vec!["remote-http-action"];
        full.extend_from_slice(args);
        to_request(Cli::try_parse_from(full).unwrap().command).unwrap()
    }

    #[test]
    fn maps_commands_to_requests() {
        assert_eq!(parse(&["keys", "list"]), Request::KeysList);
        assert_eq!(
            parse(&["keys", "create", "phone", "--groups", "1,2"]),
            Request::KeyCreate {
                name: "phone".into(),
                group_ids: vec![1, 2]
            }
        );
        assert_eq!(
            parse(&["keys", "update", "3", "--disable", "--no-groups"]),
            Request::KeyUpdate {
                id: 3,
                name: None,
                enabled: Some(false),
                group_ids: Some(vec![])
            }
        );
        assert_eq!(
            parse(&["groups", "update", "1", "--flows", "4,5", "--name", "ops"]),
            Request::GroupUpdate {
                id: 1,
                name: Some("ops".into()),
                description: None,
                key_ids: None,
                flow_ids: Some(vec![4, 5])
            }
        );
        assert_eq!(
            parse(&["flows", "run", "sleep", "--wait"]),
            Request::FlowRun {
                flow: "sleep".into(),
                wait: true
            }
        );
        assert_eq!(
            parse(&["flows", "disable", "7"]),
            Request::FlowSetEnabled {
                flow: "7".into(),
                enabled: false
            }
        );
        assert_eq!(
            parse(&["runs", "list", "--limit", "5"]),
            Request::RunsList {
                flow_id: None,
                limit: Some(5)
            }
        );
        assert_eq!(parse(&["server", "stop"]), Request::ServerStop);
        assert_eq!(
            parse(&["app", "restart-elevated"]),
            Request::AppRestartElevated
        );
    }

    #[test]
    fn settings_values_are_parsed_as_json_or_string() {
        assert_eq!(
            parse(&["settings", "set", "server.port", "9000"]),
            Request::SettingsSet {
                path: "server.port".into(),
                value: json!(9000)
            }
        );
        assert_eq!(
            parse(&["settings", "set", "server.bind_address", "127.0.0.1"]),
            Request::SettingsSet {
                path: "server.bind_address".into(),
                value: json!("127.0.0.1")
            }
        );
        assert_eq!(
            parse(&["settings", "set", "server.allowed_ips", r#"["10.0.0.0/8"]"#]),
            Request::SettingsSet {
                path: "server.allowed_ips".into(),
                value: json!(["10.0.0.0/8"])
            }
        );
    }

    #[test]
    fn conflicting_flags_are_rejected() {
        assert!(
            Cli::try_parse_from(["x", "keys", "update", "1", "--enable", "--disable"]).is_err()
        );
        assert!(
            Cli::try_parse_from(["x", "keys", "update", "1", "--groups", "1", "--no-groups"])
                .is_err()
        );
    }

    #[test]
    fn flow_files_are_read() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.json");
        std::fs::write(&path, r#"{"name":"Sleep","path":"sleep","actions":[{"type":"sleep"}],"id":5,"created_at":"x"}"#).unwrap();
        let path_str = path.to_str().unwrap();
        match parse(&["flows", "create", "--file", path_str]) {
            Request::FlowCreate { flow } => {
                assert_eq!(flow.path, "sleep");
                assert_eq!(flow.actions.len(), 1);
                assert_eq!(flow.timeout_sec, 60);
            }
            other => panic!("{other:?}"),
        }
        let mut full = vec!["x", "flows", "create", "--file"];
        full.push("/definitely/missing.json");
        let cmd = Cli::try_parse_from(full).unwrap().command;
        assert_eq!(to_request(cmd).unwrap_err().code, ErrorCode::Validation);
    }

    #[test]
    fn renders_tables_and_secrets() {
        let rows = vec![
            json!({"id": 1, "name": "phone", "key_prefix": "rha_x", "enabled": true, "group_ids": [1, 2], "last_used_at": null}),
        ];
        let out = render(&Request::KeysList, &json!(rows));
        assert!(out.lines().next().unwrap().starts_with("ID"));
        assert!(out.contains("1,2"));
        let out = render(
            &Request::KeyRotate { id: 1 },
            &json!({"key": {"id": 1, "name": "phone"}, "secret": "rha_abc"}),
        );
        assert!(out.contains("rha_abc"));
        assert_eq!(render(&Request::KeysList, &json!([])), "(empty)");
    }

    #[test]
    fn exit_codes_are_distinct_for_common_errors() {
        assert_eq!(exit_code(ErrorCode::NotRunning), 3);
        assert_eq!(exit_code(ErrorCode::CliDisabled), 4);
        assert_eq!(exit_code(ErrorCode::NotFound), 6);
    }
}
