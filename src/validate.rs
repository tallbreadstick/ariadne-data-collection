use chrono::Utc;

use crate::error::{AppError, AppResult};
use crate::models::{
    ClientInfo, DiffRequest, FeatureCounts, HandshakeRequest, SessionForwardPayload, DIFF_FORMAT,
    DIFF_VERSION, HANDSHAKE_FORMAT, HANDSHAKE_VERSION, SESSION_FORWARD_FORMAT,
    SESSION_FORWARD_VERSION,
};

pub fn validate_session_forward_payload(body: &SessionForwardPayload) -> AppResult<()> {
    if body.format != SESSION_FORWARD_FORMAT {
        return Err(AppError::BadRequest("unsupported format".into()));
    }
    if body.version != SESSION_FORWARD_VERSION {
        return Err(AppError::BadRequest("unsupported version".into()));
    }
    validate_forwarded_at(body.forwarded_at)?;
    validate_client(&body.client)?;
    validate_feature_counts(&body.feature_counts)?;

    if body.session_snapshot.is_null() {
        return Err(AppError::BadRequest("sessionSnapshot is required".into()));
    }

    if let Some(username) = &body.github_username {
        let trimmed = username.trim();
        if trimmed.is_empty() || trimmed.len() > 255 {
            return Err(AppError::BadRequest("invalid githubUsername".into()));
        }
    }

    if let Some(account_id) = &body.github_account_id {
        let trimmed = account_id.trim();
        if !trimmed.is_empty() && trimmed.len() > 64 {
            return Err(AppError::BadRequest("invalid githubAccountId".into()));
        }
    }

    Ok(())
}

pub fn validate_handshake_request(req: &HandshakeRequest) -> AppResult<()> {
    if req.format != HANDSHAKE_FORMAT {
        return Err(AppError::BadRequest("unsupported format".into()));
    }
    if req.version != HANDSHAKE_VERSION {
        return Err(AppError::BadRequest("unsupported version".into()));
    }
    if req.workspace_id.trim().is_empty() {
        return Err(AppError::BadRequest("workspaceId is required".into()));
    }
    if req.client_state_hash.trim().is_empty() {
        return Err(AppError::BadRequest("clientStateHash is required".into()));
    }
    Ok(())
}

pub fn validate_diff_request(req: &DiffRequest) -> AppResult<()> {
    if req.format != DIFF_FORMAT {
        return Err(AppError::BadRequest("unsupported format".into()));
    }
    if req.version != DIFF_VERSION {
        return Err(AppError::BadRequest("unsupported version".into()));
    }
    if req.workspace_id.trim().is_empty() {
        return Err(AppError::BadRequest("workspaceId is required".into()));
    }
    if req.target_state_hash.trim().is_empty() {
        return Err(AppError::BadRequest("targetStateHash is required".into()));
    }
    if req.chunk.chunk_id.trim().is_empty() {
        return Err(AppError::BadRequest("chunkId is required".into()));
    }
    if req.chunk.index == 0 || req.chunk.total_chunks == 0 || req.chunk.index > req.chunk.total_chunks {
        return Err(AppError::BadRequest("invalid chunk index/totalChunks".into()));
    }
    validate_client(&req.client)?;
    Ok(())
}

fn validate_client(client: &ClientInfo) -> AppResult<()> {
    if client.extension_version.trim().is_empty() || client.extension_version.trim().len() > 64 {
        return Err(AppError::BadRequest("invalid extensionVersion".into()));
    }
    if client.vscode_version.trim().is_empty() || client.vscode_version.trim().len() > 64 {
        return Err(AppError::BadRequest("invalid vscodeVersion".into()));
    }
    if client.platform.trim().is_empty() || client.platform.trim().len() > 64 {
        return Err(AppError::BadRequest("invalid platform".into()));
    }
    Ok(())
}

fn validate_forwarded_at(ts: chrono::DateTime<Utc>) -> AppResult<()> {
    let now = Utc::now();
    if ts > now + chrono::Duration::minutes(5) {
        return Err(AppError::BadRequest("forwardedAt is in the future".into()));
    }
    if ts < chrono::DateTime::parse_from_rfc3339("2024-01-01T00:00:00Z")
        .map_err(|err| AppError::Other(err.into()))?
        .with_timezone(&Utc)
    {
        return Err(AppError::BadRequest("forwardedAt is too old".into()));
    }
    Ok(())
}

fn validate_feature_counts(counts: &FeatureCounts) -> AppResult<()> {
    let values = [
        counts.explain_vulnerability_count,
        counts.audit_graph_open_count,
        counts.file_location_open_count,
    ];
    if values.iter().any(|count| *count < 0) {
        return Err(AppError::BadRequest("featureCounts cannot be negative".into()));
    }
    Ok(())
}
