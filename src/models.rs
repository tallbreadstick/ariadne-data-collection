use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const SESSION_FORWARD_FORMAT: &str = "ariadne-session-forward";
pub const SESSION_FORWARD_VERSION: i32 = 1;

pub const HANDSHAKE_FORMAT: &str = "ariadne-sync-handshake";
pub const HANDSHAKE_VERSION: i32 = 2;

pub const DIFF_FORMAT: &str = "ariadne-session-diff";
pub const DIFF_VERSION: i32 = 2;

// --- Legacy v1 models ---

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionForwardTrigger {
    Save,
    Close,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionForwardPayload {
    pub format: String,
    pub version: i32,
    pub forwarded_at: DateTime<Utc>,
    pub trigger: SessionForwardTrigger,
    pub github_account_id: Option<String>,
    pub github_username: Option<String>,
    pub client: ClientInfo,
    pub feature_counts: FeatureCounts,
    pub session_snapshot: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub extension_version: String,
    pub vscode_version: String,
    pub platform: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureCounts {
    pub explain_vulnerability_count: i64,
    pub audit_graph_open_count: i64,
    pub file_location_open_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionForwardAccepted {
    pub accepted: bool,
    pub record_id: i32,
}

// --- v2 Handshake models ---

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeRequest {
    pub format: String,
    pub version: i32,
    pub timestamp: String,
    pub client_id: Option<String>,
    pub workspace_id: String,
    pub client_state_hash: String,
    pub meta: HandshakeMeta,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HandshakeMeta {
    pub session_id_seed: i64,
    pub total_scans_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum HandshakeResponse {
    UpToDate {
        server_state_hash: String,
    },
    NeedDiff {
        server_state_hash: String,
        known_completed_session_ids: Vec<String>,
        last_synced_timestamp: i64,
    },
}

// --- v2 Diff models ---

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffTrigger {
    Save,
    Close,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkEnvelope {
    pub chunk_id: String,
    pub index: u32,
    pub total_chunks: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffRequest {
    pub format: String,
    pub version: i32,
    pub timestamp: String,
    pub trigger: DiffTrigger,
    pub client_id: Option<String>,
    pub workspace_id: String,
    pub base_state_hash: Option<String>,
    pub target_state_hash: String,
    pub client: ClientInfo,
    pub chunk: ChunkEnvelope,
    pub delta: DiffDelta,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffDelta {
    #[serde(default)]
    pub new_completed_sessions: Vec<serde_json::Value>,
    pub active_session_update: Option<ActiveSessionUpdate>,
    #[serde(default)]
    pub upsert_lifecycles: Vec<serde_json::Value>,
    pub feature_count_increments: FeatureCountIncrements,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveSessionUpdate {
    pub session_id: String,
    pub started_at: i64,
    pub status: String,
    pub latest_checkpoint: Option<LatestCheckpoint>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LatestCheckpoint {
    pub timestamp: i64,
    pub findings_count: i64,
    #[serde(default)]
    pub severity_counts: HashMap<String, i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeatureCountIncrements {
    #[serde(default)]
    pub explain_vulnerability_delta: i64,
    #[serde(default)]
    pub audit_graph_open_delta: i64,
    #[serde(default)]
    pub file_location_open_delta: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffResponse {
    pub success: bool,
    pub chunk_received: u32,
    pub chunks_remaining: u32,
    pub current_server_state_hash: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffConflictResponse {
    pub success: bool,
    pub error: &'static str,
    pub server_state_hash: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_session_forward_payload() {
        let payload = r#"{
            "format": "ariadne-session-forward",
            "version": 1,
            "forwardedAt": "2026-11-16T14:55:00.000Z",
            "trigger": "save",
            "githubAccountId": "12345678",
            "githubUsername": "octocat",
            "client": {
                "extensionVersion": "0.1.4",
                "vscodeVersion": "1.100.0",
                "platform": "linux-x64"
            },
            "featureCounts": {
                "explainVulnerabilityCount": 5,
                "auditGraphOpenCount": 2,
                "fileLocationOpenCount": 8
            },
            "sessionSnapshot": {
                "format": "ariadne-session-export",
                "version": 1,
                "exportedAt": "2026-11-16T14:55:00.000Z",
                "activeSession": null,
                "completedSessions": [],
                "findingLifecycles": [],
                "sessionMeta": {},
                "saveScanState": {},
                "graduationHistory": {},
                "dismissedNotifications": []
            }
        }"#;

        let parsed: SessionForwardPayload = serde_json::from_str(payload).unwrap();
        assert_eq!(parsed.format, "ariadne-session-forward");
        assert_eq!(parsed.version, 1);
        assert_eq!(parsed.trigger, SessionForwardTrigger::Save);
        assert_eq!(parsed.github_username.as_deref(), Some("octocat"));
    }

    #[test]
    fn parses_handshake_request() {
        let payload = r#"{
            "format": "ariadne-sync-handshake",
            "version": 2,
            "timestamp": "2026-11-18T08:45:00.000Z",
            "clientId": "a35f79bc89234857ef1234567890abcdef1234567890abcdef1234567890abcd",
            "workspaceId": "f78234891234abcdef567890abcdef1234567890abcdef1234567890abcdef12",
            "clientStateHash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "meta": {
                "sessionIdSeed": 1,
                "totalScansCount": 10
            }
        }"#;
        let parsed: HandshakeRequest = serde_json::from_str(payload).unwrap();
        assert_eq!(parsed.format, "ariadne-sync-handshake");
        assert_eq!(parsed.version, 2);
    }

    #[test]
    fn parses_diff_request() {
        let payload = r#"{
            "format": "ariadne-session-diff",
            "version": 2,
            "timestamp": "2026-11-18T08:45:00.000Z",
            "trigger": "save",
            "clientId": "a35f79bc89234857ef1234567890abcdef1234567890abcdef1234567890abcd",
            "workspaceId": "f78234891234abcdef567890abcdef1234567890abcdef1234567890abcdef12",
            "baseStateHash": "1111111111111111111111111111111111111111111111111111111111111111",
            "targetStateHash": "2222222222222222222222222222222222222222222222222222222222222222",
            "client": {
                "extensionVersion": "0.1.5",
                "vscodeVersion": "1.100.0",
                "platform": "linux-x64"
            },
            "chunk": {
                "chunkId": "3b29c118-20da-4a57-8176-7871b658d3c1",
                "index": 1,
                "totalChunks": 1
            },
            "delta": {
                "newCompletedSessions": [],
                "activeSessionUpdate": {
                    "sessionId": "session-002",
                    "startedAt": 1794837600000,
                    "status": "active",
                    "latestCheckpoint": {
                        "timestamp": 1794837900000,
                        "findingsCount": 1,
                        "severityCounts": { "critical": 1, "high": 0, "medium": 0, "low": 0 }
                    }
                },
                "upsertLifecycles": [
                    {
                        "logicalFingerprint": "CWE-89:query:src/Main.java:24",
                        "cweId": "CWE-89",
                        "type": "SQL Injection",
                        "severity": "critical"
                    }
                ],
                "featureCountIncrements": {
                    "explainVulnerabilityDelta": 1,
                    "auditGraphOpenDelta": 0,
                    "fileLocationOpenDelta": 0
                }
            }
        }"#;
        let parsed: DiffRequest = serde_json::from_str(payload).unwrap();
        assert_eq!(parsed.format, "ariadne-session-diff");
        assert_eq!(parsed.chunk.index, 1);
        assert_eq!(parsed.delta.upsert_lifecycles.len(), 1);
    }
}
