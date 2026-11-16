mod auth;
mod config;
mod db;
mod error;
mod hash;
mod models;
mod routes;
mod state;
mod validate;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::Router;
use sqlx::postgres::PgPoolOptions;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::db::Store;
use crate::routes::router;
use crate::state::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
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

    let app: Router = router(state)
        .layer(TraceLayer::new_for_http())
        .layer(DefaultBodyLimit::max(config.max_body_bytes));

    let bind_addr: SocketAddr = config.bind_addr.parse()?;
    tracing::info!(%bind_addr, "HTTP ingest server listening");
    axum::serve(
        tokio::net::TcpListener::bind(bind_addr).await?,
        app.into_make_service(),
    )
    .await?;

    Ok(())
}
