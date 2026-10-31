//! Action types: parameter validation and execution.
//!
//! To add an action type: add a variant to [`ActionParams`], handle it in [`execute`],
//! list it in [`ACTION_TYPES`], add its form to the UI and document it.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::model::{ActionDef, StepStatus};
use crate::platform::{LaunchWait, Platform, PowerAction, ProcessSpec};

/// Max characters of stdout/stderr kept per step.
pub const OUTPUT_LIMIT: usize = 16 * 1024;
const MAX_DELAY_SEC: f64 = 86_400.0;

#[derive(Debug, Clone, Serialize)]
pub struct ActionTypeInfo {
    pub id: &'static str,
    /// The action usually needs administrator rights.
    pub admin_hint: bool,
}

pub const ACTION_TYPES: &[ActionTypeInfo] = &[
    ActionTypeInfo {
        id: "delay",
        admin_hint: false,
    },
    ActionTypeInfo {
        id: "shutdown",
        admin_hint: false,
    },
    ActionTypeInfo {
        id: "reboot",
        admin_hint: false,
    },
    ActionTypeInfo {
        id: "sleep",
        admin_hint: false,
    },
    ActionTypeInfo {
        id: "cli_command",
        admin_hint: false,
    },
    ActionTypeInfo {
        id: "run_script",
        admin_hint: false,
    },
    ActionTypeInfo {
        id: "launch_app",
        admin_hint: false,
    },
];

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DelayParams {
    pub seconds: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct PowerParams {
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Default)]
#[serde(deny_unknown_fields, default)]
pub struct SleepParams {
    pub hibernate: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliCommandParams {
    pub shell: String,
    pub command: String,
    #[serde(default)]
    pub working_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunScriptParams {
    pub shell: String,
    pub script_path: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Defaults to the script's folder.
    #[serde(default)]
    pub working_dir: Option<String>,
}

/// What `launch_app` waits for before the next step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WaitMode {
    /// Continue as soon as the program has started.
    #[default]
    None,
    /// Until the started process exits; a non-zero exit code fails the step.
    Process,
    /// Until the started process and all processes it started have exited.
    Tree,
    /// Until no process with `wait_process_name` is running (launchers, single-instance apps).
    Name,
}

impl<'de> Deserialize<'de> for WaitMode {
    /// Also accepts the older boolean form: `true` = `process`, `false` = `none`.
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Bool(bool),
            Text(String),
        }
        match Raw::deserialize(d)? {
            Raw::Bool(true) => Ok(WaitMode::Process),
            Raw::Bool(false) => Ok(WaitMode::None),
            Raw::Text(t) => match t.as_str() {
                "none" => Ok(WaitMode::None),
                "process" => Ok(WaitMode::Process),
                "tree" => Ok(WaitMode::Tree),
                "name" => Ok(WaitMode::Name),
                other => Err(serde::de::Error::custom(format!(
                    "unknown wait mode \"{other}\" (none, process, tree, name)"
                ))),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchAppParams {
    pub path: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Defaults to the program's folder.
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub wait: WaitMode,
    /// For `wait: name`; defaults to the file name of `path`.
    #[serde(default)]
    pub wait_process_name: Option<String>,
    /// For `wait: name`: how long the process may take to appear.
    #[serde(default)]
    pub wait_start_timeout_sec: Option<u64>,
}

impl LaunchAppParams {
    pub fn process_name(&self) -> String {
        self.wait_process_name
            .as_deref()
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| {
                let path = self.path.trim();
                path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
            })
    }
}

/// Polling interval and confirmation count for `wait: name`.
const NAME_POLL: Duration = Duration::from_secs(1);
const NAME_GONE_CONFIRMATIONS: u32 = 3;
const DEFAULT_START_TIMEOUT_SEC: u64 = 60;

#[derive(Debug, Clone, PartialEq)]
pub enum ActionParams {
    Delay(DelayParams),
    Shutdown(PowerParams),
    Reboot(PowerParams),
    Sleep(SleepParams),
    CliCommand(CliCommandParams),
    RunScript(RunScriptParams),
    LaunchApp(LaunchAppParams),
}

pub const SHELLS: &[&str] = &["cmd", "powershell", "pwsh", "wsl", "sh"];

/// Parses and validates the params of an action definition.
pub fn parse(def: &ActionDef) -> AppResult<ActionParams> {
    let p = def.params.clone();
    let bad = |e: serde_json::Error| {
        AppError::validation(format!("{}: invalid params: {e}", def.action_type))
    };
    let params = match def.action_type.as_str() {
        "delay" => {
            let d: DelayParams = serde_json::from_value(p).map_err(bad)?;
            if !(0.0..=MAX_DELAY_SEC).contains(&d.seconds) || !d.seconds.is_finite() {
                return Err(AppError::validation(
                    "delay: seconds must be between 0 and 86400",
                ));
            }
            ActionParams::Delay(d)
        }
        "shutdown" => ActionParams::Shutdown(serde_json::from_value(p).map_err(bad)?),
        "reboot" => ActionParams::Reboot(serde_json::from_value(p).map_err(bad)?),
        "sleep" => ActionParams::Sleep(serde_json::from_value(p).map_err(bad)?),
        "cli_command" => {
            let c: CliCommandParams = serde_json::from_value(p).map_err(bad)?;
            if !SHELLS.contains(&c.shell.as_str()) {
                return Err(AppError::validation(format!(
                    "cli_command: unknown shell \"{}\"",
                    c.shell
                )));
            }
            if c.command.trim().is_empty() {
                return Err(AppError::validation(
                    "cli_command: command must not be empty",
                ));
            }
            ActionParams::CliCommand(c)
        }
        "run_script" => {
            let c: RunScriptParams = serde_json::from_value(p).map_err(bad)?;
            if !SHELLS.contains(&c.shell.as_str()) {
                return Err(AppError::validation(format!(
                    "run_script: unknown shell \"{}\"",
                    c.shell
                )));
            }
            if c.script_path.trim().is_empty() {
                return Err(AppError::validation(
                    "run_script: script path must not be empty",
                ));
            }
            ActionParams::RunScript(c)
        }
        "launch_app" => {
            let c: LaunchAppParams = serde_json::from_value(p).map_err(bad)?;
            if c.path.trim().is_empty() {
                return Err(AppError::validation(
                    "launch_app: program path must not be empty",
                ));
            }
            if c.wait == WaitMode::Name && c.process_name().contains(['\\', '/']) {
                return Err(AppError::validation(
                    "launch_app: process name must be a file name, e.g. steam.exe",
                ));
            }
            if c.wait_start_timeout_sec == Some(0) {
                return Err(AppError::validation(
                    "launch_app: start timeout must be positive",
                ));
            }
            ActionParams::LaunchApp(c)
        }
        other => {
            return Err(AppError::validation(format!(
                "unknown action type \"{other}\""
            )));
        }
    };
    if def.timeout_sec == Some(0) {
        return Err(AppError::validation(format!(
            "{}: timeout must be positive",
            def.action_type
        )));
    }
    Ok(params)
}

/// Builds the process invocation for a shell command.
pub fn shell_command(shell: &str, command: &str, working_dir: Option<String>) -> ProcessSpec {
    let (program, args, raw_args) = match shell {
        // `cmd /C` has its own quoting rules, so the command line is passed verbatim.
        "cmd" => (
            "cmd.exe".to_string(),
            vec![
                "/D".into(),
                "/S".into(),
                "/C".into(),
                format!("\"{command}\""),
            ],
            true,
        ),
        "powershell" | "pwsh" => (
            format!("{shell}.exe"),
            vec![
                "-NoProfile".into(),
                "-NonInteractive".into(),
                "-Command".into(),
                format!("[Console]::OutputEncoding=[Text.Encoding]::UTF8; {command}"),
            ],
            false,
        ),
        "wsl" => (
            "wsl.exe".to_string(),
            vec![
                "--exec".into(),
                "sh".into(),
                "-c".into(),
                command.to_string(),
            ],
            false,
        ),
        _ => (
            "sh".to_string(),
            vec!["-c".into(), command.to_string()],
            false,
        ),
    };
    ProcessSpec {
        program,
        args,
        raw_args,
        working_dir,
    }
}

/// Quotes one argument for a `cmd.exe` command line.
fn quote_cmd_arg(arg: &str) -> String {
    let needs = arg.is_empty()
        || arg
            .chars()
            .any(|c| c.is_whitespace() || "&|<>^\"(),;=".contains(c));
    if needs {
        format!("\"{}\"", arg.replace('"', "\"\""))
    } else {
        arg.to_string()
    }
}

/// `C:\dir\file.sh` → `/mnt/c/dir/file.sh`; other paths are returned unchanged.
pub fn to_wsl_path(path: &str) -> String {
    let b = path.as_bytes();
    if b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && (b[2] == b'\\' || b[2] == b'/')
    {
        let drive = (b[0] as char).to_ascii_lowercase();
        format!("/mnt/{drive}/{}", path[3..].replace('\\', "/"))
    } else {
        path.to_string()
    }
}

fn parent_dir(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .parent()
        .map(|p| p.display().to_string())
        .filter(|p| !p.is_empty())
}

/// Builds the process invocation for a script file.
pub fn script_command(p: &RunScriptParams) -> ProcessSpec {
    let path = p.script_path.trim();
    let (program, args, raw_args) = match p.shell.as_str() {
        "cmd" => {
            let mut line = quote_cmd_arg(path);
            for a in &p.args {
                line.push(' ');
                line.push_str(&quote_cmd_arg(a));
            }
            (
                "cmd.exe".to_string(),
                vec!["/D".into(), "/S".into(), "/C".into(), format!("\"{line}\"")],
                true,
            )
        }
        "powershell" | "pwsh" => {
            let mut args: Vec<String> = [
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                path,
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            args.extend(p.args.iter().cloned());
            (format!("{}.exe", p.shell), args, false)
        }
        "wsl" => {
            let mut args = vec!["--exec".into(), "sh".into(), to_wsl_path(path)];
            args.extend(p.args.iter().cloned());
            ("wsl.exe".to_string(), args, false)
        }
        _ => {
            let mut args = vec![path.to_string()];
            args.extend(p.args.iter().cloned());
            ("sh".to_string(), args, false)
        }
    };
    let working_dir = p
        .working_dir
        .clone()
        .filter(|d| !d.trim().is_empty())
        .or_else(|| (p.shell != "wsl").then(|| parent_dir(path)).flatten());
    ProcessSpec {
        program,
        args,
        raw_args,
        working_dir,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub status: StepStatus,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub error: Option<String>,
}

impl Outcome {
    fn success() -> Self {
        Self {
            status: StepStatus::Success,
            exit_code: None,
            stdout: None,
            stderr: None,
            error: None,
        }
    }
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            status: StepStatus::Failed,
            error: Some(error.into()),
            ..Self::success()
        }
    }
    pub fn canceled() -> Self {
        Self {
            status: StepStatus::Canceled,
            error: Some("canceled".into()),
            ..Self::success()
        }
    }
}

fn truncate(s: String) -> String {
    if s.chars().count() <= OUTPUT_LIMIT {
        return s;
    }
    let mut t: String = s.chars().take(OUTPUT_LIMIT).collect();
    t.push_str("\n…[truncated]");
    t
}

/// Executes one action. Admin checks and timeouts are applied by the runner.
pub async fn execute(
    params: ActionParams,
    platform: &dyn Platform,
    cancel: CancellationToken,
) -> Outcome {
    match params {
        ActionParams::Delay(d) => {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs_f64(d.seconds)) => Outcome::success(),
                _ = cancel.cancelled() => Outcome::canceled(),
            }
        }
        ActionParams::Shutdown(p) => {
            power(platform, PowerAction::Shutdown { force: p.force }).await
        }
        ActionParams::Reboot(p) => power(platform, PowerAction::Reboot { force: p.force }).await,
        ActionParams::Sleep(p) => {
            power(
                platform,
                PowerAction::Sleep {
                    hibernate: p.hibernate,
                },
            )
            .await
        }
        ActionParams::CliCommand(c) => {
            let spec = shell_command(&c.shell, &c.command, c.working_dir);
            run_captured(platform, spec, cancel).await
        }
        ActionParams::RunScript(p) => {
            if p.shell != "wsl" && !std::path::Path::new(p.script_path.trim()).is_file() {
                return Outcome::failed(format!("script not found: {}", p.script_path.trim()));
            }
            run_captured(platform, script_command(&p), cancel).await
        }
        ActionParams::LaunchApp(p) => {
            let name = p.process_name();
            let start_timeout = p
                .wait_start_timeout_sec
                .unwrap_or(DEFAULT_START_TIMEOUT_SEC);
            let mode = p.wait;
            let path = p.path.trim().to_string();
            if !std::path::Path::new(&path).is_file() {
                return Outcome::failed(format!("program not found: {path}"));
            }
            let working_dir = p
                .working_dir
                .filter(|d| !d.trim().is_empty())
                .or_else(|| parent_dir(&path));
            let spec = ProcessSpec {
                program: path,
                args: p.args,
                raw_args: false,
                working_dir,
            };
            let launch_wait = match mode {
                WaitMode::None | WaitMode::Name => LaunchWait::None,
                WaitMode::Process => LaunchWait::Process,
                WaitMode::Tree => LaunchWait::Tree,
            };
            let launched = platform.launch(spec, launch_wait, cancel.clone()).await;
            if mode == WaitMode::Name {
                return match launched {
                    Ok(_) => wait_for_name(platform, &name, start_timeout, cancel).await,
                    Err(e) => Outcome::failed(e.message),
                };
            }
            match launched {
                Ok(None) => Outcome::success(),
                Ok(Some(0)) => Outcome {
                    exit_code: Some(0),
                    ..Outcome::success()
                },
                Ok(Some(code)) => Outcome {
                    exit_code: Some(code),
                    ..Outcome::failed(format!("exit code {code}"))
                },
                Err(_) if cancel.is_cancelled() => Outcome::canceled(),
                Err(e) => Outcome::failed(e.message),
            }
        }
    }
}

