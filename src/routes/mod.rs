mod health;
mod ingest;

use axum::routing::{get, post};
use axum::Router;

use crate::state::AppState;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .route("/v1/session-forward", post(ingest::post_session_forward))
        .with_state(state)
}
