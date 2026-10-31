//! Operating-system integration behind a trait, so the core can be tested without
//! shutting down or suspending the machine.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::error::AppResult;

#[cfg(test)]
pub mod mock;
pub mod system;
#[cfg(windows)]
pub mod win;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PowerAction {
    Shutdown { force: bool },
    Reboot { force: bool },
    Sleep { hibernate: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    /// Windows only: pass `args` joined verbatim instead of quoting each one (needed for `cmd /C`).
    pub raw_args: bool,
    pub working_dir: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShellInfo {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[async_trait]
pub trait Platform: Send + Sync {
    /// Whether the current process runs with administrator rights.
    fn is_elevated(&self) -> bool;
    async fn power(&self, action: PowerAction) -> AppResult<()>;
    /// Runs a process to completion and captures its output. Killed when `cancel` fires.
    async fn run_process(
        &self,
        spec: ProcessSpec,
        cancel: CancellationToken,
    ) -> AppResult<ProcessOutput>;
    /// Shells available for `cli_command`.
    fn shells(&self) -> Vec<ShellInfo>;
    /// Starts an elevated copy of the application (UAC prompt). The caller exits afterwards.
    fn relaunch_elevated(&self) -> AppResult<()>;
}

/// Searches `PATH` for an executable.
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}
