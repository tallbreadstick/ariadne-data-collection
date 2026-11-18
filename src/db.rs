use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::{PgPool, Row};

use crate::error::{AppError, AppResult};
use crate::models::{FeatureCountIncrements, SessionForwardPayload};

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

#[derive(Debug)]
pub struct WorkspaceStateRecord {
    pub client_id: String,
    pub workspace_id: String,
    pub state_hash: String,
    pub feature_counts: serde_json::Value,
    pub updated_at: DateTime<Utc>,
}

impl Store {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // --- Legacy session forward insertion ---

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

    // --- v2 Handshake queries ---

    pub async fn get_workspace_state(
        &self,
        client_id: &str,
        workspace_id: &str,
    ) -> AppResult<Option<WorkspaceStateRecord>> {
        let row = sqlx::query(
            r#"
            SELECT client_id, workspace_id, state_hash, feature_counts, updated_at
            FROM workspace_state
            WHERE client_id = $1 AND workspace_id = $2
            "#,
        )
        .bind(client_id)
        .bind(workspace_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            Ok(Some(WorkspaceStateRecord {
                client_id: row.get("client_id"),
                workspace_id: row.get("workspace_id"),
                state_hash: row.get("state_hash"),
                feature_counts: row.get("feature_counts"),
                updated_at: row.get("updated_at"),
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn get_known_completed_session_ids(
        &self,
        client_id: &str,
        workspace_id: &str,
    ) -> AppResult<Vec<String>> {
        let rows = sqlx::query_scalar::<_, String>(
            r#"
            SELECT session_id
            FROM completed_sessions
            WHERE client_id = $1 AND workspace_id = $2
            "#,
        )
        .bind(client_id)
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows)
    }

    // --- v2 Diff application transaction ---

    pub async fn apply_diff_transaction(
        &self,
        client_id: &str,
        workspace_id: &str,
        target_state_hash: &str,
        completed_sessions: &[Value],
        upsert_lifecycles: &[Value],
        increments: &FeatureCountIncrements,
    ) -> AppResult<()> {
        let mut tx = self.pool.begin().await?;

        // 1. Upsert finding lifecycles
        for lifecycle in upsert_lifecycles {
            let logical_fingerprint = lifecycle
                .get("logicalFingerprint")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if logical_fingerprint.is_empty() {
                continue;
            }

            sqlx::query(
                r#"
                INSERT INTO finding_lifecycles (client_id, workspace_id, logical_fingerprint, data, updated_at)
                VALUES ($1, $2, $3, $4, NOW())
                ON CONFLICT (client_id, workspace_id, logical_fingerprint)
                DO UPDATE SET
                    data = EXCLUDED.data,
                    updated_at = NOW()
                "#,
            )
            .bind(client_id)
            .bind(workspace_id)
            .bind(logical_fingerprint)
            .bind(lifecycle)
            .execute(&mut *tx)
            .await?;
        }

        // 2. Insert completed sessions
        for session in completed_sessions {
            let session_id = session
                .get("sessionId")
                .and_then(|v| v.as_str())
                .unwrap_or_default();

            if session_id.is_empty() {
                continue;
            }

            sqlx::query(
                r#"
                INSERT INTO completed_sessions (client_id, workspace_id, session_id, data, completed_at)
                VALUES ($1, $2, $3, $4, NOW())
                ON CONFLICT (client_id, workspace_id, session_id)
                DO NOTHING
                "#,
            )
            .bind(client_id)
            .bind(workspace_id)
            .bind(session_id)
            .bind(session)
            .execute(&mut *tx)
            .await?;
        }

        // 3. Upsert workspace state record with incremented feature counts
        // Fetch current feature counts in transaction if exists
        let current_counts_row = sqlx::query(
            r#"
            SELECT feature_counts
            FROM workspace_state
            WHERE client_id = $1 AND workspace_id = $2
            FOR UPDATE
            "#,
        )
        .bind(client_id)
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?;

        let mut explain_count: i64 = 0;
        let mut audit_count: i64 = 0;
        let mut file_loc_count: i64 = 0;

        if let Some(row) = current_counts_row {
            let counts_val: Value = row.get("feature_counts");
            if let Some(obj) = counts_val.as_object() {
                explain_count = obj.get("explainVulnerabilityDelta")
                    .or_else(|| obj.get("explainVulnerabilityCount"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                audit_count = obj.get("auditGraphOpenDelta")
                    .or_else(|| obj.get("auditGraphOpenCount"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                file_loc_count = obj.get("fileLocationOpenDelta")
                    .or_else(|| obj.get("fileLocationOpenCount"))
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
            }
        }

        explain_count += increments.explain_vulnerability_delta;
        audit_count += increments.audit_graph_open_delta;
        file_loc_count += increments.file_location_open_delta;

        let new_feature_counts = serde_json::json!({
            "explainVulnerabilityCount": explain_count,
            "auditGraphOpenCount": audit_count,
            "fileLocationOpenCount": file_loc_count,
        });

        sqlx::query(
            r#"
            INSERT INTO workspace_state (client_id, workspace_id, state_hash, feature_counts, updated_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (client_id, workspace_id)
            DO UPDATE SET
                state_hash = EXCLUDED.state_hash,
                feature_counts = EXCLUDED.feature_counts,
                updated_at = NOW()
            "#,
        )
        .bind(client_id)
        .bind(workspace_id)
        .bind(target_state_hash)
        .bind(&new_feature_counts)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }
}
