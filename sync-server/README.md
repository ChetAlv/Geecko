# Geecko Sync Server

Servicio central de sincronización para Geecko. Las aplicaciones Windows y Android continúan trabajando con SQLite y envían cambios a este servicio; ninguna aplicación cliente recibe credenciales directas de PostgreSQL.

## Requisitos

- PostgreSQL 18 o posterior.
- Una base y un usuario propios para Geecko. No se debe ejecutar el servicio con el usuario `postgres`.
- HTTPS delante del servicio cuando se acceda desde otra red. El proceso escucha solamente en `127.0.0.1:8787` de forma predeterminada para facilitar el uso de un proxy inverso.

## Producción en Oracle Linux

El paquete incluye compilación release, unidad `systemd` endurecida, instalación idempotente, proxy Nginx y una guía completa para Oracle Linux 8/9:

- [`deploy/oracle-linux/README.md`](deploy/oracle-linux/README.md)
- `deploy/oracle-linux/build-release.sh`
- `deploy/oracle-linux/install.sh`

Compila directamente en el servidor para soportar automáticamente instancias Oracle Cloud `x86_64` y Ampere `aarch64`.

## Configuración inicial de PostgreSQL

Ejecuta como administrador de PostgreSQL y sustituye la contraseña:

```sql
CREATE ROLE geecko_sync LOGIN PASSWORD 'UNA_CONTRASEÑA_LARGA_Y_UNICA';
CREATE DATABASE geecko OWNER geecko_sync;
```

En PowerShell configura la conexión solamente para la sesión actual:

```powershell
$env:DATABASE_URL='postgresql://geecko_sync:CONTRASEÑA@SERVIDOR:5432/geecko'
```

Como alternativa, copia `.env.example` como `.env` dentro de la carpeta principal del proyecto y completa sus valores. El archivo `.env` está excluido del control de versiones.

Al iniciar el servicio se comprueba que PostgreSQL sea versión 18 o posterior y se aplican automáticamente las migraciones de `migrations/`.

```powershell
cargo run --manifest-path sync-server/Cargo.toml --bin geecko-sync-server
```

Comprobación local:

```text
GET http://127.0.0.1:8787/health
```

## Registrar un dispositivo

Con `DATABASE_URL` configurada:

```powershell
cargo run --manifest-path sync-server/Cargo.toml --bin geecko-sync-admin -- create-device "Geecko" "PC principal" windows
cargo run --manifest-path sync-server/Cargo.toml --bin geecko-sync-admin -- create-device "Geecko" "Teléfono Ricardo" android
```

El comando devuelve el UUID, prefijo y token. El token se muestra una sola vez y la base guarda solamente su hash SHA-256.

En cada aplicación inicia sesión como administrador y abre **Usuarios → Sincronización con PostgreSQL**. Introduce:

- la URL pública HTTPS del servicio, por ejemplo `https://sync.geecko.example`;
- el UUID de ese dispositivo;
- su prefijo (`PC01`, `M01`, etc.);
- el token mostrado por el comando.

No reutilices el registro de Windows en Android: cada instalación necesita su propio UUID, prefijo y token. Los prefijos se añaden a los nuevos consecutivos para impedir que ambos dispositivos creen el mismo número mientras trabajan simultáneamente.

## API inicial

- `GET /health`: versión del protocolo y de PostgreSQL.
- `POST /v1/sync/push`: lote idempotente de hasta 250 cambios.
- `GET /v1/sync/pull?cursor=0&limit=250`: cambios incrementales posteriores al cursor.

Los endpoints de sincronización requieren `Authorization: Bearer <token>` y comprueban que el `deviceId` del lote corresponda al token.

Los clientes envían su cola al guardar y realizan un ciclo automático cada cinco segundos. Cada ciclo primero hace `push` y luego `pull`; por ello los cambios suelen aparecer en el otro dispositivo en menos de diez segundos. Si se corta la red, SQLite continúa disponible y el siguiente ciclo reintenta la cola.

## Seguridad de red

No publiques el puerto HTTP directamente en internet. Para acceso externo coloca Caddy, Nginx u otro proxy TLS delante del servicio, permite sólo HTTPS y conserva PostgreSQL en una red privada sin exponer el puerto 5432 a los teléfonos.

En una instalación con proxy inverso conserva:

```dotenv
GEECKO_SYNC_BIND=127.0.0.1:8787
```

El proxy público recibe HTTPS y reenvía internamente a `http://127.0.0.1:8787`. Sólo para pruebas dentro de una LAN privada puede usarse `GEECKO_SYNC_BIND=0.0.0.0:8787` y una URL `http://IP-PRIVADA:8787`; la aplicación rechaza HTTP hacia direcciones públicas.
