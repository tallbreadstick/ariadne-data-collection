use axum::http::HeaderMap;
use subtle::ConstantTimeEq;

use crate::error::{AppError, AppResult};

pub fn authorize_ingest(headers: &HeaderMap, expected_key: &str) -> AppResult<()> {
    let auth = headers
        .get(http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(AppError::Unauthorized)?;
    let token = auth
        .strip_prefix("Bearer ")
        .ok_or(AppError::Unauthorized)?;
    if token.as_bytes().ct_eq(expected_key.as_bytes()).into() {
        Ok(())
    } else {
        Err(AppError::Unauthorized)
    }
}
