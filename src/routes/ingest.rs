use std::io::Read;
use axum::body::Bytes;
use axum::extract::{FromRequest, Request, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use flate2::read::GzDecoder;

use crate::auth::authorize_ingest;
use crate::error::{AppError, AppResult};
use crate::models::{
    DiffConflictResponse, DiffRequest, DiffResponse, FeatureCountIncrements,
    HandshakeRequest, HandshakeResponse, SessionForwardPayload,
};
use crate::state::{AppState, BufferedChunk};
use crate::validate::{
    validate_diff_request, validate_handshake_request, validate_session_forward_payload,
};

// --- Extractor to automatically handle Content-Encoding: gzip or raw JSON ---

pub struct MaybeGzipJson<T>(pub T);

impl<S, T> FromRequest<S> for MaybeGzipJson<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let headers = req.headers().clone();
        let is_gzip = headers
            .get(axum::http::header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.contains("gzip"))
            .unwrap_or(false);

        let bytes = Bytes::from_request(req, state)
            .await
            .map_err(|err| err.into_response())?;

        let decompressed = if is_gzip {
            let mut decoder = GzDecoder::new(&bytes[..]);
            let mut decoded = Vec::new();
            decoder.read_to_end(&mut decoded).map_err(|err| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("gzip decompression failed: {}", err)
                    })),
                )
                    .into_response()
            })?;
            decoded
        } else {
            bytes.to_vec()
        };

        let data = serde_json::from_slice::<T>(&decompressed).map_err(|err| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("failed to parse json: {}", err)
                })),
            )
                .into_response()
        })?;

        Ok(MaybeGzipJson(data))
    }
}

// --- Legacy Session Forward ---

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

// --- v2 Handshake Endpoint ---

pub async fn post_handshake(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<HandshakeRequest>,
) -> AppResult<Json<HandshakeResponse>> {
    authorize_ingest(&headers, &state.ingest_api_key)?;
    validate_handshake_request(&body)?;

    let client_id = body.client_id.as_deref().unwrap_or("");
    let workspace_id = &body.workspace_id;

    let existing_state = state.store.get_workspace_state(client_id, workspace_id).await?;

    match existing_state {
        Some(ws_state) => {
            if ws_state.state_hash == body.client_state_hash {
                // Client hash matches server hash -> up-to-date
                return Ok(Json(HandshakeResponse::UpToDate {
                    server_state_hash: ws_state.state_hash,
                }));
            }

            // Mismatch: client needs diff
            let known_session_ids = state
                .store
                .get_known_completed_session_ids(client_id, workspace_id)
                .await?;

            Ok(Json(HandshakeResponse::NeedDiff {
                server_state_hash: ws_state.state_hash,
                known_completed_session_ids: known_session_ids,
                last_synced_timestamp: ws_state.updated_at.timestamp_millis(),
            }))
        }
        None => {
            // New workspace: empty server state hash, needs full diff
            Ok(Json(HandshakeResponse::NeedDiff {
                server_state_hash: String::new(),
                known_completed_session_ids: Vec::new(),
                last_synced_timestamp: 0,
            }))
        }
    }
}

// --- v2 Diff / Delta Ingestion Endpoint ---

pub async fn post_diff(
    State(state): State<AppState>,
    headers: HeaderMap,
    MaybeGzipJson(body): MaybeGzipJson<DiffRequest>,
) -> Result<Response, AppError> {
    authorize_ingest(&headers, &state.ingest_api_key)?;
    validate_diff_request(&body)?;

    let client_id = body.client_id.as_deref().unwrap_or("");
    let workspace_id = &body.workspace_id;

    // Check baseStateHash against current server state hash
    let existing_state = state.store.get_workspace_state(client_id, workspace_id).await?;
    let current_server_hash = existing_state
        .as_ref()
        .map(|s| s.state_hash.as_str())
        .unwrap_or("");

    // If client supplied a baseStateHash (not null / not empty), check if it matches current server hash
    if let Some(ref base_hash) = body.base_state_hash {
        if !base_hash.is_empty() && base_hash != current_server_hash {
            tracing::warn!(
                client_id,
                workspace_id,
                expected = %current_server_hash,
                received = %base_hash,
                "Diff rejected due to state hash mismatch"
            );
            return Ok((
                StatusCode::CONFLICT,
                Json(DiffConflictResponse {
                    success: false,
                    error: "STATE_HASH_MISMATCH",
                    server_state_hash: current_server_hash.to_string(),
                    message: "Server state does not match baseStateHash. Initiate fresh handshake."
                        .to_string(),
                }),
            )
                .into_response());
        }
    }

    let chunk_id = body.chunk.chunk_id.clone();
    let index = body.chunk.index;
    let total_chunks = body.chunk.total_chunks;

    // Buffer the incoming chunk
    let (ready_to_commit, chunks_remaining) = {
        let mut buffer = state.chunk_buffer.lock().await;
        buffer.cleanup_stale();

        let entry = buffer
            .batches
            .entry(chunk_id.clone())
            .or_insert_with(|| (total_chunks, std::collections::HashMap::new()));

        entry.0 = total_chunks;
        entry.1.insert(
            index,
            BufferedChunk {
                delta: body.delta.clone(),
                received_at: std::time::Instant::now(),
            },
        );

        let received_count = entry.1.len() as u32;
        let remaining = total_chunks.saturating_sub(received_count);
        let complete = received_count == total_chunks;

        (complete, remaining)
    };

    if ready_to_commit {
        // Retrieve and assemble all chunks for this batch
        let collected_chunks = {
            let mut buffer = state.chunk_buffer.lock().await;
            buffer.batches.remove(&chunk_id)
        };

        if let Some((_, chunk_map)) = collected_chunks {
            let mut all_completed_sessions = Vec::new();
            let mut all_upsert_lifecycles = Vec::new();
            let mut total_increments = FeatureCountIncrements {
                explain_vulnerability_delta: 0,
                audit_graph_open_delta: 0,
                file_location_open_delta: 0,
            };

            // Order chunks by index 1..=total_chunks
            for i in 1..=total_chunks {
                if let Some(buffered) = chunk_map.get(&i) {
                    all_completed_sessions.extend(buffered.delta.new_completed_sessions.clone());
                    all_upsert_lifecycles.extend(buffered.delta.upsert_lifecycles.clone());
                    total_increments.explain_vulnerability_delta +=
                        buffered.delta.feature_count_increments.explain_vulnerability_delta;
                    total_increments.audit_graph_open_delta +=
                        buffered.delta.feature_count_increments.audit_graph_open_delta;
                    total_increments.file_location_open_delta +=
                        buffered.delta.feature_count_increments.file_location_open_delta;
                }
            }

            // Persist atomically in database
            state
                .store
                .apply_diff_transaction(
                    client_id,
                    workspace_id,
                    &body.target_state_hash,
                    &all_completed_sessions,
                    &all_upsert_lifecycles,
                    &total_increments,
                )
                .await?;

            tracing::info!(
                chunk_id = %chunk_id,
                workspace_id = %workspace_id,
                target_state_hash = %body.target_state_hash,
                "Diff successfully applied and workspace state updated"
            );
        }

        let resp = DiffResponse {
            success: true,
            chunk_received: index,
            chunks_remaining: 0,
            current_server_state_hash: body.target_state_hash,
        };
        return Ok((StatusCode::OK, Json(resp)).into_response());
    }

    // Still waiting for more chunks
    let resp = DiffResponse {
        success: true,
        chunk_received: index,
        chunks_remaining,
        current_server_state_hash: current_server_hash.to_string(),
    };
    Ok((StatusCode::OK, Json(resp)).into_response())
}
