//! Local IPC between the CLI and the running instance.
//!
//! Protocol: the client writes one JSON line with an [`IpcRequest`], the server answers with one
//! JSON line containing an [`IpcResponse`]. On Windows the transport is a named pipe that only the
//! current user and administrators can open; elsewhere a Unix socket inside the data directory.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};

use crate::core::{Caller, Core, Request};
use crate::error::{AppError, AppResult, ErrorCode};

const MAX_MESSAGE: u64 = 4 * 1024 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcRequest {
    pub request: Request,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpcResponse {
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub error: Option<AppError>,
}

impl IpcResponse {
    fn from_result(r: AppResult<Value>) -> Self {
        match r {
            Ok(data) => Self { data, error: None },
            Err(e) => Self {
                data: Value::Null,
                error: Some(e),
            },
        }
    }
}

/// Executes a CLI request, honouring the "CLI access" setting.
pub async fn dispatch(core: &Arc<Core>, req: IpcRequest) -> AppResult<Value> {
    if !core.settings().cli.enabled {
        return Err(AppError::new(
            ErrorCode::CliDisabled,
            "CLI access is disabled. Enable it in Settings → CLI in the application window.",
        ));
    }
    core.handle(Caller::Cli, req.request).await
}

async fn serve_connection<S: AsyncRead + AsyncWrite + Unpin>(core: Arc<Core>, stream: S) {
    let (read, mut write) = tokio::io::split(stream);
    let mut reader = BufReader::new(read.take(MAX_MESSAGE));
    let mut line = String::new();
    let response = match reader.read_line(&mut line).await {
        Ok(0) => return,
        Ok(_) => match serde_json::from_str::<IpcRequest>(&line) {
            Ok(req) => IpcResponse::from_result(dispatch(&core, req).await),
            Err(e) => {
                IpcResponse::from_result(Err(AppError::validation(format!("bad request: {e}"))))
            }
        },
        Err(e) => IpcResponse::from_result(Err(AppError::internal(e.to_string()))),
    };
    let mut out = serde_json::to_vec(&response).unwrap_or_default();
    out.push(b'\n');
    let _ = write.write_all(&out).await;
    let _ = write.flush().await;
}

async fn exchange<S: AsyncRead + AsyncWrite + Unpin>(
    stream: S,
    req: &IpcRequest,
) -> AppResult<IpcResponse> {
    let (read, mut write) = tokio::io::split(stream);
    let mut msg = serde_json::to_vec(req).map_err(|e| AppError::internal(e.to_string()))?;
    msg.push(b'\n');
    write.write_all(&msg).await?;
    write.flush().await?;
    let mut reader = BufReader::new(read.take(MAX_MESSAGE));
    let mut line = String::new();
    reader.read_line(&mut line).await?;
    serde_json::from_str(&line)
        .map_err(|e| AppError::internal(format!("bad response from the application: {e}")))
}

fn not_running() -> AppError {
    AppError::new(
        ErrorCode::NotRunning,
        "remote-http-action is not running. Start the application first.",
    )
}

#[cfg(windows)]
mod transport {
    use std::ffi::c_void;
    use std::time::Duration;

    use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
    use windows::Win32::Foundation::{ERROR_PIPE_BUSY, HLOCAL, LocalFree};
    use windows::Win32::Security::Authorization::{
        ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
    };
    use windows::Win32::Security::{PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES};
    use windows::core::HSTRING;

    use super::*;

    /// Security descriptor: full access for SYSTEM, Administrators and the current user;
    /// medium integrity label so a non-elevated terminal can reach an elevated instance.
    fn create_pipe(endpoint: &str, first: bool) -> std::io::Result<NamedPipeServer> {
        let sid = crate::platform::win::current_user_sid().unwrap_or_else(|| "OW".into());
        let dacl = format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;GA;;;{sid})");
        let mut opts = ServerOptions::new();
        opts.first_pipe_instance(first).reject_remote_clients(true);
        for sddl in [format!("{dacl}S:(ML;;NW;;;ME)"), dacl] {
            let mut sd = PSECURITY_DESCRIPTOR::default();
            let ok = unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    &HSTRING::from(sddl),
                    SDDL_REVISION_1,
                    &mut sd,
                    None,
                )
            }
            .is_ok();
            if !ok {
                continue;
            }
            let mut attrs = SECURITY_ATTRIBUTES {
                nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
                lpSecurityDescriptor: sd.0,
                bInheritHandle: false.into(),
            };
            let result = unsafe {
                opts.create_with_security_attributes_raw(
                    endpoint,
                    &mut attrs as *mut _ as *mut c_void,
                )
            };
            unsafe {
                let _ = LocalFree(Some(HLOCAL(sd.0)));
            }
            if let Ok(server) = result {
                return Ok(server);
            }
        }
        Err(std::io::Error::other("cannot create the CLI named pipe"))
    }

    pub async fn serve(core: Arc<Core>, endpoint: String) -> std::io::Result<()> {
        let mut server = create_pipe(&endpoint, true)?;
        loop {
            server.connect().await?;
            let connected = server;
            server = create_pipe(&endpoint, false)?;
            tokio::spawn(serve_connection(core.clone(), connected));
        }
    }

    pub async fn send(endpoint: &str, req: &IpcRequest) -> AppResult<IpcResponse> {
        for _ in 0..50 {
            match ClientOptions::new().open(endpoint) {
                Ok(client) => return exchange(client, req).await,
                Err(e) if e.raw_os_error() == Some(ERROR_PIPE_BUSY.0 as i32) => {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(not_running()),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(AppError::new(
                        ErrorCode::Forbidden,
                        "access to the running instance was denied",
                    ));
                }
                Err(e) => return Err(e.into()),
            }
        }
        Err(AppError::internal("the application is busy, try again"))
    }
}

