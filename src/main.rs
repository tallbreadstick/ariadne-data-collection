mod auth;
mod config;
mod db;
mod error;
mod hash;
mod models;
mod redirect;
mod routes;
mod state;
mod validate;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::db::Store;
use crate::routes::router;
use crate::state::AppState;

fn install_rustls_provider() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow::anyhow!("rustls CryptoProvider already installed"))?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    install_rustls_provider()?;
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = Config::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;

    let state = AppState {
        store: Store::new(pool),
        ingest_api_key: Arc::from(config.ingest_api_key),
    };

    let app = router(state)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(config.max_body_bytes));

    let https_addr: SocketAddr = config.https_addr.parse()?;
    let rustls_config =
        axum_server::tls_rustls::RustlsConfig::from_pem_file(config.tls_cert, config.tls_key).await?;

    if config.http_redirect_port > 0 {
        let redirect_bind = SocketAddr::from(([0, 0, 0, 0], config.http_redirect_port));
        let https_port = https_addr.port();
        tokio::spawn(async move {
            if let Err(err) = redirect::serve_http_redirect(redirect_bind, https_port).await {
                tracing::error!(?err, "HTTP redirect server exited");
            }
        });
    }

    tracing::info!(%https_addr, "HTTPS ingest server listening");
    axum_server::bind_rustls(https_addr, rustls_config)
        .serve(app.into_make_service())
        .await?;

    Ok(())
}
