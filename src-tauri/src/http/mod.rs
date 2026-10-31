//! HTTP API server.

pub mod guard;
pub mod net;

use std::cell::RefCell;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use axum::Router;
use axum::extract::{ConnectInfo, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use serde::Serialize;
use serde_json::{Value, json};

use crate::core::Core;
use crate::error::{AppError, ErrorCode};
use crate::events::AppEvent;
use crate::logs::LogCategory;
use crate::model::{ApiKey, Run, RunSource, RunStatus};
use crate::runner::RunRequest;
use crate::secrets::hash_key;
use crate::settings::parse_nets;
use net::{host_allowed, ip_allowed, resolve_client_ip};

pub const RUNS_PREFIX: &str = "/runs/";

tokio::task_local! {
    /// Name of the API key that authenticated the current request (for the HTTP log).
    static REQUEST_KEY: RefCell<Option<String>>;
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServerState {
    #[default]
    Stopped,
    Running,
    Error,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct ServerStatus {
    pub state: ServerState,
    pub address: Option<String>,
    pub error: Option<String>,
    /// The server uses HTTPS.
    pub tls: bool,
}

struct Running {
    handle: Handle<SocketAddr>,
    task: tokio::task::JoinHandle<()>,
}

#[derive(Default)]
pub struct HttpServer {
    running: tokio::sync::Mutex<Option<Running>>,
    status: RwLock<ServerStatus>,
}

impl HttpServer {
    pub fn status(&self) -> ServerStatus {
        self.status.read().unwrap().clone()
    }

    fn set_status(&self, core: &Core, status: ServerStatus) {
        *self.status.write().unwrap() = status;
        core.events.emit(AppEvent::ServerStatusChanged);
    }

    fn fail(&self, core: &Core, address: String, error: String, tls: bool) -> ServerStatus {
        let status = ServerStatus {
            state: ServerState::Error,
            address: Some(address),
            error: Some(error),
            tls,
        };
        self.set_status(core, status.clone());
        status
    }

    /// (Re)starts the server on the configured address, with HTTPS if enabled.
    pub async fn start(&self, core: Arc<Core>) -> ServerStatus {
        self.stop_inner().await;
        let s = core.settings().server;
        let addr = format!("{}:{}", s.bind_address, s.port);
        let tls = if s.tls.enabled {
            match crate::tls::load(&s.tls, &core.paths.data_dir).await {
                Ok(c) => Some(c),
                Err(e) => return self.fail(&core, addr, e.message, true),
            }
        } else {
            None
        };
        match std::net::TcpListener::bind(&addr) {
            Ok(listener) => self.serve(core, listener, tls).await,
            Err(e) => {
                let msg = format!("cannot listen on {addr}: {e}");
                self.fail(&core, addr, msg, s.tls.enabled)
            }
        }
    }

    /// Starts serving on an already bound listener; HTTPS when `tls` is given.
    pub async fn serve(
        &self,
        core: Arc<Core>,
        listener: std::net::TcpListener,
        tls: Option<RustlsConfig>,
    ) -> ServerStatus {
        self.stop_inner().await;
        let addr = listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_default();
        let is_tls = tls.is_some();
        if let Err(e) = listener.set_nonblocking(true) {
            return self.fail(&core, addr, e.to_string(), is_tls);
        }
        let make_service = Router::new()
            .fallback(handle)
            .with_state(core.clone())
            .into_make_service_with_connect_info::<SocketAddr>();
        let handle = Handle::new();
        let task = match tls {
            Some(config) => match axum_server::from_tcp_rustls(listener, config) {
                Ok(server) => {
                    let server = server.handle(handle.clone());
                    tokio::spawn(async move {
                        let _ = server.serve(make_service).await;
                    })
                }
                Err(e) => return self.fail(&core, addr, e.to_string(), true),
            },
            None => match axum_server::from_tcp(listener) {
                Ok(server) => {
                    let server = server.handle(handle.clone());
                    tokio::spawn(async move {
                        let _ = server.serve(make_service).await;
                    })
                }
                Err(e) => return self.fail(&core, addr, e.to_string(), false),
            },
        };
        *self.running.lock().await = Some(Running { handle, task });
        let status = ServerStatus {
            state: ServerState::Running,
            address: Some(addr),
            error: None,
            tls: is_tls,
        };
        self.set_status(&core, status.clone());
        status
    }

    pub async fn stop(&self, core: &Core) -> ServerStatus {
        self.stop_inner().await;
        let status = ServerStatus::default();
        self.set_status(core, status.clone());
        status
    }

    async fn stop_inner(&self) {
        if let Some(r) = self.running.lock().await.take() {
            // In-flight sync requests may wait for long flows; don't block shutdown on them.
            r.handle.graceful_shutdown(Some(Duration::from_secs(2)));
            let _ = tokio::time::timeout(Duration::from_secs(5), r.task).await;
        }
    }
}

fn error_response(e: &AppError) -> Response {
    let status =
        StatusCode::from_u16(e.code.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    (
        status,
        axum::Json(json!({ "error": { "code": e.code, "message": e.message } })),
    )
        .into_response()
}

fn err(code: ErrorCode, message: &str) -> Response {
    error_response(&AppError::new(code, message))
}

fn extract_key(headers: &HeaderMap) -> Option<String> {
    if let Some(v) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        let v = v.trim();
        if v.len() > 7 && v[..7].eq_ignore_ascii_case("bearer ") {
            return Some(v[7..].trim().to_string());
        }
    }
    headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

/// Validates the API key. `Ok(None)` = no key sent and none required.
#[allow(clippy::result_large_err)]
fn authenticate(
    core: &Core,
    headers: &HeaderMap,
    ip: std::net::IpAddr,
    required: bool,
) -> Result<Option<ApiKey>, Response> {
    let Some(secret) = extract_key(headers) else {
        return if required {
            Err(err(ErrorCode::Unauthorized, "API key required"))
        } else {
            Ok(None)
        };
    };
    match core.db.find_key_by_hash(&hash_key(&secret)) {
        Ok(Some(key)) if key.enabled => {
            let _ = core.db.touch_key(key.id);
            let _ = REQUEST_KEY.try_with(|k| *k.borrow_mut() = Some(key.name.clone()));
            Ok(Some(key))
        }
        Ok(_) => {
            let cfg = core.settings().server.brute_force;
            core.guard.record_failure(ip, &cfg, Instant::now());
            Err(err(ErrorCode::Unauthorized, "invalid API key"))
        }
        Err(e) => Err(error_response(&e)),
    }
}

fn normalize_path(path: &str) -> String {
    let t = path.trim_matches('/');
    if t.is_empty() {
        "/".into()
    } else {
        format!("/{t}/")
    }
}

async fn handle(
    State(core): State<Arc<Core>>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
) -> Response {
    let started = Instant::now();
    let trusted = parse_nets(&core.settings().server.trusted_proxies).unwrap_or_default();
    let xff = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok());
    let ip = resolve_client_ip(peer.ip(), xff, &trusted);
    let (response, key) = REQUEST_KEY
        .scope(RefCell::new(None), async {
            let response = handle_inner(&core, ip, &method, &uri, &headers).await;
            (response, REQUEST_KEY.with(|k| k.borrow().clone()))
        })
        .await;
    core.logs.write(
        LogCategory::Http,
        json!({
            "ip": ip.to_string(),
            "method": method.as_str(),
            "path": uri.path(),
            "status": response.status().as_u16(),
            "key": key,
            "duration_ms": started.elapsed().as_millis() as u64,
            "user_agent": headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()),
        }),
    );
    response
}

async fn handle_inner(
    core: &Arc<Core>,
    ip: std::net::IpAddr,
    method: &Method,
    uri: &Uri,
    headers: &HeaderMap,
) -> Response {
    let s = core.settings().server;
    let method = method.clone();
    let headers = headers.clone();
    let core = core.clone();

    if !ip_allowed(ip, &parse_nets(&s.allowed_ips).unwrap_or_default()) {
        return err(ErrorCode::Forbidden, "client address is not allowed");
    }
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    if !host_allowed(host, &s.allowed_hosts) {
        return err(ErrorCode::Forbidden, "host is not allowed");
    }
    if let Some(left) = core.guard.banned(ip, Instant::now()) {
        let mut r = err(
            ErrorCode::TooManyRequests,
            "too many failed authentication attempts",
        );
        if let Ok(v) = HeaderValue::from_str(&left.as_secs().max(1).to_string()) {
            r.headers_mut().insert(header::RETRY_AFTER, v);
        }
        return r;
    }

    let path = normalize_path(uri.path());
    let method_not_allowed = || err(ErrorCode::Validation, "method not allowed").into_response();
    let with_405 = |r: Response| {
        let mut r = r;
        *r.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        r
    };

    if path == s.status_path {
        if method != Method::GET {
            return with_405(method_not_allowed());
        }
        return status(&core, &headers, ip, s.public_status);
    }
    if path == s.allowed_path {
        if method != Method::GET {
            return with_405(method_not_allowed());
        }
        return allowed(&core, &headers, ip, &s.flow_prefix);
    }
    if let Some(rest) = path.strip_prefix(&s.flow_prefix) {
        if method != Method::POST {
            return with_405(method_not_allowed());
        }
        return run_flow(
            &core,
            &headers,
            ip,
            rest.trim_matches('/'),
            s.hide_forbidden_flows,
        )
        .await;
    }
    if let Some(rest) = path.strip_prefix(RUNS_PREFIX) {
        let parts: Vec<&str> = rest.trim_matches('/').split('/').collect();
        return match (parts.as_slice(), &method) {
            ([id], &Method::GET) => get_run(&core, &headers, ip, id),
            ([id, "cancel"], &Method::POST) => cancel_run(&core, &headers, ip, id),
            ([_], _) | ([_, "cancel"], _) => with_405(method_not_allowed()),
            _ => err(ErrorCode::NotFound, "not found"),
        };
    }
    err(ErrorCode::NotFound, "not found")
}

fn status(core: &Core, headers: &HeaderMap, ip: std::net::IpAddr, public: bool) -> Response {
    match authenticate(core, headers, ip, false) {
        Err(r) => r,
        Ok(Some(_)) => axum::Json(json!({
            "status": "ok",
            "version": env!("CARGO_PKG_VERSION"),
            "uptime_sec": core.uptime().as_secs(),
            "elevated": core.platform.is_elevated(),
        }))
        .into_response(),
        Ok(None) if public => axum::Json(json!({ "status": "ok" })).into_response(),
        Ok(None) => err(ErrorCode::Unauthorized, "API key required"),
    }
}

fn allowed(core: &Core, headers: &HeaderMap, ip: std::net::IpAddr, flow_prefix: &str) -> Response {
    let key = match authenticate(core, headers, ip, true) {
        Ok(Some(k)) => k,
        Ok(None) => unreachable!(),
        Err(r) => return r,
    };
    let result = (|| -> Result<Value, AppError> {
        let ids = core.db.flow_ids_for_key(key.id)?;
        let flows: Vec<Value> = core
            .db
            .list_flows()?
            .into_iter()
            .filter(|f| f.input.enabled && ids.contains(&f.id))
            .map(|f| {
                json!({
                    "path": f.input.path,
                    "url": format!("{flow_prefix}{}/", f.input.path),
                    "method": "POST",
                    "name": f.input.name,
                    "description": f.input.description,
                    "response_mode": f.input.response_mode,
                    "start_delay_sec": f.input.start_delay_sec,
                    "cancelable": f.input.cancelable,
                    "client_meta": f.input.client_meta,
                })
            })
            .collect();
        Ok(json!({ "key": key.name, "flows": flows }))
    })();
    match result {
        Ok(v) => axum::Json(v).into_response(),
        Err(e) => error_response(&e),
    }
}

/// Run representation for HTTP clients: output is included only for steps with `capture_output`.
pub fn public_run(run: &Run) -> Value {
    let steps: Vec<Value> = run
        .steps
        .iter()
        .map(|s| {
            let mut v = json!({
                "index": s.index,
                "type": s.action_type,
                "status": s.status,
                "started_at": s.started_at,
                "finished_at": s.finished_at,
            });
            if let Some(c) = s.exit_code {
                v["exit_code"] = json!(c);
            }
            if let Some(e) = &s.error {
                v["error"] = json!(e);
            }
            if s.capture_output {
                v["stdout"] = json!(s.stdout);
                v["stderr"] = json!(s.stderr);
            }
            v
        })
        .collect();
    json!({
        "run_id": run.id,
        "flow": run.flow_path,
        "status": run.status,
        "created_at": run.created_at,
        "started_at": run.started_at,
        "finished_at": run.finished_at,
        "error": run.error,
        "steps": steps,
    })
}

fn run_http_status(status: RunStatus) -> StatusCode {
    match status {
        RunStatus::Success => StatusCode::OK,
        RunStatus::Failed => StatusCode::INTERNAL_SERVER_ERROR,
        RunStatus::Timeout => StatusCode::GATEWAY_TIMEOUT,
        RunStatus::Canceled => StatusCode::CONFLICT,
        _ => StatusCode::ACCEPTED,
    }
}

async fn run_flow(
    core: &Arc<Core>,
    headers: &HeaderMap,
    ip: std::net::IpAddr,
    path: &str,
    hide_forbidden: bool,
) -> Response {
    let key = match authenticate(core, headers, ip, true) {
        Ok(Some(k)) => k,
        Ok(None) => unreachable!(),
        Err(r) => return r,
    };
    let flow = match core.db.find_flow_by_path(path) {
        Ok(Some(f)) if f.input.enabled => f,
        Ok(_) => return err(ErrorCode::NotFound, "flow not found"),
        Err(e) => return error_response(&e),
    };
    match core.db.flow_ids_for_key(key.id) {
        Ok(ids) if ids.contains(&flow.id) => {}
        Ok(_) if hide_forbidden => return err(ErrorCode::NotFound, "flow not found"),
        Ok(_) => {
            return err(
                ErrorCode::Forbidden,
                "this API key has no access to the flow",
            );
        }
        Err(e) => return error_response(&e),
    }
    let wait = Duration::from_secs(flow.input.start_delay_sec + flow.input.timeout_sec + 5);
    let mode = flow.input.response_mode;
    let handle = match core.runner.start(RunRequest {
        flow,
        source: RunSource::Http,
        api_key: Some((key.id, key.name.clone())),
        client_ip: Some(ip.to_string()),
    }) {
        Ok(h) => h,
        Err(e) => return error_response(&e),
    };
    let run_id = handle.run.id.clone();
    let accepted = || {
        (
            StatusCode::ACCEPTED,
            axum::Json(json!({ "run_id": run_id, "status": "pending", "status_url": format!("{RUNS_PREFIX}{run_id}") })),
        )
            .into_response()
    };
    match mode {
        crate::model::ResponseMode::Async => accepted(),
        crate::model::ResponseMode::Sync => match handle.wait(wait).await {
            Some(run) => {
                (run_http_status(run.status), axum::Json(public_run(&run))).into_response()
            }
            None => accepted(),
        },
    }
}

#[allow(clippy::result_large_err)]
fn owned_run(
    core: &Core,
    headers: &HeaderMap,
    ip: std::net::IpAddr,
    id: &str,
) -> Result<Run, Response> {
    let key = match authenticate(core, headers, ip, true) {
        Ok(Some(k)) => k,
        Ok(None) => unreachable!(),
        Err(r) => return Err(r),
    };
    match core.db.get_run(id) {
        Ok(run) if run.api_key_id == Some(key.id) => Ok(run),
        Ok(_) | Err(_) => Err(err(ErrorCode::NotFound, "run not found")),
    }
}

fn get_run(core: &Core, headers: &HeaderMap, ip: std::net::IpAddr, id: &str) -> Response {
    match owned_run(core, headers, ip, id) {
        Ok(run) => axum::Json(public_run(&run)).into_response(),
        Err(r) => r,
    }
}

fn cancel_run(core: &Core, headers: &HeaderMap, ip: std::net::IpAddr, id: &str) -> Response {
    match owned_run(core, headers, ip, id) {
        Ok(run) if run.status.is_finished() => err(ErrorCode::Conflict, "run already finished"),
        Ok(run) => match core.runner.cancel(&run.id, false) {
            Ok(()) => {
                axum::Json(json!({ "run_id": run.id, "status": "canceling" })).into_response()
            }
            Err(e) => error_response(&e),
        },
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests;
