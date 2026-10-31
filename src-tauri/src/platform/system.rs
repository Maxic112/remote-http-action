//! Real platform implementation.

use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

use super::*;
use crate::error::AppError;

pub struct SystemPlatform {
    /// Log power actions instead of performing them (`RHA_DRY_RUN_POWER=1`).
    pub dry_run_power: bool,
}

impl SystemPlatform {
    pub fn from_env() -> Self {
        let dry = std::env::var("RHA_DRY_RUN_POWER")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        Self { dry_run_power: dry }
    }
}

#[async_trait]
impl Platform for SystemPlatform {
    fn is_elevated(&self) -> bool {
        #[cfg(windows)]
        {
            super::win::is_elevated()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }

    async fn power(&self, action: PowerAction) -> AppResult<()> {
        if self.dry_run_power {
            eprintln!("[dry-run] power action skipped: {action:?}");
            return Ok(());
        }
        power_impl(action).await
    }

    async fn run_process(
        &self,
        spec: ProcessSpec,
        cancel: CancellationToken,
    ) -> AppResult<ProcessOutput> {
        let mut cmd = tokio::process::Command::new(&spec.program);
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
            if spec.raw_args {
                cmd.raw_arg(spec.args.join(" "));
            } else {
                cmd.args(&spec.args);
            }
        }
        #[cfg(not(windows))]
        cmd.args(&spec.args);
        if let Some(dir) = spec.working_dir.as_deref().filter(|d| !d.is_empty()) {
            cmd.current_dir(dir);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::internal(format!("failed to start {}: {e}", spec.program)))?;
        let mut out = child.stdout.take().expect("stdout piped");
        let mut err = child.stderr.take().expect("stderr piped");
        let read_out = async {
            let mut buf = Vec::new();
            let _ = out.read_to_end(&mut buf).await;
            buf
        };
        let read_err = async {
            let mut buf = Vec::new();
            let _ = err.read_to_end(&mut buf).await;
            buf
        };
        let run = async {
            let (o, e, status) = tokio::join!(read_out, read_err, child.wait());
            (o, e, status)
        };
        tokio::select! {
            (o, e, status) = run => {
                let status = status.map_err(|e| AppError::internal(format!("wait failed: {e}")))?;
                Ok(ProcessOutput {
                    exit_code: status.code(),
                    stdout: decode(&o),
                    stderr: decode(&e),
                })
            }
            _ = cancel.cancelled() => Err(AppError::internal("process killed")),
        }
    }

    fn shells(&self) -> Vec<ShellInfo> {
        detect_shells()
    }

    fn relaunch_elevated(&self) -> AppResult<()> {
        #[cfg(windows)]
        {
            super::win::relaunch_elevated()
        }
        #[cfg(not(windows))]
        {
            Err(AppError::internal("elevation is only supported on Windows"))
        }
    }

    async fn launch(
        &self,
        spec: ProcessSpec,
        wait: LaunchWait,
        cancel: CancellationToken,
    ) -> AppResult<Option<i32>> {
        let mut cmd = tokio::process::Command::new(&spec.program);
        cmd.args(&spec.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dir) = spec.working_dir.as_deref().filter(|d| !d.is_empty()) {
            cmd.current_dir(dir);
        }
        // For `Tree` the process starts suspended, so it joins the job before it can start
        // children of its own.
        #[cfg(windows)]
        if wait == LaunchWait::Tree {
            const CREATE_SUSPENDED: u32 = 0x0000_0004;
            cmd.creation_flags(CREATE_SUSPENDED);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::internal(format!("failed to start {}: {e}", spec.program)))?;

        // Cancelling stops the waiting only: the program is left running.
        let stopped = || AppError::internal("stopped waiting");
        match wait {
            // Dropping the handle leaves the program running.
            LaunchWait::None => Ok(None),
            #[cfg(windows)]
            LaunchWait::Tree => {
                let job = super::win::Job::new();
                let pid = child.id();
                let assigned = match (&job, child.raw_handle()) {
                    (Ok(job), Some(h)) => job.assign(windows::Win32::Foundation::HANDLE(h)).is_ok(),
                    _ => false,
                };
                if let Some(pid) = pid {
                    super::win::resume_process(pid);
                }
                let job = match job {
                    Ok(job) if assigned => job,
                    // Without a job only the started process can be waited for.
                    _ => {
                        return tokio::select! {
                            status = child.wait() => Ok(status.ok().and_then(|s| s.code())),
                            _ = cancel.cancelled() => Err(stopped()),
                        };
                    }
                };
                loop {
                    if job.active_processes() == 0 {
                        return Ok(child.try_wait().ok().flatten().and_then(|s| s.code()));
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {}
                        _ = cancel.cancelled() => return Err(stopped()),
                    }
                }
            }
            _ => {
                tokio::select! {
                    status = child.wait() => {
                        let status = status.map_err(|e| AppError::internal(format!("wait failed: {e}")))?;
                        Ok(status.code())
                    }
                    _ = cancel.cancelled() => Err(stopped()),
                }
            }
        }
    }

