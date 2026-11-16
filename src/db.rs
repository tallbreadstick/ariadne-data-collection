use sqlx::PgPool;

use crate::error::{AppError, AppResult};
use crate::models::SessionForwardPayload;

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn insert_session_forward(
        &self,
        github_username: Option<&str>,
        payload: &SessionForwardPayload,
    ) -> AppResult<i32> {
        let payload_json = serde_json::to_value(payload).map_err(|err| AppError::Other(err.into()))?;

        let record_id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO session_forwards (github_username, payload, forwarded_at)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
        )
        .bind(github_username.filter(|value| !value.trim().is_empty()))
        .bind(&payload_json)
        .bind(payload.forwarded_at)
        .fetch_one(&self.pool)
        .await?;

        Ok(record_id)
    }
}
