//! Action types: parameter validation and execution.
//!
//! To add an action type: add a variant to [`ActionParams`], handle it in [`execute`],
//! list it in [`ACTION_TYPES`], add its form to the UI and document it.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::model::{ActionDef, StepStatus};
use crate::platform::{Platform, PowerAction, ProcessSpec};

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

#[derive(Debug, Clone, PartialEq)]
pub enum ActionParams {
    Delay(DelayParams),
    Shutdown(PowerParams),
    Reboot(PowerParams),
    Sleep(SleepParams),
    CliCommand(CliCommandParams),
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
}
