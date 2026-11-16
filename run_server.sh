#!/usr/bin/env bash
# Start local Postgres (if needed), ensure dev TLS + env, then run the HTTPS ingest server.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

PGDATA="${PGDATA:-$ROOT/.postgres}"
PGPORT="${PGPORT:-5433}"
PGHOST="${PGHOST:-127.0.0.1}"
PGUSER="${PGUSER:-ariadne}"
PGDATABASE="${PGDATABASE:-ariadne_metrics}"

RELEASE=0
STOP_ONLY=0
NO_DB_UI=0
PGWEB_PID=""
STARTED_PG=0
SHUTTING_DOWN=0

usage() {
  cat <<'EOF'
Usage: ./run_server.sh [options]

  Starts embedded/local Postgres (when using ./.postgres), prepares certs and env,
  builds if needed, and runs the Ariadne metrics ingest server (HTTPS + HTTP redirect).
  Also starts pgweb on 127.0.0.1:8081 for a browser DB UI (override with ARIADNE_PGWEB_PORT).

Options:
  --release     cargo run --release
  --no-db-ui    skip pgweb browser UI
  --stop        stop ingest server, pgweb, and ./.postgres (if running) and exit
  -h, --help    show this help

Environment:
  PGDATA, PGPORT, PGHOST — Postgres data dir and connection (defaults: ./.postgres, 5433, 127.0.0.1)
  DATABASE_URL           — overrides default postgres://ariadne:ariadne@127.0.0.1:$PGPORT/ariadne_metrics
  Other vars: see .env.example (loaded from .env when present)

Requires: cargo, openssl; Postgres tools (initdb, pg_ctl, psql, pg_isready) on PATH
          (e.g. nix develop, or system postgresql).
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) RELEASE=1 ;;
    --no-db-ui) NO_DB_UI=1 ;;
    --stop) STOP_ONLY=1 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "Unknown option: $1" >&2; usage >&2; exit 1 ;;
  esac
  shift
done

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing required command: $1" >&2
    echo "Tip: run 'nix develop' in this directory, or install PostgreSQL client/server tools." >&2
    exit 1
  fi
}

ensure_env_file() {
  local explicit_database_url="${DATABASE_URL:-}"

  if [[ ! -f "$ROOT/.env" && -f "$ROOT/.env.example" ]]; then
    cp "$ROOT/.env.example" "$ROOT/.env"
    echo "Created .env from .env.example"
  fi
  if [[ -f "$ROOT/.env" ]]; then
    set -a
    # shellcheck disable=SC1091
    source "$ROOT/.env"
    set +a
  fi

  if [[ -n "$explicit_database_url" ]]; then
    export DATABASE_URL="$explicit_database_url"
  else
    export DATABASE_URL="postgres://${PGUSER}:${PGUSER}@${PGHOST}:${PGPORT}/${PGDATABASE}"
  fi
}

