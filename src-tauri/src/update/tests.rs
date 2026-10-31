use axum::Router;
use axum::routing::get;
use serde_json::json;

use super::*;
use crate::core::tests::{core_with, core_with_paths};
use crate::core::{Caller, Request};
use crate::platform::mock::MockPlatform;

fn github_release(base: &str, version: &str, assets: &[(&str, u64, Option<String>)]) -> Value {
    json!({
        "tag_name": format!("v{version}"),
        "name": format!("Remote HTTP Action {version}"),
        "body": "- New things\n- Fixes",
        "published_at": "2026-11-01T10:00:00Z",
        "html_url": format!("{base}/releases/v{version}"),
        "draft": false,
        "prerelease": false,
        "assets": assets.iter().map(|(name, size, digest)| json!({
            "name": name,
            "size": size,
            "digest": digest,
            "browser_download_url": format!("{base}/files/{name}"),
        })).collect::<Vec<_>>(),
    })
}

#[test]
fn parses_github_release() {
    let r = parse_release(&github_release(
        "http://x",
        "1.2.3",
        &[("a_x64_portable.zip", 10, None)],
    ))
    .unwrap();
    assert_eq!(r.version, "1.2.3");
    assert_eq!(r.tag, "v1.2.3");
    assert_eq!(r.name, "Remote HTTP Action 1.2.3");
    assert_eq!(r.notes, "- New things\n- Fixes");
    assert_eq!(r.assets[0].url, "http://x/files/a_x64_portable.zip");
    assert!(parse_release(&json!({"tag_name": "latest"})).is_err());
    assert!(parse_release(&json!({})).is_err());
}

#[test]
fn default_feed_points_to_the_project_releases() {
    assert_eq!(
        default_feed_url(),
        "https://api.github.com/repos/Maxic112/remote-http-action/releases/latest"
    );
}

#[test]
fn compares_versions() {
    assert!(is_newer("1.0.1", "1.0.0"));
    assert!(is_newer("v1.10.0", "1.9.9"));
    assert!(!is_newer("1.0.0", "1.0.0"));
    assert!(!is_newer("0.9.0", "1.0.0"));
    assert!(!is_newer("1.1.0-beta.1", "1.1.0"));
    assert!(!is_newer("garbage", "1.0.0"));
}

fn release_with(names: &[&str]) -> ReleaseInfo {
    parse_release(&github_release(
        "http://x",
        "2.0.0",
        &names.iter().map(|n| (*n, 1, None)).collect::<Vec<_>>(),
    ))
    .unwrap()
}

#[test]
fn picks_the_file_for_the_installation() {
    let r = release_with(&[
        "remote-http-action_2.0.0_x64_en-US.msi",
        "remote-http-action_2.0.0_x64_ru-RU.msi",
        "remote-http-action_2.0.0_x64_portable.zip",
        "SHA256SUMS.txt",
    ]);
    assert!(
        pick_asset(&r, InstallKind::Msi, true)
            .unwrap()
            .name
            .ends_with("ru-RU.msi")
    );
    assert!(
        pick_asset(&r, InstallKind::Msi, false)
            .unwrap()
            .name
            .ends_with("en-US.msi")
    );
    assert!(
        pick_asset(&r, InstallKind::Portable, true)
            .unwrap()
            .name
            .ends_with("portable.zip")
    );
    assert!(pick_asset(&r, InstallKind::Development, true).is_none());
    let only_en = release_with(&["remote-http-action_2.0.0_x64_en-US.msi"]);
    assert!(
        pick_asset(&only_en, InstallKind::Msi, true)
            .unwrap()
            .name
            .ends_with("en-US.msi")
    );
    assert!(pick_asset(&only_en, InstallKind::Portable, true).is_none());
}

#[test]
fn reads_checksums_from_digest_or_sums_file() {
    let mut asset = ReleaseAsset {
        name: "a.zip".into(),
        url: String::new(),
        size: 0,
        digest: Some("sha256:ABCD".into()),
    };
    assert_eq!(expected_sha256(&asset, None).unwrap(), "abcd");
    asset.digest = None;
    let h = "e".repeat(64);
    let sums = format!("{} other.msi\n{h} *a.zip\n", "f".repeat(64));
    assert_eq!(expected_sha256(&asset, Some(&sums)).unwrap(), h);
    assert!(expected_sha256(&asset, Some("nothing here")).is_none());
    assert!(expected_sha256(&asset, None).is_none());
}

