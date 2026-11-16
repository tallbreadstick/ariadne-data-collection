use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;

use crate::auth::authorize_ingest;
use crate::error::AppResult;
use crate::models::SessionForwardPayload;
use crate::state::AppState;
use crate::validate::validate_session_forward_payload;

pub async fn post_session_forward(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SessionForwardPayload>,
) -> AppResult<StatusCode> {
    authorize_ingest(&headers, &state.ingest_api_key)?;
    validate_session_forward_payload(&body)?;

    let github_username = body
        .github_username
        .as_deref()
        .filter(|value| !value.trim().is_empty());

    let record_id = state.store.insert_session_forward(github_username, &body).await?;
    tracing::info!(record_id, trigger = ?body.trigger, "session forward stored");

    Ok(StatusCode::OK)
}
