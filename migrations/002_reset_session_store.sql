DROP TABLE IF EXISTS ingest_events;
DROP TABLE IF EXISTS observation_sessions;
DROP TABLE IF EXISTS participants;

CREATE TABLE IF NOT EXISTS session_forwards (
    id SERIAL PRIMARY KEY,
    github_username TEXT,
    payload JSONB NOT NULL,
    forwarded_at TIMESTAMPTZ NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS session_forwards_github_username_idx
    ON session_forwards (github_username, forwarded_at DESC);

CREATE INDEX IF NOT EXISTS session_forwards_received_at_idx
    ON session_forwards (forwarded_at DESC);
