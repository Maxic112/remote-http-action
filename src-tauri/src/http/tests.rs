//! HTTP API integration tests over a real socket.

use std::sync::Arc;

use reqwest::StatusCode;
use serde_json::{Value, json};

use crate::core::tests::{core_with, sample_flow};
use crate::core::{Caller, Core, Request};
use crate::model::{ActionDef, OnError, ResponseMode};
use crate::platform::mock::{MockPlatform, MockResponse};

fn client() -> reqwest::Client {
    crate::tls::ensure_crypto_provider();
    reqwest::Client::builder()
        .tls_danger_accept_invalid_certs(true)
        .build()
        .unwrap()
}

struct Fixture {
    core: Arc<Core>,
    base: String,
    client: reqwest::Client,
    key: String,
}

impl Fixture {
    async fn new(platform: MockPlatform) -> Self {
        let core = core_with(platform);
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        core.http.serve(core.clone(), listener, None).await;
        let flow = core
            .handle(
                Caller::Ui,
                Request::FlowCreate {
                    flow: sample_flow("sleep"),
                },
            )
            .await
            .unwrap();
        let group = core
            .handle(
                Caller::Ui,
                Request::GroupCreate {
                    name: "g".into(),
                    description: String::new(),
                    key_ids: vec![],
                    flow_ids: vec![flow["id"].as_i64().unwrap()],
                },
            )
            .await
            .unwrap();
        let issued = core
            .handle(
                Caller::Ui,
                Request::KeyCreate {
                    name: "phone".into(),
                    group_ids: vec![group["id"].as_i64().unwrap()],
                },
            )
            .await
            .unwrap();
        Self {
            core,
            base: format!("http://{addr}"),
            client: client(),
            key: issued["secret"].as_str().unwrap().to_string(),
        }
    }

    async fn post(&self, path: &str, key: Option<&str>) -> (StatusCode, Value) {
        let mut req = self.client.post(format!("{}{path}", self.base));
        if let Some(k) = key {
            req = req.bearer_auth(k);
        }
        let resp = req.send().await.unwrap();
        let status = resp.status();
        (status, resp.json().await.unwrap_or(Value::Null))
    }

    async fn get(&self, path: &str, key: Option<&str>) -> (StatusCode, Value) {
        let mut req = self.client.get(format!("{}{path}", self.base));
        if let Some(k) = key {
            req = req.header("X-API-Key", k);
        }
        let resp = req.send().await.unwrap();
        let status = resp.status();
        (status, resp.json().await.unwrap_or(Value::Null))
    }

    async fn add_flow(&self, path: &str, f: impl FnOnce(&mut crate::model::FlowInput)) -> i64 {
        let mut input = sample_flow(path);
        f(&mut input);
        let flow = self
            .core
            .handle(Caller::Ui, Request::FlowCreate { flow: input })
            .await
            .unwrap();
        let id = flow["id"].as_i64().unwrap();
        let group = self.core.db.list_groups().unwrap().remove(0);
        let mut ids = group.flow_ids.clone();
        ids.push(id);
        self.core
            .db
            .update_group(group.id, None, None, None, Some(&ids))
            .unwrap();
        id
    }
}

fn cmd_action(capture: bool) -> ActionDef {
    ActionDef {
        action_type: "cli_command".into(),
        params: json!({"shell": "cmd", "command": "echo hi"}),
        on_error: OnError::Stop,
        timeout_sec: None,
        run_as_admin: false,
        capture_output: capture,
    }
}

