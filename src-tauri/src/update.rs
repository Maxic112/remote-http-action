//! Application updates from GitHub releases: check, verified download, installation.
//!
//! The release feed is the GitHub "latest release" API (`/repos/<owner>/<repo>/releases/latest`)
//! or any server answering in the same format. Installation is done by a small PowerShell
//! helper started right before the application exits:
//! - MSI: waits for the application to exit, runs the installer wizard, restarts the application;
//! - portable: waits for the exit, replaces both executables, restarts the application.
//!
//! Downloads are verified with SHA-256 from the asset `digest` field or a `SHA256SUMS.txt` asset.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::core::Core;
use crate::error::{AppError, AppResult, ErrorCode};
use crate::events::AppEvent;
use crate::model::now;

/// GitHub repository the releases are published in.
pub const REPOSITORY: &str = "remote-http-action/remote-http-action";
pub const STATE_KEY: &str = "update_state";
pub const CHECKSUMS_ASSET: &str = "SHA256SUMS.txt";
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 3600);
const MAIN_EXE: &str = "remote-http-action.exe";
const CLI_EXE: &str = "remote-http-action-cli.exe";

pub fn default_feed_url() -> String {
    format!("https://api.github.com/repos/{REPOSITORY}/releases/latest")
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// `sha256:<hex>` as reported by GitHub.
    #[serde(default)]
    pub digest: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ReleaseInfo {
    pub version: String,
    pub tag: String,
    pub name: String,
    pub notes: String,
    pub published_at: Option<String>,
    pub html_url: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

/// Parses a release in the GitHub API format.
pub fn parse_release(v: &Value) -> AppResult<ReleaseInfo> {
    let tag = v["tag_name"]
        .as_str()
        .ok_or_else(|| AppError::validation("release feed: tag_name is missing"))?
        .to_string();
    let version = tag.trim_start_matches(['v', 'V']).to_string();
    semver::Version::parse(&version)
        .map_err(|_| AppError::validation(format!("release feed: \"{tag}\" is not a version")))?;
    let assets = v["assets"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|a| {
                    Some(ReleaseAsset {
                        name: a["name"].as_str()?.to_string(),
                        url: a["browser_download_url"].as_str()?.to_string(),
                        size: a["size"].as_u64().unwrap_or(0),
                        digest: a["digest"].as_str().map(str::to_string),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let text = |k: &str| v[k].as_str().map(str::to_string);
    Ok(ReleaseInfo {
        name: text("name")
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| tag.clone()),
        notes: text("body").unwrap_or_default(),
        published_at: text("published_at"),
        html_url: text("html_url"),
        version,
        tag,
        assets,
    })
}

/// `latest` is a higher version than `current` (semantic versioning, `v` prefix allowed).
pub fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| semver::Version::parse(s.trim_start_matches(['v', 'V'])).ok();
    match (parse(latest), parse(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InstallKind {
    Msi,
    Portable,
    /// Development builds are never replaced by an update.
    Development,
}

/// Picks the release file for this installation: the MSI in the interface language
/// (falling back to the other one) or the portable archive.
pub fn pick_asset(
    release: &ReleaseInfo,
    kind: InstallKind,
    russian: bool,
) -> Option<&ReleaseAsset> {
    let find = |suffix: &str| {
        release
            .assets
            .iter()
            .find(|a| a.name.to_lowercase().ends_with(&suffix.to_lowercase()))
    };
    match kind {
        InstallKind::Msi => {
            let (first, second) = if russian {
                ("_x64_ru-RU.msi", "_x64_en-US.msi")
            } else {
                ("_x64_en-US.msi", "_x64_ru-RU.msi")
            };
            find(first).or_else(|| find(second))
        }
        InstallKind::Portable => find("_x64_portable.zip"),
        InstallKind::Development => None,
    }
}

/// Expected SHA-256 (lowercase hex) of an asset, from its digest or a `SHA256SUMS.txt` text.
pub fn expected_sha256(asset: &ReleaseAsset, sums: Option<&str>) -> Option<String> {
    if let Some(hex) = asset
        .digest
        .as_deref()
        .and_then(|d| d.strip_prefix("sha256:"))
    {
        return Some(hex.to_lowercase());
    }
    sums?.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        (name == asset.name && hash.len() == 64).then(|| hash.to_lowercase())
    })
}

fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// PowerShell helper that applies a prepared update after the application (`pid`) has exited.
pub fn helper_script(
    kind: InstallKind,
    pid: u32,
    package: &Path,
    app_exe: &Path,
    work_dir: &Path,
    log: &Path,
) -> String {
    let p = |path: &Path| ps_quote(&path.display().to_string());
    let apply = match kind {
        InstallKind::Msi => format!(
            "$installer = Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', ('\"' + {package} + '\"')) -Wait -PassThru\n\
             \"msiexec exit code: $($installer.ExitCode)\"",
            package = p(package)
        ),
        _ => {
            // Split by hand: the path is a Windows path even when this runs elsewhere (tests).
            let exe = app_exe.display().to_string();
            let dest = PathBuf::from(exe.rsplit_once(['\\', '/']).map_or(".", |(dir, _)| dir));
            let dest = dest.as_path();
            format!(
                "foreach ($file in @({main}, {cli})) {{\n\
                 \x20 $from = Join-Path {src} $file\n\
                 \x20 if (-not (Test-Path -LiteralPath $from)) {{ continue }}\n\
                 \x20 for ($i = 0; $i -lt 30; $i++) {{\n\
                 \x20   try {{ Copy-Item -LiteralPath $from -Destination (Join-Path {dst} $file) -Force -ErrorAction Stop; \"updated $file\"; break }}\n\
                 \x20   catch {{ Start-Sleep -Milliseconds 500 }}\n\
                 \x20 }}\n\
                 }}",
                main = ps_quote(MAIN_EXE),
                cli = ps_quote(CLI_EXE),
                src = p(package),
                dst = p(dest),
            )
        }
    };
    format!(
        "$ErrorActionPreference = 'Continue'\n\
         Start-Transcript -LiteralPath {log} -Force | Out-Null\n\
         try {{ Wait-Process -Id {pid} -Timeout 120 -ErrorAction Stop }} catch {{ }}\n\
         {apply}\n\
         Start-Process -FilePath {exe}\n\
         Stop-Transcript | Out-Null\n\
         Remove-Item -LiteralPath {work} -Recurse -Force -ErrorAction SilentlyContinue\n",
        log = p(log),
        exe = p(app_exe),
        work = p(work_dir),
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct UpdateState {
    pub last_check_at: Option<String>,
    pub latest: Option<ReleaseInfo>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    /// Downloaded and verified; waiting for the user to install.
    Ready {
        version: String,
    },
    Installing,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub install_kind: InstallKind,
    pub auto_check: bool,
    pub feed_url: String,
    pub last_check_at: Option<String>,
    pub available: bool,
    pub latest: Option<ReleaseInfo>,
    /// File that would be downloaded for this installation.
    pub asset: Option<ReleaseAsset>,
    pub error: Option<String>,
    pub phase: Phase,
}

struct Prepared {
    version: String,
    kind: InstallKind,
    /// MSI file, or the folder with the new executables for the portable version.
    package: PathBuf,
}

pub struct Updater {
    /// Download and helper folder.
    dir: PathBuf,
    http: reqwest::Client,
    phase: Mutex<Phase>,
    prepared: Mutex<Option<Prepared>>,
    busy: tokio::sync::Mutex<()>,
}

impl Default for Updater {
    fn default() -> Self {
        Self::new()
    }
}

/// Temporary folder for downloads and the helper script.
pub fn work_dir() -> PathBuf {
    std::env::temp_dir().join("remote-http-action-update")
}

/// Removes files left from a previous update (called at start-up).
pub fn cleanup_leftovers() {
    let _ = std::fs::remove_dir_all(work_dir());
}

fn status_err(resp: &reqwest::Response, what: &str) -> AppError {
    match resp.status().as_u16() {
        404 => AppError::not_found(format!("{what}: no published release found")),
        403 | 429 => AppError::new(
            ErrorCode::TooManyRequests,
            format!("{what}: rate limited ({})", resp.status()),
        ),
        _ => AppError::internal(format!("{what}: server answered {}", resp.status())),
    }
}

impl Updater {
    pub fn new() -> Self {
        crate::tls::ensure_crypto_provider();
        let http = reqwest::Client::builder()
            .user_agent(concat!("remote-http-action/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("HTTP client");
        // Tests run in parallel: each updater gets its own folder.
        let dir = if cfg!(test) {
            std::env::temp_dir().join(format!("rha-update-test-{}", uuid::Uuid::new_v4()))
        } else {
            work_dir()
        };
        Self {
            dir,
            http,
            phase: Mutex::new(Phase::Idle),
            prepared: Mutex::new(None),
            busy: tokio::sync::Mutex::new(()),
        }
    }

    pub fn work_dir(&self) -> &Path {
        &self.dir
    }

    fn set_phase(&self, core: &Core, phase: Phase) {
        *self.phase.lock().unwrap() = phase;
        core.events.emit(AppEvent::UpdateChanged);
    }

    pub fn install_kind(core: &Core) -> InstallKind {
        if cfg!(debug_assertions) && !cfg!(test) {
            InstallKind::Development
        } else if core.paths.portable {
            InstallKind::Portable
        } else {
            InstallKind::Msi
        }
    }

    fn feed_url(core: &Core) -> String {
        let s = core.settings().updates.feed_url;
        if s.is_empty() { default_feed_url() } else { s }
    }

    pub fn status(&self, core: &Core) -> UpdateStatus {
        let state: UpdateState = core
            .db
            .get_state(STATE_KEY)
            .ok()
            .flatten()
            .unwrap_or_default();
        let current = env!("CARGO_PKG_VERSION").to_string();
        let kind = Self::install_kind(core);
        let russian = crate::settings::prefers_russian(&core.settings().app.language);
        let available = state
            .latest
            .as_ref()
            .is_some_and(|l| is_newer(&l.version, &current));
        UpdateStatus {
            asset: state
                .latest
                .as_ref()
                .and_then(|l| pick_asset(l, kind, russian))
                .cloned(),
            current_version: current,
            install_kind: kind,
            auto_check: core.settings().updates.auto_check,
            feed_url: Self::feed_url(core),
            last_check_at: state.last_check_at,
            available,
            latest: state.latest,
            error: state.error,
            phase: self.phase.lock().unwrap().clone(),
        }
    }

    /// Asks the release feed for the latest version and stores the result.
    pub async fn check(&self, core: &Arc<Core>) -> AppResult<UpdateStatus> {
        let _busy = self
            .busy
            .try_lock()
            .map_err(|_| AppError::conflict("an update operation is already in progress"))?;
        self.set_phase(core, Phase::Checking);
        let result = self.fetch_latest(&Self::feed_url(core)).await;
        let mut state: UpdateState = core.db.get_state(STATE_KEY)?.unwrap_or_default();
        state.last_check_at = Some(now());
        match &result {
            Ok(release) => {
                state.latest = Some(release.clone());
                state.error = None;
            }
            Err(e) => state.error = Some(e.message.clone()),
        }
        core.db.set_state(STATE_KEY, &state)?;
        let phase = match &*self.prepared.lock().unwrap() {
            Some(p) => Phase::Ready {
                version: p.version.clone(),
            },
            None => Phase::Idle,
        };
        self.set_phase(core, phase);
        result.map(|_| self.status(core))
    }

    async fn fetch_latest(&self, url: &str) -> AppResult<ReleaseInfo> {
        let resp = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| AppError::internal(format!("update check failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(status_err(&resp, "update check"));
        }
        let json: Value = resp
            .json()
            .await
            .map_err(|e| AppError::internal(format!("update check: invalid answer: {e}")))?;
        parse_release(&json)
    }

    /// Downloads and verifies the update for this installation.
    pub async fn download(&self, core: &Arc<Core>) -> AppResult<UpdateStatus> {
        let _busy = self
            .busy
            .try_lock()
            .map_err(|_| AppError::conflict("an update operation is already in progress"))?;
        let status = self.status(core);
        if status.install_kind == InstallKind::Development {
            return Err(AppError::new(
                ErrorCode::Forbidden,
                "updates are installed only in release builds",
            ));
        }
        let latest = status
            .latest
            .clone()
            .filter(|_| status.available)
            .ok_or_else(|| {
                AppError::not_found("no newer version is known; check for updates first")
            })?;
        let asset = status.asset.clone().ok_or_else(|| {
            AppError::not_found("the release has no file for this installation type")
        })?;

        let result = self
            .download_verified(core, &latest, &asset, status.install_kind)
            .await;
        match result {
            Ok(package) => {
                *self.prepared.lock().unwrap() = Some(Prepared {
                    version: latest.version.clone(),
                    kind: status.install_kind,
                    package,
                });
                self.set_phase(
                    core,
                    Phase::Ready {
                        version: latest.version,
                    },
                );
                Ok(self.status(core))
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&self.dir);
                self.set_phase(core, Phase::Idle);
                Err(e)
            }
        }
    }

    async fn download_verified(
        &self,
        core: &Arc<Core>,
        release: &ReleaseInfo,
        asset: &ReleaseAsset,
        kind: InstallKind,
    ) -> AppResult<PathBuf> {
        let sums = if asset.digest.is_none() {
            match release
                .assets
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case(CHECKSUMS_ASSET))
            {
                Some(s) => Some(self.fetch_text(&s.url).await?),
                None => None,
            }
        } else {
            None
        };
        let expected = expected_sha256(asset, sums.as_deref()).ok_or_else(|| {
            AppError::validation("the release provides no SHA-256 checksum for the file")
        })?;

        let dir = self.dir.clone();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir)?;
        let file_path = dir.join(&asset.name);

        let mut resp = self
            .http
            .get(&asset.url)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("download failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(status_err(&resp, "download"));
        }
        let total = resp
            .content_length()
            .or(Some(asset.size).filter(|s| *s > 0));
        let mut file = std::fs::File::create(&file_path)?;
        let mut hasher = Sha256::new();
        let (mut downloaded, mut reported) = (0u64, 0u64);
        self.set_phase(core, Phase::Downloading { downloaded, total });
        while let Some(chunk) = resp
            .chunk()
            .await
            .map_err(|e| AppError::internal(format!("download interrupted: {e}")))?
        {
            hasher.update(&chunk);
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            if downloaded - reported >= 512 * 1024 {
                reported = downloaded;
                self.set_phase(core, Phase::Downloading { downloaded, total });
            }
        }
        file.flush()?;
        drop(file);
        let actual = hex::encode(hasher.finalize());
        if actual != expected {
            return Err(AppError::validation(format!(
                "checksum mismatch for {}: expected {expected}, got {actual}",
                asset.name
            )));
        }
        match kind {
            InstallKind::Portable => extract_portable(&file_path, &dir.join("staging")),
            _ => Ok(file_path),
        }
    }

    async fn fetch_text(&self, url: &str) -> AppResult<String> {
        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| AppError::internal(format!("download failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(status_err(&resp, "checksum download"));
        }
        resp.text()
            .await
            .map_err(|e| AppError::internal(e.to_string()))
    }

    /// Starts the helper and asks the application to quit. The helper finishes the update.
    pub fn install(&self, core: &Arc<Core>) -> AppResult<()> {
        let prepared = self.prepared.lock().unwrap();
        let p = prepared
            .as_ref()
            .ok_or_else(|| AppError::conflict("the update has not been downloaded yet"))?;
        let exe = std::env::current_exe().map_err(|e| AppError::internal(e.to_string()))?;
        let dir = self.dir.clone();
        let log = core.logs.dir().join("update.log");
        let _ = std::fs::create_dir_all(core.logs.dir());
        let script = dir.join("apply-update.ps1");
        // UTF-8 with BOM so Windows PowerShell reads non-ASCII paths correctly.
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend(
            helper_script(p.kind, std::process::id(), &p.package, &exe, &dir, &log).into_bytes(),
        );
        std::fs::write(&script, bytes)?;
        spawn_helper(&script)?;
        drop(prepared);
        self.set_phase(core, Phase::Installing);
        let core = core.clone();
        tokio::spawn(async move {
            // Let the UI or CLI receive the answer first.
            tokio::time::sleep(Duration::from_millis(500)).await;
            core.control.quit();
        });
        Ok(())
    }
}

/// Extracts the portable archive and returns the folder that contains the executables.
pub fn extract_portable(zip_path: &Path, dest: &Path) -> AppResult<PathBuf> {
    let file = std::fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::validation(format!("invalid archive: {e}")))?;
    let mut exe_dir = None;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| AppError::validation(format!("invalid archive: {e}")))?;
        // Names with `..` or absolute paths are skipped.
        let Some(rel) = entry.enclosed_name() else {
            continue;
        };
        let out = dest.join(rel);
        if entry.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut f = std::fs::File::create(&out)?;
        std::io::copy(&mut entry, &mut f)?;
        if out
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case(MAIN_EXE))
        {
            exe_dir = out.parent().map(Path::to_path_buf);
        }
    }
    exe_dir.ok_or_else(|| AppError::validation(format!("the archive does not contain {MAIN_EXE}")))
}

#[cfg(windows)]
fn spawn_helper(script: &Path) -> AppResult<()> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_BREAKAWAY_FROM_JOB: u32 = 0x0100_0000;
    let spawn = |flags: u32| {
        std::process::Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-WindowStyle",
                "Hidden",
                "-File",
            ])
            .arg(script)
            .creation_flags(flags)
            .spawn()
    };
    // Leave the job object of the application (e.g. when it was started by Task Scheduler),
    // so the helper survives the application's exit. Not every job allows that.
    spawn(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB)
        .or_else(|_| spawn(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP))
        .map(|_| ())
        .map_err(|e| AppError::internal(format!("cannot start the update helper: {e}")))
}

#[cfg(not(windows))]
fn spawn_helper(_script: &Path) -> AppResult<()> {
    Err(AppError::internal("updates are installed only on Windows"))
}

/// Checks once a day while automatic checks are enabled. The first check runs a minute after start.
pub async fn auto_check_loop(core: Arc<Core>) {
    tokio::time::sleep(Duration::from_secs(60)).await;
    loop {
        if core.settings().updates.auto_check {
            let state: UpdateState = core
                .db
                .get_state(STATE_KEY)
                .ok()
                .flatten()
                .unwrap_or_default();
            let due = state
                .last_check_at
                .as_deref()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .is_none_or(|t| {
                    chrono::Utc::now()
                        .signed_duration_since(t)
                        .to_std()
                        .unwrap_or_default()
                        >= CHECK_INTERVAL
                });
            if due {
                let _ = core.updates.check(&core).await;
            }
        }
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}

#[cfg(test)]
mod tests;
