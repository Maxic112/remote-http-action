//! Operating-system integration behind a trait, so the core can be tested without
//! shutting down or suspending the machine.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

use crate::error::AppResult;

pub mod autostart;
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

/// How `Platform::launch` waits for a started program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchWait {
    /// Return as soon as the program has started.
    None,
    /// Wait for the started process.
    Process,
    /// Wait for the started process and every process it starts (Windows job object).
    Tree,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AutostartStatus {
    pub supported: bool,
    pub installed: bool,
    /// Command registered in the scheduled task, if any.
    pub command: Option<String>,
    /// The task starts this executable (it may point to an older location after a move).
    pub matches_current: bool,
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
    /// Starts a program with its own window. While waiting, returns the exit code of the started
    /// process. `cancel` stops waiting; the program keeps running.
    async fn launch(
        &self,
        spec: ProcessSpec,
        wait: LaunchWait,
        cancel: CancellationToken,
    ) -> AppResult<Option<i32>>;
    /// Number of running processes with this executable name (case-insensitive, e.g. `steam.exe`).
    fn process_count(&self, name: &str) -> usize;
    /// Opens a folder in the file manager.
    fn open_folder(&self, path: &std::path::Path) -> AppResult<()>;
    fn autostart_status(&self) -> AppResult<AutostartStatus>;
    /// Registers start at logon with administrator rights.
    fn autostart_install(&self) -> AppResult<()>;
    fn autostart_remove(&self) -> AppResult<()>;
}

/// Searches `PATH` for an executable.
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}
