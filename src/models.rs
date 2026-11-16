use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const SESSION_FORWARD_FORMAT: &str = "ariadne-session-forward";
pub const SESSION_FORWARD_VERSION: i32 = 1;

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

#[derive(Debug, Deserialize, Serialize)]
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

#[cfg(test)]
mod tests {
    use super::{SessionForwardPayload, SessionForwardTrigger};

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
}
