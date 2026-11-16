# ariadne-data-collection

Plain-HTTP ingestion server for **anonymous** Ariadne session metrics during live study periods. The VS Code extension sends aggregated scores and CWE-level lifecycle summaries — never source paths, fingerprints, or rule ids.

## Participant identity

The server hashes each participant’s GitHub numeric account id with a server-side pepper:

`user_hash = SHA-256(pepper || github_account_id)` (hex)

The raw GitHub id is validated and discarded; it is not stored or logged.

## When the extension should upload

Based on `ariadne-extension-vscode` session lifecycle:

| Event | When | Endpoint payload `event` |
| --- | --- | --- |
| **Session finalized** | Immediately after `finalizeSession(...)` for `completed` or `incomplete` (hourly rollover, shutdown, abrupt recovery) | `session_finalized` |
| **Hourly checkpoint** | After each hourly full-scan checkpoint is appended to `hourlyCheckpoints` (optional, lighter aggregate) | `hourly_checkpoint` |
| **Feature usage** | Privacy-safe counters (`explain_vulnerability`, `explain_vulnerability_fallback` with `reason`, optional `cweId`) | `feature_usage` |

Do **not** upload on every live scan or `onFindings` tick.

### Client-side privacy filter (required)

Before POSTing, map `SessionRecord` / `FindingLifecycleRecord` to the public DTO:

- Include: `cweId`, vulnerability `type`, `severity`, lifecycle state, occurrence counts, F/P/T, severity counts, CWE counts.
- Strip: `filePath`, `instanceName`, `logicalFingerprint`, `contentFingerprint`, `scopeFingerprint`, `ruleId`, and any raw code or repo paths.

The server rejects requests that still contain those keys.

## API

All ingest routes are protected by the shared bearer token; Railway terminates TLS upstream, so the app listens on plain HTTP.

- `GET /health` — liveness (`{"status":"ok"}`)
- `POST /v1/session-forward` — raw session-forward capture

Headers:

- `Authorization: Bearer <ARIADNE_INGEST_API_KEY>`
- `Content-Type: application/json`

Envelope (camelCase JSON):

```json
{
  "format": "ariadne-metrics-ingest",
  "version": 1,
  "githubAccountId": "12345678",
  "event": "session_finalized",
  "observedAt": "2026-03-22T12:00:00Z",
  "client": {
    "extensionVersion": "0.1.0",
    "vscodeVersion": "1.98.0",
    "platform": "linux-x64"
  },
  "session": {
    "clientSessionId": "session-001",
    "status": "completed",
    "startedAt": "2026-03-22T10:00:00Z",
    "endedAt": "2026-03-22T11:00:00Z",
    "finalScores": { "f": 7.5, "p": 6.0, "t": 1.2 },
    "baselineFindingCount": 12,
    "finalFindingCount": 8,
    "severityCounts": { "critical": 0, "high": 2, "medium": 4, "low": 2 },
    "lifecycleSummaries": []
  }
}
```

Success: `202`-style body `{"accepted":true,"eventId":"<uuid>"}` with HTTP 200.

Errors: JSON `{"error":"..."}` with `400`, `401`, or `500`.

## Local development

One command (Postgres, env, ingest server, and browser DB UI):

```bash
nix develop
./run_server.sh
```

- Ingest API: `http://127.0.0.1:8080`
- **DB browser (pgweb):** [http://127.0.0.1:8081/](http://127.0.0.1:8081/)

`./run_server.sh --release` for an optimized build. `./run_server.sh --no-db-ui` skips pgweb. **Stop:** Ctrl+C in the same terminal. From another terminal: `./run_server.sh --stop`.

Manual flow:

```bash
cp .env.example .env
cargo run
```

Migrations run automatically on startup (`sqlx::migrate!`).

## Railway deployment

Set these environment variables in Railway:

- `DATABASE_URL` — the Railway Postgres connection string
- `ARIADNE_INGEST_API_KEY` — a long shared secret, at least 32 chars
- `PORT` — Railway sets this automatically; default is `8080`

The app listens on plain HTTP and expects TLS to be terminated upstream by Railway. The health endpoint is available at `/health`.

## Configuration

See `.env.example` for `DATABASE_URL`, `ARIADNE_INGEST_API_KEY`, `PORT`, pgweb settings, and max body size.
