//! Tauri shell: window, tray, `api` command and event forwarding.

use std::sync::{Arc, Mutex};

use serde_json::Value;
use tauri::menu::{CheckMenuItem, Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, WindowEvent, Wry};

use crate::core::{AppControl, Caller, Core, Request};
use crate::db::Db;
use crate::error::AppError;
use crate::events::{AppEvent, EventSink};
use crate::http::ServerState;
use crate::paths::AppPaths;
use crate::platform::system::SystemPlatform;

const MAIN_WINDOW: &str = "main";

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_WINDOW) {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

struct TauriSink {
    app: AppHandle,
}

impl EventSink for TauriSink {
    fn emit(&self, event: AppEvent) {
        match &event {
            AppEvent::RunDelayed { .. } => show_main(&self.app),
            AppEvent::ServerStatusChanged => sync_server_item(&self.app),
            AppEvent::DataChanged { entity: "settings" } => refresh_tray_menu(&self.app),
            _ => {}
        }
        let _ = self.app.emit("app-event", &event);
    }
}

struct GuiControl {
    app: AppHandle,
}

impl AppControl for GuiControl {
    fn show_window(&self) {
        show_main(&self.app);
    }
    fn quit(&self) {
        self.app.exit(0);
    }
}

/// Tray menu state: the server check item and the language the menu was built with.
#[derive(Default)]
struct TrayMenu(Mutex<Option<(CheckMenuItem<Wry>, bool)>>);

fn sync_server_item(app: &AppHandle) {
    if let (Some(core), Some(menu)) = (app.try_state::<Arc<Core>>(), app.try_state::<TrayMenu>())
        && let Some((item, _)) = menu.0.lock().unwrap().as_ref()
    {
        let _ = item.set_checked(core.http.status().state == ServerState::Running);
    }
}

fn build_menu(app: &AppHandle, ru: bool) -> tauri::Result<(Menu<Wry>, CheckMenuItem<Wry>)> {
    let t = |ru_text: &'static str, en_text: &'static str| if ru { ru_text } else { en_text };
    let open = MenuItem::with_id(app, "open", t("Открыть", "Open"), true, None::<&str>)?;
    let server = CheckMenuItem::with_id(
        app,
        "server",
        t("HTTP-сервер", "HTTP server"),
        true,
        false,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", t("Выход", "Quit"), true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &server, &quit])?;
    Ok((menu, server))
}

/// Rebuilds the tray menu when the interface language changes.
fn refresh_tray_menu(app: &AppHandle) {
    let (Some(core), Some(state), Some(tray)) = (
        app.try_state::<Arc<Core>>(),
        app.try_state::<TrayMenu>(),
        app.tray_by_id("main"),
    ) else {
        return;
    };
    let ru = ui_is_russian(&core.settings().app.language);
    if state
        .0
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|(_, lang)| *lang == ru)
    {
        return;
    }
    if let Ok((menu, server)) = build_menu(app, ru) {
        let _ = tray.set_menu(Some(menu));
        *state.0.lock().unwrap() = Some((server, ru));
        sync_server_item(app);
    }
}

#[tauri::command]
async fn api(core: tauri::State<'_, Arc<Core>>, request: Request) -> Result<Value, AppError> {
    let core = core.inner().clone();
    core.handle(Caller::Ui, request).await
}

use crate::settings::prefers_russian as ui_is_russian;

fn build_tray(app: &AppHandle, core: &Arc<Core>) -> tauri::Result<()> {
    let ru = ui_is_russian(&core.settings().app.language);
    let (menu, server) = build_menu(app, ru)?;
    app.manage(TrayMenu(Mutex::new(Some((server, ru)))));

    let mut tray = TrayIconBuilder::with_id("main")
        .tooltip("Remote HTTP Action")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "quit" => app.exit(0),
            "server" => {
                let core = app.state::<Arc<Core>>().inner().clone();
                tauri::async_runtime::spawn(async move {
                    let running = core.http.status().state == ServerState::Running;
                    let req = if running {
                        Request::ServerStop
                    } else {
                        Request::ServerStart
                    };
                    let _ = core.handle(Caller::Ui, req).await;
                    core.events.emit(AppEvent::ServerStatusChanged);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

fn init_core(app: &AppHandle) -> Result<Arc<Core>, Box<dyn std::error::Error>> {
    let paths = AppPaths::resolve();
    paths.ensure()?;
    let db = Arc::new(Db::open(&paths.db_path)?);
    let core = Core::new(
        db,
        Arc::new(SystemPlatform::from_env()),
        Arc::new(TauriSink { app: app.clone() }),
        Arc::new(GuiControl { app: app.clone() }),
        paths,
    )?;
    Ok(core)
}

pub fn run(start_minimized: bool) {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![api])
        .setup(move |app| {
            let handle = app.handle().clone();
            let core = init_core(&handle)?;
            app.manage(core.clone());
            build_tray(&handle, &core)?;

            let endpoint = core.paths.ipc_endpoint();
            let ipc_core = core.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = crate::ipc::serve(ipc_core, endpoint).await {
                    eprintln!("CLI channel stopped: {e}");
                }
            });
            let http_core = core.clone();
            tauri::async_runtime::spawn(async move {
                http_core.start_server_if_enabled().await;
                http_core.events.emit(AppEvent::ServerStatusChanged);
            });
            crate::update::cleanup_leftovers();
            tauri::async_runtime::spawn(crate::update::auto_check_loop(core.clone()));

            if !start_minimized {
                show_main(&handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let close_to_tray = window
                    .app_handle()
                    .try_state::<Arc<Core>>()
                    .is_some_and(|c| c.settings().app.close_to_tray);
                if close_to_tray {
                    api.prevent_close();
                    let _ = window.hide();
                } else {
                    window.app_handle().exit(0);
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