#[cfg(unix)]
mod transport {
    use tokio::net::{UnixListener, UnixStream};

    use super::*;

    pub async fn serve(core: Arc<Core>, endpoint: String) -> std::io::Result<()> {
        let _ = std::fs::remove_file(&endpoint);
        let listener = UnixListener::bind(&endpoint)?;
        loop {
            let (stream, _) = listener.accept().await?;
            tokio::spawn(serve_connection(core.clone(), stream));
        }
    }

    pub async fn send(endpoint: &str, req: &IpcRequest) -> AppResult<IpcResponse> {
        match UnixStream::connect(endpoint).await {
            Ok(stream) => exchange(stream, req).await,
            Err(_) => Err(not_running()),
        }
    }
}

pub use transport::{send, serve};

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::core::tests::core_with;
    use crate::platform::mock::MockPlatform;

    fn endpoint(tag: &str) -> (String, Option<tempfile::TempDir>) {
        if cfg!(windows) {
            (
                format!(r"\\.\pipe\rha-test-{tag}-{}", uuid::Uuid::new_v4()),
                None,
            )
        } else {
            let dir = tempfile::tempdir().unwrap();
            (dir.path().join("s.sock").display().to_string(), Some(dir))
        }
    }

    async fn set_cli(core: &Arc<Core>, enabled: bool) {
        let mut s = core.settings();
        s.server.enabled = false;
        s.cli.enabled = enabled;
        core.apply_settings(s).await.unwrap();
    }

    #[tokio::test]
    async fn round_trip_through_transport() {
        let core = core_with(MockPlatform::default());
        set_cli(&core, true).await;
        let (ep, _dir) = endpoint("rt");
        tokio::spawn(serve(core.clone(), ep.clone()));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let resp = send(
            &ep,
            &IpcRequest {
                request: Request::KeyCreate {
                    name: "cli".into(),
                    group_ids: vec![],
                },
            },
        )
        .await
        .unwrap();
        assert!(resp.error.is_none());
        assert!(resp.data["secret"].as_str().unwrap().starts_with("rha_"));
        let resp = send(
            &ep,
            &IpcRequest {
                request: Request::KeysList,
            },
        )
        .await
        .unwrap();
        assert_eq!(resp.data.as_array().unwrap().len(), 1);
        let resp = send(
            &ep,
            &IpcRequest {
                request: Request::KeyDelete { id: 999 },
            },
        )
        .await
        .unwrap();
        assert_eq!(resp.error.unwrap().code, ErrorCode::NotFound);
    }

    #[tokio::test]
    async fn disabled_cli_rejects_everything() {
        let core = core_with(MockPlatform::default());
        set_cli(&core, false).await;
        let (ep, _dir) = endpoint("off");
        tokio::spawn(serve(core.clone(), ep.clone()));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let resp = send(
            &ep,
            &IpcRequest {
                request: Request::KeysList,
            },
        )
        .await
        .unwrap();
        assert_eq!(resp.error.unwrap().code, ErrorCode::CliDisabled);
        let resp = send(
            &ep,
            &IpcRequest {
                request: Request::SettingsSet {
                    path: "cli.enabled".into(),
                    value: json!(true),
                },
            },
        )
        .await
        .unwrap();
        assert_eq!(resp.error.unwrap().code, ErrorCode::CliDisabled);
        assert!(!core.settings().cli.enabled);
    }

    #[tokio::test]
    async fn missing_instance_is_reported() {
        let (ep, _dir) = endpoint("none");
        let err = send(
            &ep,
            &IpcRequest {
                request: Request::KeysList,
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotRunning);
    }
}
