#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

PGHOST="${PGHOST:-127.0.0.1}"
PGPORT="${PGPORT:-5433}"
PGUSER="${PGUSER:-ariadne}"
PGDATABASE="${PGDATABASE:-ariadne_metrics}"
PGPASSWORD="${PGPASSWORD:-ariadne}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT}"

mkdir -p "$OUTPUT_DIR"

next_dump_path() {
  local base="$1"
  local name="${base%.*}"
  local ext="${base##*.}"
  local candidate="$base"
  local n=1

  while [[ -e "$candidate" ]]; do
    if [[ "$base" == *.* ]]; then
      candidate="${name} (${n}).${ext}"
    else
      candidate="${base} (${n})"
    fi
    ((n += 1))
  done

  printf '%s\n' "$candidate"
}

TARGET="$(next_dump_path "$OUTPUT_DIR/dump.txt")"

PGPASSWORD="$PGPASSWORD" psql \
  -h "$PGHOST" \
  -p "$PGPORT" \
  -U "$PGUSER" \
  -d "$PGDATABASE" \
  -At \
  -c "SELECT id, github_username, forwarded_at, payload::text FROM session_forwards ORDER BY id ASC;" \
  > "$TARGET"

echo "Dumped session_forwards to: $TARGET"
