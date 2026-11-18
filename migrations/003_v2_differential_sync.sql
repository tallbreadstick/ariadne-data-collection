CREATE TABLE IF NOT EXISTS workspace_state (
    client_id TEXT NOT NULL DEFAULT '',
    workspace_id TEXT NOT NULL,
    state_hash TEXT NOT NULL,
    feature_counts JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (client_id, workspace_id)
);

CREATE TABLE IF NOT EXISTS finding_lifecycles (
    client_id TEXT NOT NULL DEFAULT '',
    workspace_id TEXT NOT NULL,
    logical_fingerprint TEXT NOT NULL,
    data JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (client_id, workspace_id, logical_fingerprint)
);

CREATE TABLE IF NOT EXISTS completed_sessions (
    client_id TEXT NOT NULL DEFAULT '',
    workspace_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    data JSONB NOT NULL,
    completed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (client_id, workspace_id, session_id)
);