#[test]
fn helper_scripts_wait_apply_restart_and_clean_up() {
    let work = Path::new(r"C:\Temp\rha update");
    let exe = Path::new(r"C:\Program Files\Remote HTTP Action\remote-http-action.exe");
    let log = Path::new(r"C:\data\logs\update.log");
    let msi = helper_script(
        InstallKind::Msi,
        4242,
        Path::new(r"C:\Temp\rha update\x.msi"),
        exe,
        work,
        log,
    );
    assert!(msi.contains("Wait-Process -Id 4242"));
    assert!(msi.contains("msiexec.exe"));
    assert!(msi.contains(r"'C:\Temp\rha update\x.msi'"));
    assert!(msi.contains(
        r"Start-Process -FilePath 'C:\Program Files\Remote HTTP Action\remote-http-action.exe'"
    ));
    assert!(msi.contains(r"Remove-Item -LiteralPath 'C:\Temp\rha update' -Recurse"));
    // The application restarts only after the installer has finished.
    assert!(
        msi.find("msiexec").unwrap() < msi.find("Start-Process -FilePath 'C:\\Program").unwrap()
    );

    let portable = helper_script(
        InstallKind::Portable,
        7,
        Path::new(r"C:\Temp\rha update\staging"),
        Path::new(r"D:\Tools\O'Neil\remote-http-action.exe"),
        work,
        log,
    );
    assert!(portable.contains("'remote-http-action.exe', 'remote-http-action-cli.exe'"));
    assert!(portable.contains(r"Join-Path 'D:\Tools\O''Neil' $file"));
    assert!(!portable.contains("msiexec"));
}

fn portable_zip(dir: &Path, with_exe: bool) -> PathBuf {
    let path = dir.join("p.zip");
    let mut w = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    if with_exe {
        w.start_file("Remote HTTP Action/remote-http-action.exe", opts)
            .unwrap();
        w.write_all(b"new main").unwrap();
        w.start_file("Remote HTTP Action/remote-http-action-cli.exe", opts)
            .unwrap();
        w.write_all(b"new cli").unwrap();
    }
    w.start_file("Remote HTTP Action/portable.flag", opts)
        .unwrap();
    w.write_all(b"flag").unwrap();
    w.start_file("../evil.txt", opts).unwrap();
    w.write_all(b"x").unwrap();
    w.finish().unwrap();
    path
}

#[test]
fn extracts_the_portable_archive_safely() {
    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("staging");
    let exe_dir = extract_portable(&portable_zip(dir.path(), true), &dest).unwrap();
    assert_eq!(exe_dir, dest.join("Remote HTTP Action"));
    assert_eq!(
        std::fs::read(exe_dir.join("remote-http-action.exe")).unwrap(),
        b"new main"
    );
    assert!(!dir.path().join("evil.txt").exists());

    let dir2 = tempfile::tempdir().unwrap();
    assert!(extract_portable(&portable_zip(dir2.path(), false), &dir2.path().join("s")).is_err());
}

/// Serves a GitHub-like feed for version 9.9.9 and its files.
async fn feed_server(files: Vec<(String, Vec<u8>)>, digest: bool, corrupt: bool) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let mut assets = Vec::new();
    let mut sums = String::new();
    for (name, data) in &files {
        let hash = hex::encode(Sha256::digest(data));
        sums.push_str(&format!("{hash}  {name}\n"));
        assets.push((
            name.as_str(),
            data.len() as u64,
            digest.then(|| format!("sha256:{hash}")),
        ));
    }
    let mut all = files.clone();
    if !digest {
        assets.push((CHECKSUMS_ASSET, sums.len() as u64, None));
        all.push((CHECKSUMS_ASSET.to_string(), sums.into_bytes()));
    }
    let feed = github_release(&base, "9.9.9", &assets);
    let mut app = Router::new().route("/feed", get(move || async move { axum::Json(feed) }));
    for (name, mut data) in all {
        if corrupt && name != CHECKSUMS_ASSET {
            data.push(0);
        }
        app = app.route(&format!("/files/{name}"), get(move || async move { data }));
    }
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    base
}

async fn use_feed(core: &Arc<Core>, base: &str) {
    let mut s = core.settings();
    s.server.enabled = false;
    s.updates.feed_url = format!("{base}/feed");
    core.apply_settings(s).await.unwrap();
}