#[tokio::test]
async fn requests_without_valid_key_are_rejected() {
    let fx = Fixture::new(MockPlatform::default()).await;
    let (s, body) = fx.post("/flow/sleep/", None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "UNAUTHORIZED");
    let (s, _) = fx.post("/flow/sleep/", Some("rha_wrong")).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
    // Key in the query string is not accepted.
    let (s, _) = fx.post(&format!("/flow/sleep/?key={}", fx.key), None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn sync_flow_runs_and_returns_result() {
    let fx = Fixture::new(MockPlatform::default()).await;
    let (s, body) = fx.post("/flow/sleep/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "success");
    assert_eq!(body["flow"], "sleep");
    // Trailing slash is optional; X-API-Key works too.
    let resp = fx
        .client
        .post(format!("{}/flow/sleep", fx.base))
        .header("X-API-Key", &fx.key)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let runs = fx.core.db.list_runs(None, 10).unwrap();
    assert_eq!(runs[0].api_key_name.as_deref(), Some("phone"));
    assert_eq!(runs[0].client_ip.as_deref(), Some("127.0.0.1"));
}

#[tokio::test]
async fn output_is_returned_only_with_capture_output() {
    let fx = Fixture::new(MockPlatform::with_responses(vec![
        MockResponse::ok("secret-out"),
        MockResponse::ok("shown"),
    ]))
    .await;
    fx.add_flow("hidden", |f| f.actions = vec![cmd_action(false)])
        .await;
    fx.add_flow("shown", |f| f.actions = vec![cmd_action(true)])
        .await;
    let (_, body) = fx.post("/flow/hidden/", Some(&fx.key)).await;
    assert!(body["steps"][0].get("stdout").is_none());
    assert!(!body.to_string().contains("secret-out"));
    let (_, body) = fx.post("/flow/shown/", Some(&fx.key)).await;
    assert_eq!(body["steps"][0]["stdout"], "shown");
}

#[tokio::test]
async fn failed_flow_returns_500_with_details() {
    let fx = Fixture::new(MockPlatform::with_responses(vec![MockResponse::exit(5)])).await;
    fx.add_flow("bad", |f| f.actions = vec![cmd_action(false)])
        .await;
    let (s, body) = fx.post("/flow/bad/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["status"], "failed");
    assert_eq!(body["steps"][0]["exit_code"], 5);
}

#[tokio::test]
async fn async_flow_returns_202_and_status_endpoint_works() {
    let fx = Fixture::new(MockPlatform::default()).await;
    fx.add_flow("bg", |f| f.response_mode = ResponseMode::Async)
        .await;
    let (s, body) = fx.post("/flow/bg/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::ACCEPTED);
    let url = body["status_url"].as_str().unwrap().to_string();
    let mut status = Value::Null;
    for _ in 0..50 {
        let (_, b) = fx.get(&url, Some(&fx.key)).await;
        status = b["status"].clone();
        if status == "success" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(status, "success");

    // Another key cannot see the run.
    let other = fx
        .core
        .handle(
            Caller::Ui,
            Request::KeyCreate {
                name: "other".into(),
                group_ids: vec![],
            },
        )
        .await
        .unwrap();
    let (s, _) = fx.get(&url, other["secret"].as_str()).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn delayed_run_can_be_canceled_over_http() {
    let fx = Fixture::new(MockPlatform::default()).await;
    fx.add_flow("later", |f| {
        f.response_mode = ResponseMode::Async;
        f.start_delay_sec = 30;
    })
    .await;
    let (_, body) = fx.post("/flow/later/", Some(&fx.key)).await;
    let id = body["run_id"].as_str().unwrap();
    let (s, _) = fx.post(&format!("/runs/{id}/cancel"), Some(&fx.key)).await;
    assert_eq!(s, StatusCode::OK);
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let (_, b) = fx.get(&format!("/runs/{id}"), Some(&fx.key)).await;
    assert_eq!(b["status"], "canceled");
}

#[tokio::test]
async fn access_is_limited_to_group_flows() {
    let fx = Fixture::new(MockPlatform::default()).await;
    fx.core
        .handle(
            Caller::Ui,
            Request::FlowCreate {
                flow: sample_flow("private"),
            },
        )
        .await
        .unwrap();
    let (s, _) = fx.post("/flow/private/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let (s, _) = fx.post("/flow/missing/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);

    let mut settings = fx.core.settings();
    settings.server.hide_forbidden_flows = true;
    fx.core.apply_settings(settings).await.unwrap();
    let (s, _) = fx.post("/flow/private/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn disabled_key_or_flow_is_rejected() {
    let fx = Fixture::new(MockPlatform::default()).await;
    fx.core
        .handle(
            Caller::Ui,
            Request::FlowSetEnabled {
                flow: "sleep".into(),
                enabled: false,
            },
        )
        .await
        .unwrap();
    let (s, _) = fx.post("/flow/sleep/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let key_id = fx.core.db.list_keys().unwrap()[0].id;
    fx.core
        .handle(
            Caller::Ui,
            Request::KeyUpdate {
                id: key_id,
                name: None,
                enabled: Some(false),
                group_ids: None,
            },
        )
        .await
        .unwrap();
    let (s, _) = fx.get("/get-allowed/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_allowed_lists_only_accessible_enabled_flows() {
    let fx = Fixture::new(MockPlatform::default()).await;
    fx.core
        .handle(
            Caller::Ui,
            Request::FlowCreate {
                flow: sample_flow("private"),
            },
        )
        .await
        .unwrap();
    fx.add_flow("off", |f| f.enabled = false).await;
    let (s, body) = fx.get("/get-allowed/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body["key"], "phone");
    let paths: Vec<&str> = body["flows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["sleep"]);
    assert_eq!(body["flows"][0]["url"], "/flow/sleep/");
}

#[tokio::test]
async fn status_endpoint_public_and_detailed() {
    let fx = Fixture::new(MockPlatform::default()).await;
    let (s, body) = fx.get("/status/", None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(body, json!({"status": "ok"}));
    let (_, body) = fx.get("/status", Some(&fx.key)).await;
    assert_eq!(body["status"], "ok");
    assert!(body["version"].is_string());
    assert_eq!(body["elevated"], false);

    let mut settings = fx.core.settings();
    settings.server.public_status = false;
    fx.core.apply_settings(settings).await.unwrap();
    let (s, _) = fx.get("/status/", None).await;
    assert_eq!(s, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn custom_prefixes_apply_without_restart() {
    let fx = Fixture::new(MockPlatform::default()).await;
    let mut settings = fx.core.settings();
    settings.server.flow_prefix = "/do/".into();
    settings.server.status_path = "/health/".into();
    fx.core.apply_settings(settings).await.unwrap();
    let (s, _) = fx.post("/do/sleep/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::OK);
    let (s, _) = fx.post("/flow/sleep/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::NOT_FOUND);
    let (s, _) = fx.get("/health/", None).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn wrong_method_returns_405() {
    let fx = Fixture::new(MockPlatform::default()).await;
    let (s, _) = fx.get("/flow/sleep/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
    let (s, _) = fx.post("/status/", None).await;
    assert_eq!(s, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn ip_and_host_filters() {
    let fx = Fixture::new(MockPlatform::default()).await;
    let mut settings = fx.core.settings();
    settings.server.allowed_ips = vec!["10.0.0.0/8".into()];
    fx.core.apply_settings(settings.clone()).await.unwrap();
    let (s, body) = fx.get("/status/", None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    assert_eq!(body["error"]["message"], "client address is not allowed");

    // A trusted proxy may forward an allowed client address.
    settings.server.trusted_proxies = vec!["127.0.0.1".into()];
    fx.core.apply_settings(settings.clone()).await.unwrap();
    let resp = fx
        .client
        .get(format!("{}/status/", fx.base))
        .header("X-Forwarded-For", "10.1.2.3")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    settings.server.allowed_ips.clear();
    settings.server.trusted_proxies.clear();
    settings.server.allowed_hosts = vec!["pc.local".into()];
    fx.core.apply_settings(settings).await.unwrap();
    let (s, _) = fx.get("/status/", None).await;
    assert_eq!(s, StatusCode::FORBIDDEN);
    let resp = fx
        .client
        .get(format!("{}/status/", fx.base))
        .header("Host", "pc.local:8765")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn repeated_bad_keys_ban_the_client() {
    let fx = Fixture::new(MockPlatform::default()).await;
    let mut settings = fx.core.settings();
    settings.server.brute_force.max_failures = 3;
    fx.core.apply_settings(settings).await.unwrap();
    for _ in 0..3 {
        let (s, _) = fx.post("/flow/sleep/", Some("rha_bad")).await;
        assert_eq!(s, StatusCode::UNAUTHORIZED);
    }
    let resp = fx
        .client
        .post(format!("{}/flow/sleep/", fx.base))
        .bearer_auth(&fx.key)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(resp.headers().contains_key("retry-after"));
    let bans = fx.core.handle(Caller::Ui, Request::BansList).await.unwrap();
    assert_eq!(bans[0]["ip"], "127.0.0.1");
    fx.core
        .handle(
            Caller::Ui,
            Request::BanRemove {
                ip: "127.0.0.1".into(),
            },
        )
        .await
        .unwrap();
    let (s, _) = fx.post("/flow/sleep/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn concurrency_conflict_and_cooldown_status_codes() {
    let fx = Fixture::new(MockPlatform::default()).await;
    fx.add_flow("cool", |f| {
        f.cooldown_sec = 60;
        f.concurrency = crate::model::Concurrency::Allow;
    })
    .await;
    let (s, _) = fx.post("/flow/cool/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::OK);
    let (s, body) = fx.post("/flow/cool/", Some(&fx.key)).await;
    assert_eq!(s, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "TOO_MANY_REQUESTS");
}

#[tokio::test]
async fn server_can_be_stopped() {
    let fx = Fixture::new(MockPlatform::default()).await;
    fx.core.http.stop(&fx.core).await;
    assert!(
        fx.client
            .get(format!("{}/status/", fx.base))
            .send()
            .await
            .is_err()
    );
}

#[tokio::test]
async fn requests_are_logged_with_key_name_only() {
    let fx = Fixture::new(MockPlatform::default()).await;
    fx.post("/flow/sleep/", Some(&fx.key)).await;
    fx.post("/flow/sleep/", Some("rha_wrong")).await;
    let entries = fx
        .core
        .logs
        .read(crate::logs::LogCategory::Http, 10, None)
        .unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["status"], 401);
    assert_eq!(entries[0]["key"], Value::Null);
    assert_eq!(entries[1]["status"], 200);
    assert_eq!(entries[1]["key"], "phone");
    assert_eq!(entries[1]["method"], "POST");
    assert_eq!(entries[1]["path"], "/flow/sleep/");
    assert_eq!(entries[1]["ip"], "127.0.0.1");
    let text = serde_json::to_string(&entries).unwrap();
    assert!(!text.contains(&fx.key));
    assert!(!text.contains("rha_wrong"));
}

#[tokio::test]
async fn https_with_self_signed_certificate() {
    let core = core_with(MockPlatform::default());
    let mut settings = core.settings();
    settings.server.enabled = false;
    settings.server.tls.enabled = true;
    core.apply_settings(settings.clone()).await.unwrap();

    let config = crate::tls::load(&settings.server.tls, &core.paths.data_dir)
        .await
        .unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let status = core.http.serve(core.clone(), listener, Some(config)).await;
    assert!(status.tls);

    let resp = client()
        .get(format!("https://localhost:{port}/status/"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    // Plain HTTP to the HTTPS port fails.
    let plain = client()
        .get(format!("http://127.0.0.1:{port}/status/"))
        .send()
        .await;
    assert!(!plain.map(|r| r.status().is_success()).unwrap_or(false));

    let info = core.handle(Caller::Ui, Request::TlsInfo).await.unwrap();
    assert_eq!(info["fingerprint"].as_str().unwrap().len(), 95);
    core.http.stop(&core).await;
}

#[tokio::test]
async fn broken_custom_certificate_reports_server_error() {
    let core = core_with(MockPlatform::default());
    let mut settings = core.settings();
    settings.server.enabled = false;
    core.apply_settings(settings.clone()).await.unwrap();
    settings.server.enabled = true;
    settings.server.port = 1;
    settings.server.bind_address = "127.0.0.1".into();
    settings.server.tls = crate::settings::TlsSettings {
        enabled: true,
        mode: crate::settings::TlsMode::Custom,
        cert_path: "/no/cert.pem".into(),
        key_path: "/no/key.pem".into(),
    };
    core.apply_settings(settings).await.unwrap();
    let status = core.http.status();
    assert_eq!(status.state, super::ServerState::Error);
    assert!(
        status
            .error
            .unwrap()
            .contains("cannot load TLS certificate")
    );
    assert!(status.tls);
}