    fn process_count(&self, name: &str) -> usize {
        #[cfg(windows)]
        {
            super::win::process_count(name)
        }
        #[cfg(not(windows))]
        {
            let wanted = name.trim().to_lowercase();
            std::process::Command::new("ps")
                .args(["-A", "-o", "comm="])
                .output()
                .map(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .filter(|l| {
                            l.trim().rsplit('/').next().unwrap_or("").to_lowercase() == wanted
                        })
                        .count()
                })
                .unwrap_or(0)
        }
    }

    fn open_folder(&self, path: &std::path::Path) -> AppResult<()> {
        let _ = std::fs::create_dir_all(path);
        let program = if cfg!(windows) {
            "explorer.exe"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        std::process::Command::new(program)
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| AppError::internal(format!("cannot open {}: {e}", path.display())))
    }

    fn autostart_status(&self) -> AppResult<AutostartStatus> {
        #[cfg(windows)]
        {
            super::autostart::status(super::autostart::task_name())
        }
        #[cfg(not(windows))]
        {
            Ok(AutostartStatus {
                supported: false,
                installed: false,
                command: None,
                matches_current: false,
            })
        }
    }

    fn autostart_install(&self) -> AppResult<()> {
        #[cfg(windows)]
        {
            super::autostart::install(super::autostart::task_name())
        }
        #[cfg(not(windows))]
        {
            Err(AppError::internal("autostart is only supported on Windows"))
        }
    }

    fn autostart_remove(&self) -> AppResult<()> {
        #[cfg(windows)]
        {
            super::autostart::remove(super::autostart::task_name())
        }
        #[cfg(not(windows))]
        {
            Err(AppError::internal("autostart is only supported on Windows"))
        }
    }
}

/// Process output is UTF-8 when the program supports it; console programs that use the OEM
/// code page (e.g. `cmd` built-ins) are decoded with it instead.
pub(crate) fn decode(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        #[cfg(windows)]
        Err(_) => decode_oem(bytes).unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned()),
        #[cfg(not(windows))]
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}

#[cfg(windows)]
fn decode_oem(bytes: &[u8]) -> Option<String> {
    use windows::Win32::Globalization::{
        GetOEMCP, MULTI_BYTE_TO_WIDE_CHAR_FLAGS, MultiByteToWideChar,
    };
    unsafe {
        let cp = GetOEMCP();
        let len = MultiByteToWideChar(cp, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, None);
        if len <= 0 {
            return None;
        }
        let mut wide = vec![0u16; len as usize];
        MultiByteToWideChar(cp, MULTI_BYTE_TO_WIDE_CHAR_FLAGS(0), bytes, Some(&mut wide));
        Some(String::from_utf16_lossy(&wide))
    }
}

#[cfg(windows)]
async fn power_impl(action: PowerAction) -> AppResult<()> {
    match action {
        PowerAction::Shutdown { force } | PowerAction::Reboot { force } => {
            let flag = if matches!(action, PowerAction::Shutdown { .. }) {
                "/s"
            } else {
                "/r"
            };
            let mut args = vec![flag, "/t", "0"];
            if force {
                args.push("/f");
            }
            let status = tokio::process::Command::new("shutdown.exe")
                .args(&args)
                .creation_flags(0x0800_0000)
                .status()
                .await
                .map_err(|e| AppError::internal(format!("failed to run shutdown.exe: {e}")))?;
            if !status.success() {
                return Err(AppError::internal(format!(
                    "shutdown.exe exited with {status}"
                )));
            }
            Ok(())
        }
        PowerAction::Sleep { hibernate } => {
            // SetSuspendState blocks until the machine wakes up, so it runs detached after a short
            // pause that lets the HTTP response leave first. The .NET wrapper enables the required
            // shutdown privilege itself.
            let state = if hibernate { "Hibernate" } else { "Suspend" };
            let script = format!(
                "Start-Sleep -Milliseconds 1500; Add-Type -AssemblyName System.Windows.Forms; \
                 [System.Windows.Forms.Application]::SetSuspendState('{state}', $false, $false) | Out-Null"
            );
            std::process::Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-WindowStyle",
                    "Hidden",
                    "-Command",
                    &script,
                ])
                .creation_flags(0x0800_0000)
                .spawn()
                .map_err(|e| AppError::internal(format!("failed to start powershell.exe: {e}")))?;
            Ok(())
        }
    }
}

#[cfg(not(windows))]
async fn power_impl(action: PowerAction) -> AppResult<()> {
    Err(AppError::internal(format!(
        "{action:?} is only supported on Windows"
    )))
}

#[cfg(windows)]
use std::os::windows::process::CommandExt as _;