ensure_dev_secrets() {
  local key="${ARIADNE_INGEST_API_KEY:-}"
  if [[ "$key" == "change-me-before-production" || ${#key} -lt 32 ]]; then
    key="$(openssl rand -hex 32)"
    export ARIADNE_INGEST_API_KEY="$key"
    echo "Generated dev ARIADNE_INGEST_API_KEY (export only; update .env for a fixed value)"
  fi
}

ensure_tls_certs() {
  local cert="${ARIADNE_TLS_CERT:-./certs/cert.pem}"
  local key="${ARIADNE_TLS_KEY:-./certs/key.pem}"
  if [[ "$cert" != /* ]]; then cert="$ROOT/$cert"; fi
  if [[ "$key" != /* ]]; then key="$ROOT/$key"; fi
  if [[ ! -f "$cert" || ! -f "$key" ]]; then
    mkdir -p "$(dirname "$cert")"
    echo "Generating self-signed TLS certificate in $(dirname "$cert")"
    openssl req -x509 -newkey rsa:2048 -nodes \
      -keyout "$key" -out "$cert" -days 825 \
      -subj "/CN=localhost" 2>/dev/null
  fi
  export ARIADNE_TLS_CERT="$cert"
  export ARIADNE_TLS_KEY="$key"
}

postgres_using_local_data_dir() {
  [[ "$(realpath "$PGDATA" 2>/dev/null || echo "$PGDATA")" == "$(realpath "$ROOT/.postgres" 2>/dev/null || echo "$ROOT/.postgres")" ]] \
    || [[ "$PGDATA" == "$ROOT/.postgres" ]]
}

configure_postgres_for_local_dev() {
  # Nix-packaged Postgres defaults to /run/postgresql for Unix sockets, which
  # is missing for user-owned clusters. Keep sockets under PGDATA instead.
  mkdir -p "$PGDATA/sockets"
  local conf="$PGDATA/postgresql.conf"
  if [[ ! -f "$conf" ]]; then
    return
  fi
  if ! grep -q "^unix_socket_directories" "$conf" 2>/dev/null; then
    {
      echo ""
      echo "# ariadne-data-collection — Nix-friendly local sockets"
      echo "unix_socket_directories = '$PGDATA/sockets'"
    } >>"$conf"
  fi
  if ! grep -qE '^[[:space:]]*listen_addresses[[:space:]]*=' "$conf" 2>/dev/null; then
    echo "listen_addresses = '$PGHOST'" >>"$conf"
  fi
  if ! grep -qE '^[[:space:]]*port[[:space:]]*=' "$conf" 2>/dev/null; then
    echo "port = $PGPORT" >>"$conf"
  fi
}

init_postgres_if_needed() {
  if [[ ! -d "$PGDATA" ]]; then
    echo "Initializing Postgres data directory at $PGDATA"
    initdb -D "$PGDATA" --auth=trust
    configure_postgres_for_local_dev
  else
    configure_postgres_for_local_dev
  fi
}

start_postgres() {
  if pg_isready -h "$PGHOST" -p "$PGPORT" >/dev/null 2>&1; then
    echo "Postgres already accepting connections on $PGHOST:$PGPORT"
    return 0
  fi

  if ! postgres_using_local_data_dir; then
    echo "Postgres is not ready at $PGHOST:$PGPORT and PGDATA is not $ROOT/.postgres" >&2
    echo "Start your database or set PGDATA to the local ./.postgres directory." >&2
    exit 1
  fi

  init_postgres_if_needed
  echo "Starting Postgres (PGDATA=$PGDATA, port=$PGPORT)"
  if ! pg_ctl -D "$PGDATA" -l "$PGDATA/server.log" start -w; then
    echo "pg_ctl failed. Last lines of $PGDATA/server.log:" >&2
    tail -n 20 "$PGDATA/server.log" >&2 || true
    exit 1
  fi
  STARTED_PG=1

  export PGHOST PGPORT
  createuser -h "$PGHOST" -p "$PGPORT" -s "$PGUSER" 2>/dev/null || true
  psql -h "$PGHOST" -p "$PGPORT" -v ON_ERROR_STOP=0 -c "ALTER USER \"$PGUSER\" WITH PASSWORD 'ariadne';" postgres >/dev/null 2>&1 || true
  createdb -h "$PGHOST" -p "$PGPORT" -O "$PGUSER" "$PGDATABASE" 2>/dev/null || true
}

pgweb_database_url() {
  local url="$DATABASE_URL"
  if [[ "$url" != *sslmode=* ]]; then
    if [[ "$url" == *\?* ]]; then
      url="${url}&sslmode=disable"
    else
      url="${url}?sslmode=disable"
    fi
  fi
  printf '%s' "$url"
}

pgweb_cmd() {
  if command -v pgweb >/dev/null 2>&1; then
    echo "pgweb"
    return 0
  fi
  if command -v nix >/dev/null 2>&1 && [[ -f "$ROOT/flake.nix" ]]; then
    echo "nix"
    return 0
  fi
  return 1
}

start_pgweb() {
  if [[ "$NO_DB_UI" == "1" ]]; then
    return
  fi
  local url port mode
  port="${ARIADNE_PGWEB_PORT:-8081}"
  url="$(pgweb_database_url)"
  if ! mode="$(pgweb_cmd)"; then
    echo "Could not start pgweb (install nix and use this repo's flake, or run: nix develop)" >&2
    return
  fi
  echo "Starting pgweb → open http://127.0.0.1:${port}/"
  if [[ "$mode" == "pgweb" ]]; then
    pgweb --url="$url" --bind=127.0.0.1 --listen="$port" >>"$ROOT/.pgweb.log" 2>&1 &
    PGWEB_PID=$!
  else
    echo "(using nix develop for pgweb; see .pgweb.log if the page does not load)"
    nix develop "$ROOT" --command pgweb \
      --url="$url" --bind=127.0.0.1 --listen="$port" >>"$ROOT/.pgweb.log" 2>&1 &
    PGWEB_PID=$!
  fi
  sleep 1
  if ! kill -0 "$PGWEB_PID" 2>/dev/null; then
    echo "pgweb exited immediately. Last log lines:" >&2
    tail -n 15 "$ROOT/.pgweb.log" >&2 || true
    PGWEB_PID=""
  fi
}

stop_pgweb() {
  if [[ -n "${PGWEB_PID:-}" ]] && kill -0 "$PGWEB_PID" 2>/dev/null; then
    kill "$PGWEB_PID" 2>/dev/null || true
    wait "$PGWEB_PID" 2>/dev/null || true
  fi
  pkill -f '[p]gweb.*127\.0\.0\.1' 2>/dev/null || true
}

stop_ingest_server() {
  pkill -f '[t]arget/(debug|release)/ariadne-data-collection' 2>/dev/null || true
}

stop_postgres() {
  if [[ -d "$PGDATA" ]] && postgres_using_local_data_dir; then
    if pg_ctl -D "$PGDATA" status >/dev/null 2>&1; then
      echo "Stopping Postgres (PGDATA=$PGDATA)"
      pg_ctl -D "$PGDATA" stop -m fast || true
    fi
  fi
}

cleanup() {
  if [[ "$SHUTTING_DOWN" == "1" ]]; then
    return
  fi
  SHUTTING_DOWN=1
  stop_ingest_server
  stop_pgweb
  if [[ "${STARTED_PG:-0}" == "1" ]]; then
    stop_postgres
  fi
}

need_cmd cargo
need_cmd openssl
need_cmd initdb
need_cmd pg_ctl
need_cmd psql
need_cmd pg_isready

ensure_env_file
export DATABASE_URL="${DATABASE_URL:-postgres://${PGUSER}:ariadne@${PGHOST}:${PGPORT}/${PGDATABASE}}"
ensure_dev_secrets
ensure_tls_certs

if [[ "$STOP_ONLY" == 1 ]]; then
  echo "Stopping ingest server, pgweb, and local Postgres (if any)..."
  stop_ingest_server
  stop_pgweb
  stop_postgres
  exit 0
fi

if postgres_using_local_data_dir; then
  start_postgres
else
  echo "Using external DATABASE_URL (not managing ./.postgres)"
fi

if ! pg_isready -h "$PGHOST" -p "$PGPORT" >/dev/null 2>&1; then
  echo "Database not reachable at $PGHOST:$PGPORT" >&2
  exit 1
fi

export RUST_LOG="${RUST_LOG:-ariadne_data_collection=debug,tower_http=info,sqlx=warn}"

trap cleanup INT TERM EXIT

CARGO_ARGS=(run)
[[ "$RELEASE" == 1 ]] && CARGO_ARGS=(run --release)

echo "DATABASE_URL=$DATABASE_URL"
echo "HTTPS=${ARIADNE_HTTPS_ADDR:-0.0.0.0:8443}  HTTP redirect port=${ARIADNE_HTTP_REDIRECT_PORT:-8080}"
start_pgweb
echo "Building and starting ingest server... (Ctrl+C or ./run_server.sh --stop to shut down)"
echo

# Foreground so Ctrl+C stops cargo and the Rust binary; cleanup stops pgweb + local Postgres.
cargo "${CARGO_ARGS[@]}"
