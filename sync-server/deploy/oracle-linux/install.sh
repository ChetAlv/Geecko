#!/usr/bin/env bash
set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SYNC_SERVER_DIR="$(cd -- "$SCRIPT_DIR/../.." && pwd)"
readonly RELEASE_DIR="$SYNC_SERVER_DIR/target/release"
readonly SERVICE_USER="geecko-sync"
readonly SERVICE_GROUP="geecko-sync"
readonly APP_DIR="/usr/local/libexec/geecko-sync"
readonly CONFIG_DIR="/etc/geecko-sync"
readonly STATE_DIR="/var/lib/geecko-sync"
readonly DOC_DIR="/usr/local/share/doc/geecko-sync"
readonly SYSTEMD_FILE="/etc/systemd/system/geecko-sync.service"
readonly NGINX_FILE="/etc/nginx/conf.d/geecko-sync.conf"

usage() {
  echo "Uso: sudo bash install.sh <dominio>" >&2
  echo "Ejemplo: sudo bash install.sh sync.geecko.com" >&2
}

valid_domain() {
  local domain="$1"
  local label
  local -a labels

  [[ ${#domain} -le 253 && "$domain" == *.* ]] || return 1
  IFS='.' read -r -a labels <<< "$domain"
  for label in "${labels[@]}"; do
    [[ "$label" =~ ^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$ ]] || return 1
  done
}

if [[ $EUID -ne 0 ]]; then
  echo "El instalador debe ejecutarse con sudo." >&2
  exit 1
fi

if [[ $# -ne 1 ]]; then
  usage
  exit 2
fi

readonly DOMAIN="${1,,}"
if ! valid_domain "$DOMAIN"; then
  echo "El dominio '$DOMAIN' no es válido." >&2
  exit 2
fi

if [[ ! -r /etc/os-release ]]; then
  echo "No fue posible identificar la distribución Linux." >&2
  exit 1
fi

# shellcheck disable=SC1091
source /etc/os-release
if [[ "${ID:-}" != "ol" && " ${ID_LIKE:-} " != *" rhel "* ]]; then
  echo "Aviso: este instalador fue preparado para Oracle Linux 8/9; se detectó ${PRETTY_NAME:-otra distribución}." >&2
fi

for binary in geecko-sync-server geecko-sync-admin; do
  if [[ ! -x "$RELEASE_DIR/$binary" ]]; then
    echo "Falta $RELEASE_DIR/$binary. Ejecuta build-release.sh sin sudo primero." >&2
    exit 1
  fi
done

if [[ ! -d /etc/nginx/conf.d ]]; then
  echo "No se encontró Nginx. Instálalo primero con: sudo dnf install -y nginx" >&2
  exit 1
fi

if ! getent group "$SERVICE_GROUP" >/dev/null; then
  groupadd --system "$SERVICE_GROUP"
fi
if ! id -u "$SERVICE_USER" >/dev/null 2>&1; then
  useradd \
    --system \
    --gid "$SERVICE_GROUP" \
    --home-dir "$STATE_DIR" \
    --shell /sbin/nologin \
    --no-create-home \
    "$SERVICE_USER"
fi

install -d -m 0755 -o root -g root "$APP_DIR" "$DOC_DIR"
install -d -m 0750 -o root -g "$SERVICE_GROUP" "$CONFIG_DIR"
install -d -m 0750 -o "$SERVICE_USER" -g "$SERVICE_GROUP" "$STATE_DIR"
install -m 0755 -o root -g root "$RELEASE_DIR/geecko-sync-server" "$APP_DIR/geecko-sync-server"
install -m 0755 -o root -g root "$RELEASE_DIR/geecko-sync-admin" "$APP_DIR/geecko-sync-admin"
install -m 0755 -o root -g root "$SCRIPT_DIR/geecko-sync-admin.sh" /usr/local/sbin/geecko-sync-admin
install -m 0644 -o root -g root "$SCRIPT_DIR/geecko-sync.service" "$SYSTEMD_FILE"
install -m 0644 -o root -g root "$SCRIPT_DIR/README.md" "$DOC_DIR/README-oracle-linux.md"

if [[ ! -e "$CONFIG_DIR/geecko-sync.env" ]]; then
  install -m 0640 -o root -g "$SERVICE_GROUP" \
    "$SCRIPT_DIR/geecko-sync.env.example" "$CONFIG_DIR/geecko-sync.env"
  echo "Se creó $CONFIG_DIR/geecko-sync.env; debes colocar allí la conexión real de PostgreSQL."
else
  chown root:"$SERVICE_GROUP" "$CONFIG_DIR/geecko-sync.env"
  chmod 0640 "$CONFIG_DIR/geecko-sync.env"
  echo "Se conservó la configuración existente de $CONFIG_DIR/geecko-sync.env."
fi

if [[ ! -e "$NGINX_FILE" ]]; then
  sed "s/__GEECKO_SYNC_DOMAIN__/$DOMAIN/g" \
    "$SCRIPT_DIR/nginx-geecko-sync.conf" > "$NGINX_FILE"
  chmod 0644 "$NGINX_FILE"
  echo "Se creó la configuración de Nginx para $DOMAIN."
else
  echo "Se conservó $NGINX_FILE para no sobrescribir los cambios TLS de Certbot."
fi

if command -v restorecon >/dev/null 2>&1; then
  restorecon -RF "$APP_DIR" "$CONFIG_DIR" "$STATE_DIR" "$DOC_DIR" \
    /usr/local/sbin/geecko-sync-admin "$SYSTEMD_FILE" "$NGINX_FILE"
fi

systemctl daemon-reload

if grep -q 'CAMBIAR_CONTRASENA' "$CONFIG_DIR/geecko-sync.env"; then
  cat <<EOF

Instalación preparada, pero el servicio no se inició porque falta DATABASE_URL.

1. Edita: sudoedit $CONFIG_DIR/geecko-sync.env
2. Inicia: sudo systemctl enable --now geecko-sync
3. Comprueba: curl --fail http://127.0.0.1:8787/health
4. Continúa con SELinux, Nginx, firewalld y HTTPS según README-oracle-linux.md.
EOF
  exit 0
fi

systemctl enable geecko-sync >/dev/null
if systemctl is-active --quiet geecko-sync; then
  systemctl restart geecko-sync
else
  systemctl start geecko-sync
fi

health_ok=false
for _ in {1..20}; do
  if curl --fail --silent http://127.0.0.1:8787/health >/dev/null 2>&1; then
    health_ok=true
    break
  fi
  sleep 0.5
done
if [[ "$health_ok" != true ]]; then
  echo "Geecko Sync no respondió correctamente en /health." >&2
  systemctl status geecko-sync --no-pager >&2 || true
  journalctl -u geecko-sync -n 30 --no-pager >&2 || true
  exit 1
fi

nginx -t
systemctl enable nginx >/dev/null
if systemctl is-active --quiet nginx; then
  systemctl reload nginx
else
  systemctl start nginx
fi

echo
echo "Geecko Sync fue instalado para $DOMAIN."
echo "Comprueba el servicio con: curl --fail http://127.0.0.1:8787/health"
echo "Completa SELinux, firewalld y HTTPS siguiendo: $DOC_DIR/README-oracle-linux.md"
