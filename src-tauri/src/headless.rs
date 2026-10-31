//! Running without a window or tray icon (`--headless`): HTTP server and CLI channel only.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use crate::core::{AppControl, Core};
use crate::db::Db;
use crate::events::NoopSink;
use crate::paths::AppPaths;
use crate::platform::system::SystemPlatform;

struct HeadlessControl {
    quit: Arc<Notify>,
}

impl AppControl for HeadlessControl {
    fn show_window(&self) {}
    fn quit(&self) {
        self.quit.notify_one();
    }
}

/// Runs until `app quit` or Ctrl+C. Returns the process exit code.
pub fn run() -> i32 {
    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    rt.block_on(async {
        let paths = AppPaths::resolve();
        if let Err(e) = paths.ensure() {
            eprintln!("error: cannot create {}: {e}", paths.data_dir.display());
            return 1;
        }
        let db = match Db::open(&paths.db_path) {
            Ok(db) => Arc::new(db),
            Err(e) => {
                eprintln!("error: {}", e.message);
                return 1;
            }
        };
        let quit = Arc::new(Notify::new());
        let core = match Core::new(
            db,
            Arc::new(SystemPlatform::from_env()),
            Arc::new(NoopSink),
            Arc::new(HeadlessControl { quit: quit.clone() }),
            paths,
        ) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {}", e.message);
                return 1;
            }
        };

        let endpoint = core.paths.ipc_endpoint();
        let ipc_core = core.clone();
        tokio::spawn(async move {
            if let Err(e) = crate::ipc::serve(ipc_core, endpoint).await {
                eprintln!("CLI channel stopped: {e}");
            }
        });
        core.start_server_if_enabled().await;
        crate::update::cleanup_leftovers();
        tokio::spawn(crate::update::auto_check_loop(core.clone()));
        let status = core.http.status();
        eprintln!(
            "remote-http-action running headless; HTTP server: {:?} {}",
            status.state,
            status.address.or(status.error).unwrap_or_default()
        );

        tokio::select! {
            _ = quit.notified() => {
                // Let the `app quit` response reach the CLI before exiting.
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
            _ = tokio::signal::ctrl_c() => {}
        }
        core.http.stop(&core).await;
        0
    })
}