/// Waits until a process named `name` has appeared and then until none is left.
/// The process counts as gone after several consecutive checks, so a program that restarts
/// itself (e.g. after an update) is still waited for.
async fn wait_for_name(
    platform: &dyn Platform,
    name: &str,
    start_timeout_sec: u64,
    cancel: CancellationToken,
) -> Outcome {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(start_timeout_sec);
    while platform.process_count(name) == 0 {
        if tokio::time::Instant::now() >= deadline {
            return Outcome::failed(format!(
                "process {name} did not start within {start_timeout_sec}s"
            ));
        }
        tokio::select! {
            _ = tokio::time::sleep(NAME_POLL) => {}
            _ = cancel.cancelled() => return Outcome::canceled(),
        }
    }
    let mut absent = 0;
    loop {
        tokio::select! {
            _ = tokio::time::sleep(NAME_POLL) => {}
            _ = cancel.cancelled() => return Outcome::canceled(),
        }
        if platform.process_count(name) == 0 {
            absent += 1;
            if absent >= NAME_GONE_CONFIRMATIONS {
                return Outcome::success();
            }
        } else {
            absent = 0;
        }
    }
}

async fn run_captured(
    platform: &dyn Platform,
    spec: ProcessSpec,
    cancel: CancellationToken,
) -> Outcome {
    match platform.run_process(spec, cancel.clone()).await {
        Ok(out) => {
            let ok = out.exit_code == Some(0);
            Outcome {
                status: if ok {
                    StepStatus::Success
                } else {
                    StepStatus::Failed
                },
                exit_code: out.exit_code,
                stdout: Some(truncate(out.stdout)),
                stderr: Some(truncate(out.stderr)),
                error: if ok {
                    None
                } else {
                    Some(match out.exit_code {
                        Some(c) => format!("exit code {c}"),
                        None => "terminated by signal".into(),
                    })
                },
            }
        }
        Err(_) if cancel.is_cancelled() => Outcome::canceled(),
        Err(e) => Outcome::failed(e.message),
    }
}

