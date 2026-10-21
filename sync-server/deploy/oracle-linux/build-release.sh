#!/usr/bin/env bash
set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SYNC_SERVER_DIR="$(cd -- "$SCRIPT_DIR/../.." && pwd)"

if ! command -v cargo >/dev/null 2>&1; then
  echo "No se encontró Cargo. Instala Rust estable con rustup antes de continuar." >&2
  exit 1
fi

echo "Compilando Geecko Sync para $(uname -m)..."
cargo build \
  --release \
  --locked \
  --manifest-path "$SYNC_SERVER_DIR/Cargo.toml" \
  --bins

echo
echo "Binarios preparados en:"
echo "  $SYNC_SERVER_DIR/target/release/geecko-sync-server"
echo "  $SYNC_SERVER_DIR/target/release/geecko-sync-admin"
echo
echo "Siguiente paso:"
echo "  sudo bash $SCRIPT_DIR/install.sh sync.tudominio.com"
