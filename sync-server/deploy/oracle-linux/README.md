# Despliegue de Geecko Sync en Oracle Linux

Esta carpeta prepara la API de sincronización como un servicio `systemd` detrás de Nginx. Está pensada para Oracle Linux 8 o 9, tanto `x86_64` como `aarch64`. La compilación se hace dentro del servidor para producir binarios compatibles con su arquitectura y versión de glibc.

## Arquitectura final

```text
Windows / Android
       │ HTTPS :443
       ▼
Nginx en Oracle Linux
       │ HTTP 127.0.0.1:8787
       ▼
geecko-sync-server
       │ conexión PostgreSQL privada
       ▼
PostgreSQL 18
```

No abras a internet los puertos `8787` ni `5432`. Los únicos puertos públicos de la API deben ser `80` y `443`; el puerto `80` se conserva para redirección HTTPS y renovación del certificado.

## 1. Requisitos previos

- Un dominio o subdominio, por ejemplo `sync.tudominio.com`, cuyo registro DNS `A` apunte a la IP pública del servidor. Añade también `AAAA` solamente si IPv6 está configurado realmente.
- PostgreSQL 18 accesible desde Oracle Linux.
- El repositorio completo de Geecko en el servidor. `sync-server` necesita el paquete hermano `sync-protocol` durante la compilación.
- Acceso SSH con un usuario que pueda ejecutar `sudo`.

Si la instancia está en Oracle Cloud, agrega reglas de ingreso TCP para los puertos `80` y `443` en el NSG o la lista de seguridad de la subred. La regla del puerto `22` debe limitarse a las direcciones desde las que administras el servidor siempre que sea posible.

## 2. Instalar herramientas

```bash
sudo dnf install -y gcc make curl nginx policycoreutils-python-utils python3 python3-pip
sudo systemctl enable --now firewalld
```

Instala Rust estable con `rustup` usando tu usuario normal, no `root`:

```bash
curl --proto '=https' --tlsv1.2 https://sh.rustup.rs -sSf | sh -s -- -y
source "$HOME/.cargo/env"
rustc --version
cargo --version
```

Referencias oficiales:

