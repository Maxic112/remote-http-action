//! Data directory and IPC endpoint locations.

use std::path::{Path, PathBuf};

use serde::Serialize;

pub const APP_DIR_NAME: &str = "remote-http-action";
pub const PORTABLE_FLAG: &str = "portable.flag";

#[derive(Debug, Clone, Serialize)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub portable: bool,
}

impl AppPaths {
    pub fn at(dir: &Path, portable: bool) -> Self {
        Self {
            data_dir: dir.to_path_buf(),
            db_path: dir.join("data.db"),
            portable,
        }
    }

    /// Resolution order: `RHA_DATA_DIR`, portable mode (`portable.flag` next to the exe → `data\`),
    /// then `%APPDATA%\remote-http-action` (`-dev` suffix in development builds).
    pub fn resolve() -> Self {
        if let Some(dir) = std::env::var_os("RHA_DATA_DIR") {
            return Self::at(Path::new(&dir), false);
        }
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            && exe_dir.join(PORTABLE_FLAG).is_file()
        {
            return Self::at(&exe_dir.join("data"), true);
        }
        let base = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        let name = if cfg!(debug_assertions) {
            format!("{APP_DIR_NAME}-dev")
        } else {
            APP_DIR_NAME.to_string()
        };
        Self::at(&base.join(name), false)
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.data_dir)
    }

    /// Named pipe (Windows) or Unix socket used by the CLI. Overridable with `RHA_IPC_ENDPOINT`.
    pub fn ipc_endpoint(&self) -> String {
        if let Ok(e) = std::env::var("RHA_IPC_ENDPOINT") {
            return e;
        }
        if cfg!(windows) {
            let suffix = if cfg!(debug_assertions) { "-dev" } else { "" };
            format!(r"\\.\pipe\{APP_DIR_NAME}{suffix}")
        } else {
            self.data_dir.join("ipc.sock").display().to_string()
        }
    }
}