async fn power(platform: &dyn Platform, action: PowerAction) -> Outcome {
    match platform.power(action).await {
        Ok(()) => Outcome::success(),
        Err(e) => Outcome::failed(e.message),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::platform::mock::{MockPlatform, MockResponse};

    fn def(t: &str, params: serde_json::Value) -> ActionDef {
        ActionDef {
            action_type: t.into(),
            params,
            on_error: Default::default(),
            timeout_sec: None,
            run_as_admin: false,
            capture_output: false,
        }
    }

    #[test]
    fn validates_params() {
        assert!(parse(&def("delay", json!({"seconds": 1.5}))).is_ok());
        assert!(parse(&def("delay", json!({"seconds": -1}))).is_err());
        assert!(parse(&def("delay", json!({}))).is_err());
        assert!(parse(&def("shutdown", json!({}))).is_ok());
        assert!(parse(&def("shutdown", json!({"force": true}))).is_ok());
        assert!(parse(&def("shutdown", json!({"bogus": 1}))).is_err());
        assert!(
            parse(&def(
                "cli_command",
                json!({"shell": "cmd", "command": "dir"})
            ))
            .is_ok()
        );
        assert!(
            parse(&def(
                "cli_command",
                json!({"shell": "bash", "command": "ls"})
            ))
            .is_err()
        );
        assert!(
            parse(&def(
                "cli_command",
                json!({"shell": "cmd", "command": "  "})
            ))
            .is_err()
        );
        assert!(parse(&def("format_disk", json!({}))).is_err());
    }

    #[test]
    fn every_listed_type_is_parseable() {
        for t in ACTION_TYPES {
            let params = match t.id {
                "delay" => json!({"seconds": 0}),
                "cli_command" => json!({"shell": "cmd", "command": "echo"}),
                "run_script" => json!({"shell": "powershell", "script_path": "C:\\s.ps1"}),
                "launch_app" => json!({"path": "C:\\app.exe"}),
                _ => json!({}),
            };
            assert!(parse(&def(t.id, params)).is_ok(), "{}", t.id);
        }
    }

    #[test]
    fn builds_shell_invocations() {
        let s = shell_command("cmd", "echo hi", None);
        assert_eq!(s.program, "cmd.exe");
        assert!(s.raw_args);
        assert_eq!(s.args.last().unwrap(), "\"echo hi\"");
        let s = shell_command("pwsh", "Get-Date", Some("C:\\".into()));
        assert_eq!(s.program, "pwsh.exe");
        assert!(s.args.last().unwrap().ends_with("Get-Date"));
        assert_eq!(s.working_dir.as_deref(), Some("C:\\"));
        let s = shell_command("wsl", "uname -a", None);
        assert_eq!(s.args, vec!["--exec", "sh", "-c", "uname -a"]);
    }

    #[tokio::test]
    async fn power_actions_go_through_platform() {
        let p = MockPlatform::default();
        let out = execute(
            parse(&def("reboot", json!({"force": true}))).unwrap(),
            &p,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(out.status, StepStatus::Success);
        assert_eq!(
            *p.power_calls.lock().unwrap(),
            vec![PowerAction::Reboot { force: true }]
        );
    }

    #[tokio::test]
    async fn nonzero_exit_code_fails_the_step() {
        let p = MockPlatform::with_responses(vec![MockResponse::exit(3)]);
        let a = parse(&def("cli_command", json!({"shell": "cmd", "command": "x"}))).unwrap();
        let out = execute(a, &p, CancellationToken::new()).await;
        assert_eq!(out.status, StepStatus::Failed);
        assert_eq!(out.exit_code, Some(3));
        assert_eq!(out.error.as_deref(), Some("exit code 3"));
    }

    #[tokio::test]
    async fn delay_can_be_canceled() {
        let p = MockPlatform::default();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let out = execute(
            parse(&def("delay", json!({"seconds": 60}))).unwrap(),
            &p,
            cancel,
        )
        .await;
        assert_eq!(out.status, StepStatus::Canceled);
    }

    #[test]
    fn output_is_truncated() {
        let long = "x".repeat(OUTPUT_LIMIT + 10);
        assert!(truncate(long).ends_with("[truncated]"));
    }

    #[test]
    fn validates_script_and_app_params() {
        assert!(
            parse(&def(
                "run_script",
                json!({"shell": "cmd", "script_path": "a.bat", "args": ["x"]})
            ))
            .is_ok()
        );
        assert!(
            parse(&def(
                "run_script",
                json!({"shell": "cmd", "script_path": " "})
            ))
            .is_err()
        );
        assert!(
            parse(&def(
                "run_script",
                json!({"shell": "zsh", "script_path": "a"})
            ))
            .is_err()
        );
        assert!(
            parse(&def(
                "launch_app",
                json!({"path": "C:\\a.exe", "wait": true})
            ))
            .is_ok()
        );
        assert!(parse(&def("launch_app", json!({"path": ""}))).is_err());
    }

    fn script(shell: &str, path: &str, args: &[&str]) -> RunScriptParams {
        RunScriptParams {
            shell: shell.into(),
            script_path: path.into(),
            args: args.iter().map(|a| a.to_string()).collect(),
            working_dir: None,
        }
    }

    #[test]
    fn builds_script_invocations() {
        let s = script_command(&script(
            "cmd",
            r"C:\My Scripts\run.bat",
            &["one", "two words", "a&b"],
        ));
        assert!(s.raw_args);
        assert_eq!(
            s.args.last().unwrap(),
            r#"""C:\My Scripts\run.bat" one "two words" "a&b"""#
        );
        #[cfg(windows)]
        assert_eq!(s.working_dir.as_deref(), Some(r"C:\My Scripts"));

        let s = script_command(&script("powershell", r"C:\s\x.ps1", &["-Name", "Петя"]));
        assert_eq!(s.program, "powershell.exe");
        assert_eq!(
            s.args,
            vec![
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                r"C:\s\x.ps1",
                "-Name",
                "Петя"
            ]
        );

        let s = script_command(&script("wsl", r"C:\Users\me\job.sh", &["1"]));
        assert_eq!(s.args, vec!["--exec", "sh", "/mnt/c/Users/me/job.sh", "1"]);
        assert_eq!(s.working_dir, None);
        assert_eq!(to_wsl_path("/home/me/x.sh"), "/home/me/x.sh");
    }

    #[tokio::test]
    async fn missing_files_fail_before_starting() {
        let p = MockPlatform::default();
        let out = execute(
            parse(&def(
                "run_script",
                json!({"shell": "cmd", "script_path": "/no/such.bat"}),
            ))
            .unwrap(),
            &p,
            CancellationToken::new(),
        )
        .await;
        assert!(out.error.unwrap().contains("script not found"));
        let out = execute(
            parse(&def("launch_app", json!({"path": "/no/such.exe"}))).unwrap(),
            &p,
            CancellationToken::new(),
        )
        .await;
        assert!(out.error.unwrap().contains("program not found"));
        assert!(p.processes.lock().unwrap().is_empty());
        assert!(p.launched.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn scripts_and_apps_run_through_platform() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("tool.exe");
        std::fs::write(&file, b"").unwrap();
        let path = file.display().to_string();

        let p = MockPlatform::with_responses(vec![MockResponse::ok("script output")]);
        let out = execute(
            parse(&def(
                "run_script",
                json!({"shell": "sh", "script_path": path}),
            ))
            .unwrap(),
            &p,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(out.status, StepStatus::Success);
        assert_eq!(out.stdout.as_deref(), Some("script output"));
        assert_eq!(
            p.processes.lock().unwrap()[0].working_dir.as_deref(),
            Some(dir.path().to_str().unwrap())
        );

        let out = execute(
            parse(&def(
                "launch_app",
                json!({"path": path, "args": ["--x"], "wait": true}),
            ))
            .unwrap(),
            &p,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(out.status, StepStatus::Success);
        assert_eq!(out.exit_code, Some(0));
        let launched = p.launched.lock().unwrap();
        assert_eq!(launched[0].0.args, vec!["--x"]);
        assert_eq!(launched[0].1, LaunchWait::Process);
    }

    #[test]
    fn wait_mode_accepts_old_boolean_form() {
        let p = |v: serde_json::Value| -> LaunchAppParams { serde_json::from_value(v).unwrap() };
        assert_eq!(
            p(json!({"path": "a.exe", "wait": true})).wait,
            WaitMode::Process
        );
        assert_eq!(
            p(json!({"path": "a.exe", "wait": false})).wait,
            WaitMode::None
        );
        assert_eq!(p(json!({"path": "a.exe"})).wait, WaitMode::None);
        assert_eq!(
            p(json!({"path": "a.exe", "wait": "tree"})).wait,
            WaitMode::Tree
        );
        assert!(
            parse(&def(
                "launch_app",
                json!({"path": "a.exe", "wait": "forever"})
            ))
            .is_err()
        );
    }

    #[test]
    fn process_name_defaults_to_file_name() {
        let p: LaunchAppParams = serde_json::from_value(
            json!({"path": r"C:\Program Files (x86)\Steam\steam.exe", "wait": "name"}),
        )
        .unwrap();
        assert_eq!(p.process_name(), "steam.exe");
        let p: LaunchAppParams = serde_json::from_value(
            json!({"path": "x.exe", "wait": "name", "wait_process_name": " game.exe "}),
        )
        .unwrap();
        assert_eq!(p.process_name(), "game.exe");
        assert!(
            parse(&def(
                "launch_app",
                json!({"path": "x.exe", "wait": "name", "wait_process_name": r"C:\a\b.exe"})
            ))
            .is_err()
        );
    }

    fn app_file() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("steam.exe");
        std::fs::write(&file, b"").unwrap();
        let path = file.display().to_string();
        (dir, path)
    }

    #[tokio::test(start_paused = true)]
    async fn wait_by_name_waits_until_all_processes_are_gone() {
        let (_dir, path) = app_file();
        // Not yet started, running (a restart in between), then gone.
        let p = MockPlatform::default();
        *p.process_counts.lock().unwrap() = vec![0, 0, 2, 1, 0, 1, 1, 0, 0, 0].into();
        let started = tokio::time::Instant::now();
        let out = execute(
            parse(&def("launch_app", json!({"path": path, "wait": "name"}))).unwrap(),
            &p,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(out.status, StepStatus::Success, "{out:?}");
        assert!(started.elapsed() >= Duration::from_secs(8));
        assert_eq!(p.launched.lock().unwrap()[0].1, LaunchWait::None);
    }

    #[tokio::test(start_paused = true)]
    async fn wait_by_name_fails_if_the_process_never_appears() {
        let (_dir, path) = app_file();
        let p = MockPlatform::default();
        let out = execute(
            parse(&def(
                "launch_app",
                json!({"path": path, "wait": "name", "wait_start_timeout_sec": 5}),
            ))
            .unwrap(),
            &p,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(out.status, StepStatus::Failed);
        assert!(out.error.unwrap().contains("did not start within 5s"));
    }

    #[tokio::test(start_paused = true)]
    async fn wait_by_name_can_be_canceled() {
        let (_dir, path) = app_file();
        let p = MockPlatform::default();
        *p.process_counts.lock().unwrap() = vec![1].into();
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(30)).await;
            c.cancel();
        });
        let out = execute(
            parse(&def("launch_app", json!({"path": path, "wait": "name"}))).unwrap(),
            &p,
            cancel,
        )
        .await;
        assert_eq!(out.status, StepStatus::Canceled);
    }

    #[tokio::test]
    async fn tree_mode_is_passed_to_the_platform() {
        let (_dir, path) = app_file();
        let p = MockPlatform::default();
        execute(
            parse(&def("launch_app", json!({"path": path, "wait": "tree"}))).unwrap(),
            &p,
            CancellationToken::new(),
        )
        .await;
        assert_eq!(p.launched.lock().unwrap()[0].1, LaunchWait::Tree);
    }
}