- [Instalar Rust](https://www.rust-lang.org/tools/install)
- [Nginx en Oracle Linux](https://docs.oracle.com/en/learn/ol-nginx/)
- [Configurar firewalld](https://docs.oracle.com/en/operating-systems/oracle-linux/9/firewall/)

## 3. Compilar e instalar

Desde la raíz de `Geecko-Dash`, compila sin `sudo`:

```bash
bash sync-server/deploy/oracle-linux/build-release.sh
```

Instala los binarios y plantillas indicando el dominio real:

```bash
sudo bash sync-server/deploy/oracle-linux/install.sh sync.tudominio.com
```

El instalador crea:

- `/usr/local/libexec/geecko-sync/`: API y herramienta administrativa.
- `/etc/geecko-sync/geecko-sync.env`: conexión privada a PostgreSQL.
- `/etc/systemd/system/geecko-sync.service`: servicio con reinicio automático y endurecimiento.
- `/etc/nginx/conf.d/geecko-sync.conf`: proxy hacia `127.0.0.1:8787`.
- `/usr/local/sbin/geecko-sync-admin`: comando administrativo que carga la conexión protegida.

La instalación es idempotente: al repetirla actualiza los binarios y la unidad, pero conserva el archivo de entorno y la configuración de Nginx para no borrar los cambios TLS de Certbot.

## 4. Configurar PostgreSQL

Edita el archivo protegido:

```bash
sudoedit /etc/geecko-sync/geecko-sync.env
```

PostgreSQL en el mismo servidor:

```dotenv
DATABASE_URL='postgresql://geecko_sync:CONTRASENA_CODIFICADA@127.0.0.1:5432/geecko'
GEECKO_SYNC_BIND=127.0.0.1:8787
RUST_BACKTRACE=0
```

PostgreSQL en otro servidor:

```dotenv
DATABASE_URL='postgresql://geecko_sync:CONTRASENA_CODIFICADA@HOST_PRIVADO:5432/geecko?sslmode=require'
GEECKO_SYNC_BIND=127.0.0.1:8787
RUST_BACKTRACE=0
```

Usa el modo TLS y los parámetros de certificado que exija tu proveedor. Si la contraseña contiene caracteres reservados como `@`, `:`, `/`, `#`, `?` o `%`, deben codificarse como parte de una URL. El archivo debe continuar con propietario `root`, grupo `geecko-sync` y modo `0640`; el instalador corrige estos permisos al ejecutarse nuevamente.

Inicia la API:

```bash
sudo systemctl enable --now geecko-sync
sudo systemctl status geecko-sync --no-pager
curl --fail --silent --show-error http://127.0.0.1:8787/health
```

La primera ejecución verifica PostgreSQL 18 y aplica las migraciones incluidas en el binario.

## 5. Habilitar Nginx, SELinux y el firewall

Permite que Nginx actúe como proxy con SELinux en modo enforcing:

```bash
sudo setsebool -P httpd_can_network_relay 1
```

Valida e inicia Nginx:

```bash
sudo nginx -t
sudo systemctl enable --now nginx
```

Abre únicamente HTTP y HTTPS en `firewalld`:

```bash
sudo firewall-cmd --permanent --add-service=http --add-service=https
sudo firewall-cmd --reload
sudo firewall-cmd --list-all
```

El puerto `8787` no debe aparecer entre los puertos públicos porque la API escucha solamente en loopback.

## 6. Emitir el certificado HTTPS

Puedes instalar Certbot en un entorno virtual aislado:

```bash
sudo python3 -m venv /opt/certbot
sudo /opt/certbot/bin/pip install --upgrade pip
sudo /opt/certbot/bin/pip install certbot certbot-nginx
sudo ln -sf /opt/certbot/bin/certbot /usr/local/bin/certbot
sudo certbot --nginx --redirect -d sync.tudominio.com
```

Sigue las indicaciones de correo y términos de Let's Encrypt. Consulta y configura la renovación según las [instrucciones oficiales de Certbot para Nginx](https://certbot.eff.org/instructions?ws=nginx&os=pip). Antes de continuar, esta dirección debe responder sin advertencias de certificado:

```bash
curl --fail --silent --show-error https://sync.tudominio.com/health
```

## 7. Registrar Windows y Android

Genera credenciales diferentes para cada instalación:

```bash
sudo geecko-sync-admin create-device "Geecko" "PC principal" windows
sudo geecko-sync-admin create-device "Geecko" "Teléfono Ricardo" android
```

Guarda inmediatamente cada resultado. El token se muestra una sola vez. En ambas aplicaciones la dirección será la misma:

```text
https://sync.tudominio.com
```

Pero cada una debe usar su propio `Dispositivo`, `Prefijo` y `Token`.

## Operación habitual

Estado y registros:

```bash
sudo systemctl status geecko-sync --no-pager
sudo journalctl -u geecko-sync -n 100 --no-pager
sudo journalctl -u geecko-sync -f
```

Comprobaciones:

```bash
curl --fail http://127.0.0.1:8787/health
curl --fail https://sync.tudominio.com/health
sudo nginx -t
```

Reiniciar:

```bash
sudo systemctl restart geecko-sync
```

Actualizar después de copiar una nueva versión del proyecto:

```bash
bash sync-server/deploy/oracle-linux/build-release.sh
sudo bash sync-server/deploy/oracle-linux/install.sh sync.tudominio.com
```

## Diagnóstico rápido

- `status=203/EXEC`: el binario no existe, no tiene permiso de ejecución o fue compilado para otra arquitectura. Compílalo dentro del propio servidor.
- `connection refused` en `127.0.0.1:8787`: revisa `journalctl -u geecko-sync`; normalmente es una `DATABASE_URL` incorrecta o PostgreSQL inaccesible.
- Nginx devuelve `502 Bad Gateway`: la API no está activa o SELinux todavía no permite el proxy.
- Desde internet no responde, pero localmente sí: revisa tanto `firewalld` como el NSG/lista de seguridad de Oracle Cloud.
- La aplicación rechaza la configuración: confirma HTTPS, ejecuta `/health` y comprueba que el UUID y token pertenezcan al mismo dispositivo.
