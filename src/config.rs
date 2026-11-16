use std::env;
use std::path::PathBuf;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub ingest_api_key: String,
    pub tls_cert: PathBuf,
    pub tls_key: PathBuf,
    pub https_addr: String,
    pub http_redirect_port: u16,
    pub max_body_bytes: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL is required")?;
        let ingest_api_key =
            env::var("ARIADNE_INGEST_API_KEY").context("ARIADNE_INGEST_API_KEY is required")?;
        if ingest_api_key.len() < 32 {
            anyhow::bail!("ARIADNE_INGEST_API_KEY must be at least 32 characters");
        }

        Ok(Self {
            database_url,
            ingest_api_key,
            tls_cert: env::var("ARIADNE_TLS_CERT")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./certs/cert.pem")),
            tls_key: env::var("ARIADNE_TLS_KEY")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./certs/key.pem")),
            https_addr: env::var("ARIADNE_HTTPS_ADDR").unwrap_or_else(|_| "0.0.0.0:8443".into()),
            http_redirect_port: env::var("ARIADNE_HTTP_REDIRECT_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8080),
            max_body_bytes: env::var("ARIADNE_MAX_BODY_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(256 * 1024),
        })
    }
}