#[tokio::test]
async fn check_finds_a_newer_version_and_remembers_it() {
    let core = core_with(MockPlatform::default());
    let base = feed_server(
        vec![(
            "remote-http-action_9.9.9_x64_en-US.msi".into(),
            b"msi".to_vec(),
        )],
        true,
        false,
    )
    .await;
    use_feed(&core, &base).await;

    let before = core
        .handle(Caller::Ui, Request::UpdateStatus)
        .await
        .unwrap();
    assert_eq!(before["available"], false);
    assert_eq!(before["last_check_at"], Value::Null);

    let status = core
        .handle(Caller::Cli, Request::UpdateCheck)
        .await
        .unwrap();
    assert_eq!(status["available"], true);
    assert_eq!(status["latest"]["version"], "9.9.9");
    assert_eq!(status["install_kind"], "msi");
    assert_eq!(
        status["asset"]["name"],
        "remote-http-action_9.9.9_x64_en-US.msi"
    );
    assert!(status["last_check_at"].is_string());
    // Stored: survives a fresh status call.
    assert_eq!(
        core.handle(Caller::Ui, Request::UpdateStatus)
            .await
            .unwrap()["available"],
        true
    );
}

#[tokio::test]
async fn failed_check_keeps_the_error() {
    let core = core_with(MockPlatform::default());
    let base = feed_server(vec![], true, false).await;
    let mut s = core.settings();
    s.server.enabled = false;
    s.updates.feed_url = format!("{base}/missing");
    core.apply_settings(s).await.unwrap();
    let err = core
        .handle(Caller::Ui, Request::UpdateCheck)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::NotFound);
    let status = core
        .handle(Caller::Ui, Request::UpdateStatus)
        .await
        .unwrap();
    assert!(
        status["error"]
            .as_str()
            .unwrap()
            .contains("no published release")
    );
    assert_eq!(status["phase"]["state"], "idle");
}

#[tokio::test]
async fn portable_update_is_downloaded_verified_and_extracted() {
    let core = core_with_paths(MockPlatform::default(), true);
    let dir = tempfile::tempdir().unwrap();
    let zip = std::fs::read(portable_zip(dir.path(), true)).unwrap();
    let base = feed_server(
        vec![("remote-http-action_9.9.9_x64_portable.zip".into(), zip)],
        false,
        false,
    )
    .await;
    use_feed(&core, &base).await;

    assert_eq!(
        core.handle(Caller::Ui, Request::UpdateDownload)
            .await
            .unwrap_err()
            .code,
        ErrorCode::NotFound,
        "download before a check"
    );
    core.handle(Caller::Ui, Request::UpdateCheck).await.unwrap();
    let status = core
        .handle(Caller::Ui, Request::UpdateDownload)
        .await
        .unwrap();
    assert_eq!(status["phase"]["state"], "ready");
    assert_eq!(status["phase"]["version"], "9.9.9");
    let staged = core
        .updates
        .work_dir()
        .join("staging")
        .join("Remote HTTP Action")
        .join("remote-http-action.exe");
    assert_eq!(std::fs::read(staged).unwrap(), b"new main");
}

#[tokio::test]
async fn corrupted_download_is_rejected_and_removed() {
    let core = core_with(MockPlatform::default());
    let base = feed_server(
        vec![(
            "remote-http-action_9.9.9_x64_en-US.msi".into(),
            b"msi bytes".to_vec(),
        )],
        true,
        true,
    )
    .await;
    use_feed(&core, &base).await;
    core.handle(Caller::Ui, Request::UpdateCheck).await.unwrap();
    let err = core
        .handle(Caller::Ui, Request::UpdateDownload)
        .await
        .unwrap_err();
    assert!(err.message.contains("checksum mismatch"), "{}", err.message);
    assert!(
        !core
            .updates
            .work_dir()
            .join("remote-http-action_9.9.9_x64_en-US.msi")
            .exists()
    );
    assert_eq!(
        core.handle(Caller::Ui, Request::UpdateStatus)
            .await
            .unwrap()["phase"]["state"],
        "idle"
    );
}

#[tokio::test]
async fn release_without_checksum_is_not_installed() {
    let core = core_with(MockPlatform::default());
    // No digest and no SHA256SUMS.txt.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let feed = github_release(
        &base,
        "9.9.9",
        &[("remote-http-action_9.9.9_x64_en-US.msi", 3, None)],
    );
    let app = Router::new().route("/feed", get(move || async move { axum::Json(feed) }));
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    use_feed(&core, &base).await;
    core.handle(Caller::Ui, Request::UpdateCheck).await.unwrap();
    let err = core
        .handle(Caller::Ui, Request::UpdateDownload)
        .await
        .unwrap_err();
    assert!(err.message.contains("no SHA-256 checksum"));
}

#[tokio::test]
async fn install_requires_a_download() {
    let core = core_with(MockPlatform::default());
    let err = core
        .handle(Caller::Ui, Request::UpdateInstall)
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Conflict);
}
