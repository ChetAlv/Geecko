#!/usr/bin/env bash
set -Eeuo pipefail

readonly ENV_FILE="/etc/geecko-sync/geecko-sync.env"
readonly ADMIN_BINARY="/usr/local/libexec/geecko-sync/geecko-sync-admin"

if [[ ! -r "$ENV_FILE" ]]; then
  echo "No se puede leer $ENV_FILE. Ejecuta este comando con sudo." >&2
  exit 1
fi

# El archivo es propiedad de root y sólo contiene la configuración del servicio.
set -a
# shellcheck disable=SC1090
source "$ENV_FILE"
set +a

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "DATABASE_URL no está configurada en $ENV_FILE." >&2
  exit 1
fi

exec /usr/sbin/runuser -u geecko-sync -- \
  /usr/bin/env -i \
  HOME=/var/lib/geecko-sync \
  PATH=/usr/bin:/bin \
  DATABASE_URL="$DATABASE_URL" \
  "$ADMIN_BINARY" "$@"
