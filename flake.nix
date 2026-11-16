{
  description = "Ariadne session metrics HTTPS ingest server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        toolchain = pkgs.rust-bin.stable.latest.default;
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            toolchain
            pkg-config
            openssl
            postgresql_16
            sqlx-cli
            mkcert
            pgweb
          ];

          shellHook = ''
            export PGDATA="''${PGDATA:-$PWD/.postgres}"
            export PGPORT="''${PGPORT:-5432}"
            export DATABASE_URL="''${DATABASE_URL:-postgres://ariadne:ariadne@127.0.0.1:''${PGPORT}/ariadne_metrics}"

            if [ ! -d "$PGDATA" ]; then
              initdb -D "$PGDATA" --auth=trust
              mkdir -p "$PGDATA/sockets"
              {
                echo "listen_addresses = '127.0.0.1'"
                echo "port = $PGPORT"
                echo "unix_socket_directories = '$PGDATA/sockets'"
              } >> "$PGDATA/postgresql.conf"
              pg_ctl -D "$PGDATA" -l "$PGDATA/server.log" start
            elif ! grep -q '^unix_socket_directories' "$PGDATA/postgresql.conf" 2>/dev/null; then
              mkdir -p "$PGDATA/sockets"
              {
                echo ""
                echo "unix_socket_directories = '$PGDATA/sockets'"
              } >> "$PGDATA/postgresql.conf"
              createuser -s ariadne || true
              psql -v ON_ERROR_STOP=1 -c "ALTER USER ariadne WITH PASSWORD 'ariadne';" || true
              createdb -O ariadne ariadne_metrics || true
            elif ! pg_isready -h 127.0.0.1 -p "$PGPORT" >/dev/null 2>&1; then
              pg_ctl -D "$PGDATA" -l "$PGDATA/server.log" start
            fi

            if [ ! -f .env ] && [ -f .env.example ]; then
              cp .env.example .env
              echo "Created .env from .env.example — set API key, pepper, and TLS paths."
            fi

            if [ ! -f certs/cert.pem ]; then
              mkdir -p certs
              echo "Generating dev TLS cert in ./certs (self-signed)."
              openssl req -x509 -newkey rsa:2048 -nodes \
                -keyout certs/key.pem -out certs/cert.pem -days 825 \
                -subj "/CN=localhost" 2>/dev/null
            fi

            echo "Postgres: $DATABASE_URL"
            echo "Run: ./run_server.sh  (ingest + pgweb at http://127.0.0.1:8081)"
          '';
        };

      });
}