fn detect_shells() -> Vec<ShellInfo> {
    let mut shells = Vec::new();
    #[cfg(windows)]
    {
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        let sys32 = std::path::Path::new(&system_root).join("System32");
        let candidates = [
            ("cmd", "Command Prompt (cmd)", sys32.join("cmd.exe")),
            (
                "powershell",
                "Windows PowerShell",
                sys32.join(r"WindowsPowerShell\v1.0\powershell.exe"),
            ),
            ("wsl", "WSL", sys32.join("wsl.exe")),
        ];
        for (id, name, path) in candidates {
            if path.is_file() {
                shells.push(ShellInfo {
                    id: id.into(),
                    name: name.into(),
                    path: path.display().to_string(),
                });
            }
        }
        if let Some(p) = find_in_path("pwsh.exe") {
            shells.insert(
                2.min(shells.len()),
                ShellInfo {
                    id: "pwsh".into(),
                    name: "PowerShell 7 (pwsh)".into(),
                    path: p.display().to_string(),
                },
            );
        }
    }
    #[cfg(not(windows))]
    {
        if let Some(p) = find_in_path("sh") {
            shells.push(ShellInfo {
                id: "sh".into(),
                name: "sh".into(),
                path: p.display().to_string(),
            });
        }
    }
    shells
}

#[cfg(all(test, windows))]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::actions::shell_command;

    fn platform() -> SystemPlatform {
        SystemPlatform {
            dry_run_power: true,
        }
    }

    async fn run(shell: &str, command: &str) -> ProcessOutput {
        platform()
            .run_process(
                shell_command(shell, command, None),
                CancellationToken::new(),
            )
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn cmd_output_is_utf8_and_exit_codes_are_reported() {
        let out = run("cmd", "echo Привет & echo second").await;
        assert_eq!(out.exit_code, Some(0));
        assert!(out.stdout.contains("Привет"), "{:?}", out.stdout);
        assert!(out.stdout.contains("second"));
        let out = run("cmd", "exit /b 7").await;
        assert_eq!(out.exit_code, Some(7));
    }

    #[tokio::test]
    async fn powershell_output_is_utf8() {
        let out = run(
            "powershell",
            "Write-Output 'Привет'; [Console]::Error.WriteLine('oops')",
        )
        .await;
        assert_eq!(out.exit_code, Some(0));
        assert!(out.stdout.contains("Привет"), "{:?}", out.stdout);
        assert!(out.stderr.contains("oops"));
    }

    #[tokio::test]
    async fn working_directory_is_applied() {
        let dir = std::env::temp_dir();
        let out = platform()
            .run_process(
                shell_command("cmd", "cd", Some(dir.display().to_string())),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let expected = dir
            .display()
            .to_string()
            .trim_end_matches('\\')
            .to_lowercase();
        assert!(
            out.stdout.to_lowercase().contains(&expected),
            "{:?}",
            out.stdout
        );
    }

    #[tokio::test]
    async fn cancel_kills_the_process() {
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            c.cancel();
        });
        let started = Instant::now();
        let res = platform()
            .run_process(
                shell_command("powershell", "Start-Sleep -Seconds 30", None),
                cancel,
            )
            .await;
        assert!(res.is_err());
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn dry_run_skips_power_actions() {
        platform()
            .power(PowerAction::Shutdown { force: true })
            .await
            .unwrap();
        platform()
            .power(PowerAction::Sleep { hibernate: false })
            .await
            .unwrap();
    }

    /// A launcher: PowerShell starts a hidden child that sleeps 3 s, then exits immediately.
    fn launcher() -> ProcessSpec {
        ProcessSpec {
            program: "powershell.exe".into(),
            args: vec![
                "-NoProfile".into(),
                "-WindowStyle".into(),
                "Hidden".into(),
                "-Command".into(),
                "Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile','-Command','Start-Sleep 3'".into(),
            ],
            raw_args: false,
            working_dir: None,
        }
    }

    #[tokio::test]
    async fn process_wait_returns_when_the_launcher_exits() {
        let started = Instant::now();
        let code = platform()
            .launch(launcher(), LaunchWait::Process, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(code, Some(0));
        assert!(
            started.elapsed() < Duration::from_millis(2900),
            "{:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn tree_wait_includes_child_processes() {
        let started = Instant::now();
        let code = platform()
            .launch(launcher(), LaunchWait::Tree, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(code, Some(0));
        assert!(
            started.elapsed() >= Duration::from_secs(3),
            "{:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn cancel_stops_waiting_without_killing() {
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            c.cancel();
        });
        let res = platform()
            .launch(launcher(), LaunchWait::Tree, cancel)
            .await;
        assert!(res.is_err());
    }

    #[test]
    fn counts_processes_by_name() {
        let exe = std::env::current_exe().unwrap();
        let name = exe.file_name().unwrap().to_string_lossy().to_uppercase();
        assert!(platform().process_count(&name) >= 1);
        assert_eq!(platform().process_count("no-such-process-rha.exe"), 0);
    }

    #[test]
    fn detects_builtin_shells() {
        let ids: Vec<String> = detect_shells().into_iter().map(|s| s.id).collect();
        assert!(ids.contains(&"cmd".to_string()));
        assert!(ids.contains(&"powershell".to_string()));
    }

    #[test]
    fn elevation_and_sid_queries_work() {
        let _ = super::super::win::is_elevated();
        let sid = super::super::win::current_user_sid().unwrap();
        assert!(sid.starts_with("S-1-"), "{sid}");
    }
}
