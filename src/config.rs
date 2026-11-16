use std::env;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub ingest_api_key: String,
    pub bind_addr: String,
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
            bind_addr: env::var("PORT").or_else(|_| env::var("BIND_ADDR")).unwrap_or_else(|_| "0.0.0.0:8080".into()),
            max_body_bytes: env::var("ARIADNE_MAX_BODY_BYTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(256 * 1024),
        })
    }
}
