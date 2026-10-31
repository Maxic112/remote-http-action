//! HTTPS certificates: self-signed generation, loading and fingerprints.

use std::fs;
use std::path::{Path, PathBuf};

use axum_server::tls_rustls::RustlsConfig;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::error::{AppError, AppResult};
use crate::settings::{TlsMode, TlsSettings};

/// Installs the `ring` crypto provider for rustls (once per process).
pub fn ensure_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TlsInfo {
    pub mode: TlsMode,
    pub cert_path: String,
    pub key_path: String,
    /// SHA-256 of the certificate (DER), `AA:BB:…`; `None` if it does not exist yet.
    pub fingerprint: Option<String>,
    pub not_after: Option<String>,
}

pub fn self_signed_paths(data_dir: &Path) -> (PathBuf, PathBuf) {
    let dir = data_dir.join("tls");
    (dir.join("cert.pem"), dir.join("key.pem"))
}

/// Certificate and key files used for the given settings.
pub fn cert_paths(tls: &TlsSettings, data_dir: &Path) -> (PathBuf, PathBuf) {
    match tls.mode {
        TlsMode::SelfSigned => self_signed_paths(data_dir),
        TlsMode::Custom => (PathBuf::from(&tls.cert_path), PathBuf::from(&tls.key_path)),
    }
}

fn subject_names() -> Vec<String> {
    let mut names = vec!["localhost".to_string(), "127.0.0.1".to_string()];
    if let Ok(host) = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")) {
        names.push(host.to_lowercase());
    }
    if let Ok(list) = if_addrs::get_if_addrs() {
        for i in list
            .into_iter()
            .filter(|i| i.ip().is_ipv4() && !i.is_loopback())
        {
            names.push(i.ip().to_string());
        }
    }
    names.dedup();
    names
}

/// Generates a new self-signed certificate (valid for 10 years) and stores it in the data folder.
pub fn generate_self_signed(data_dir: &Path) -> AppResult<()> {
    let (cert_path, key_path) = self_signed_paths(data_dir);
    let mut params = rcgen::CertificateParams::new(subject_names())
        .map_err(|e| AppError::internal(format!("certificate parameters: {e}")))?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, "Remote HTTP Action");
    let now = time_now();
    params.not_before = now;
    params.not_after = now + std::time::Duration::from_secs(10 * 365 * 24 * 3600);
    let key = rcgen::KeyPair::generate()
        .map_err(|e| AppError::internal(format!("key generation: {e}")))?;
    let cert = params
        .self_signed(&key)
        .map_err(|e| AppError::internal(format!("certificate generation: {e}")))?;
    fs::create_dir_all(cert_path.parent().unwrap())?;
    fs::write(&cert_path, cert.pem())?;
    fs::write(&key_path, key.serialize_pem())?;
    Ok(())
}

fn time_now() -> time::OffsetDateTime {
    time::OffsetDateTime::now_utc()
}

/// Makes sure certificate files exist (generating a self-signed one if needed) and loads them.
pub async fn load(tls: &TlsSettings, data_dir: &Path) -> AppResult<RustlsConfig> {
    ensure_crypto_provider();
    if tls.mode == TlsMode::SelfSigned {
        let (cert, key) = self_signed_paths(data_dir);
        if !cert.is_file() || !key.is_file() {
            generate_self_signed(data_dir)?;
        }
    }
    let (cert, key) = cert_paths(tls, data_dir);
    RustlsConfig::from_pem_file(&cert, &key).await.map_err(|e| {
        AppError::validation(format!(
            "cannot load TLS certificate {} / key {}: {e}",
            cert.display(),
            key.display()
        ))
    })
}

fn first_cert_der(cert_path: &Path) -> Option<Vec<u8>> {
    let pem = fs::read(cert_path).ok()?;
    let parsed = pem::parse_many(pem).ok()?;
    parsed
        .into_iter()
        .find(|p| p.tag() == "CERTIFICATE")
        .map(|p| p.into_contents())
}

pub fn fingerprint(cert_path: &Path) -> Option<String> {
    let der = first_cert_der(cert_path)?;
    Some(
        Sha256::digest(&der)
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(":"),
    )
}

fn not_after(cert_path: &Path) -> Option<String> {
    let der = first_cert_der(cert_path)?;
    let (_, cert) = x509_parser::parse_x509_certificate(&der).ok()?;
    Some(cert.validity().not_after.to_rfc2822().unwrap_or_default())
}

pub fn info(tls: &TlsSettings, data_dir: &Path) -> TlsInfo {
    let (cert, key) = cert_paths(tls, data_dir);
    TlsInfo {
        mode: tls.mode,
        fingerprint: fingerprint(&cert),
        not_after: not_after(&cert),
        cert_path: cert.display().to_string(),
        key_path: key.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn self_signed_certificate_is_generated_once_and_loadable() {
        let dir = tempfile::tempdir().unwrap();
        let tls = TlsSettings {
            enabled: true,
            ..Default::default()
        };
        load(&tls, dir.path()).await.unwrap();
        let first = info(&tls, dir.path());
        let fp = first.fingerprint.clone().unwrap();
        assert_eq!(fp.split(':').count(), 32);
        assert!(first.not_after.is_some());

        load(&tls, dir.path()).await.unwrap();
        assert_eq!(info(&tls, dir.path()).fingerprint.unwrap(), fp);

        generate_self_signed(dir.path()).unwrap();
        assert_ne!(info(&tls, dir.path()).fingerprint.unwrap(), fp);
    }

    #[tokio::test]
    async fn missing_custom_certificate_is_a_validation_error() {
        let dir = tempfile::tempdir().unwrap();
        let tls = TlsSettings {
            enabled: true,
            mode: TlsMode::Custom,
            cert_path: dir.path().join("none.pem").display().to_string(),
            key_path: dir.path().join("none.key").display().to_string(),
        };
        let err = load(&tls, dir.path()).await.unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::Validation);
        assert!(info(&tls, dir.path()).fingerprint.is_none());
    }
}
