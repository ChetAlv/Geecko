# Geecko Maintenance

Aplicación de escritorio local para registrar, programar y dar seguimiento a órdenes de mantenimiento. La interfaz está construida con HTML, CSS y JavaScript; Tauri 2 la empaqueta como programa nativo y Rust administra la lógica y SQLite.

## Estado de la migración

- La interfaz existente se conserva en `public/`.
- Las llamadas HTTP fueron reemplazadas por comandos IPC de Tauri.
- El nuevo núcleo se encuentra en `src-tauri/src/`.
- SQLite se compila dentro de la aplicación mediante `rusqlite`.
- La base se guarda en el directorio de datos de la aplicación del usuario.
- Las copias SQLite restaurables y las exportaciones JSON se crean en `Documentos/Geecko Backups`.
- El instalador configurado es NSIS para el usuario actual y no requiere privilegios de administrador.
- No se configuró firma digital, conforme al uso personal previsto.

Los archivos anteriores de `src/` y `test/` se conservan temporalmente como referencia del prototipo Node; ya no son usados por los comandos `npm start` o `npm run build`.

## Requisitos de desarrollo en Windows

Node.js y el CLI local de Tauri ya están instalados en este proyecto. WebView2 versión 155 también está presente en este equipo. Sólo falta instalar:

1. **Microsoft C++ Build Tools**  
   Descarga oficial directa: <https://aka.ms/vs/17/release/vs_BuildTools.exe>  
   Página oficial: <https://visualstudio.microsoft.com/visual-cpp-build-tools/>

   En el instalador selecciona la carga de trabajo **Desarrollo para el escritorio con C++**. Debe incluir MSVC y Windows SDK.

2. **Rust mediante rustup para Windows x64**  
   Instalador oficial directo: <https://win.rustup.rs/x86_64>  
   Página oficial: <https://www.rust-lang.org/tools/install>

   Conserva la opción predeterminada `x86_64-pc-windows-msvc`.

3. **WebView2 Runtime** — no hace falta instalarlo en este equipo.  
   Si en el futuro se instala Geecko en otra computadora sin WebView2, puede descargarse el instalador Evergreen Standalone x64 desde: <https://developer.microsoft.com/en-us/microsoft-edge/webview2/>

Después de instalar Build Tools y Rust, reinicia la terminal y comprueba:

```powershell
rustc --version
cargo --version
```

## Desarrollo

Abre la aplicación de escritorio en modo desarrollo:

```powershell
npm start
```

También puede usarse:

```powershell
npm run dev
```

La interfaz ya no debe abrirse directamente como archivo HTML ni mediante `http://127.0.0.1:4177`; ahora se ejecuta dentro de la ventana nativa de Tauri.

## Aplicación Android

La base Android reutiliza la interfaz, los comandos Rust y SQLite del programa de escritorio. En teléfonos muestra navegación inferior, listas en tarjetas y formularios de pantalla completa. Los datos se guardan primero en el almacenamiento privado de la aplicación y, cuando se configura el servidor, se replican automáticamente en PostgreSQL.

Ejecutar en el dispositivo conectado para ver cambios durante el desarrollo:

```powershell
npm run android:dev
```

Este comando crea primero una redirección USB con ADB y hace que la aplicación cargue `127.0.0.1:1430` a través del cable. Así el modo de desarrollo no depende de la IP local ni de una regla de entrada en el firewall. Mantén el dispositivo conectado y con la depuración USB autorizada.

Si deliberadamente quieres desarrollar por Wi-Fi, usa `npm run android:dev:wifi`; Tauri pedirá la IP de esta computadora y ambos dispositivos deberán estar en la misma red sin aislamiento entre clientes.

Crear un APK para dispositivos Android ARM64:

```powershell
npm run android:build
```

El APK se genera dentro de `src-tauri/gen/android/app/build/outputs/apk/`. Windows y Android conservan bases SQLite independientes para seguir funcionando ante una interrupción, pero comparten clientes, equipos, órdenes, inventario, compras y ventas mediante el servidor central.

## Núcleo de sincronización con PostgreSQL 18

La preparación para sincronizar Windows y Android se encuentra en tres partes:

- `sync-protocol/`: contrato compartido de cambios, resultados, conflictos y cursores.
- `sync-server/`: API Rust central y migraciones para PostgreSQL 18.
- `src-tauri/src/sync.rs`: identidad local, UUID, revisiones, conflictos y cola SQLite de cambios pendientes.

Los clientes no incorporan credenciales ni se conectan al puerto 5432. Cada instalación conserva SQLite y se comunica por HTTPS con `sync-server`; ese servicio es el único que abre conexiones a PostgreSQL. Así un token robado puede revocarse sin entregar control de la base completa. La base local está en versión 15 y asigna identidades UUID estables a los registros sincronizados sin cambiar sus IDs locales.

La aplicación envía cambios después de guardar y consulta novedades cada cinco segundos mientras está visible. También sincroniza al iniciar sesión, recuperar conectividad, volver al primer plano o pulsar **Sincronizar ahora**. Las órdenes se transfieren junto con tareas, accesorios, cotización, recursos, pagos, garantía y fotografías. Los cambios pendientes permanecen en una cola local hasta que el servidor confirma su recepción.

La instalación del servicio y el registro de Windows y Android están documentados en [`sync-server/README.md`](sync-server/README.md). Un administrador pega en **Usuarios → Sincronización con PostgreSQL** la URL HTTPS, UUID, prefijo y token generados para cada dispositivo.

## Crear el instalador

```powershell
npm run build
```

El instalador resultante se crea en:

```text
src-tauri/target/release/bundle/nsis/
```

La configuración usa el WebView2 ya instalado y por eso no necesita descargarlo durante la instalación. Si la aplicación se distribuye a otros equipos, se puede cambiar `webviewInstallMode` en `src-tauri/tauri.conf.json` a `offlineInstaller`; esto incorpora el instalador completo de WebView2, pero aumenta considerablemente el tamaño del paquete.

## Funciones actuales

- Directorio de clientes con contacto, empresa, NIT, dirección e historial de órdenes y equipos.
- Alta de clientes durante la recepción y reutilización de clientes registrados.
- Edición completa de órdenes, incluidos cliente, equipo, seguimiento, accesorios y tareas.
- Flujo de estados validado con requisitos para finalizar y entregar una reparación.
- Diagnóstico técnico y cotización por conceptos de mano de obra, repuestos u otros cargos.
- Autorización o rechazo local por firma, atención presencial, teléfono, WhatsApp o correo, con historial y responsable.
- Cotización imprimible en hoja carta con datos del cliente, equipo, totales y espacio para firmas.
- Repuestos utilizados vinculados con el inventario y descontados de existencias al guardar la orden.
- Mano de obra real por técnico, actividad, horas y tarifa, con totales por orden.
- Corrección transaccional de consumos: al editar o retirar un repuesto, su existencia se devuelve automáticamente.
- Inventario local de repuestos, consumibles y accesorios con existencias y nivel mínimo.
- Reportes de compra con selección de productos, cantidades, costos unitarios, total e historial reimprimible.
- Los productos, consumos de órdenes, mano de obra y reportes de compra se incluyen en el respaldo JSON local.
- Dashboard con órdenes activas, críticas, atrasadas y completadas.
- Estados: recibida, en diagnóstico, pendiente de autorización, en reparación, en espera, lista para entregar, entregada y cancelada.
- Catálogo de equipos y relación de órdenes por activo.
- Responsable, solicitante, ubicación, fecha y horas estimadas.
- Lista de tareas, notas de seguridad e historial.
- Consecutivo automático `GM-AÑO-0001`.
- Búsqueda y filtros.
- Impresión de órdenes.
- Centro local de respaldos y restauración.
- Funcionamiento sin servicios de internet.

## Inventario, compras y ventas

- Los reportes de compra planifican cantidades y costos sin alterar existencias.
- Las recepciones pueden ser parciales o completas; sólo lo recibido aumenta el inventario.
- El kardex registra entradas, salidas, saldo posterior, origen y referencia de cada movimiento.
- Las ventas directas descuentan accesorios del inventario sin crear una orden de servicio.
- Al anular una venta se restituyen las existencias y se conserva el motivo.
- El comprobante de venta es interno y no constituye factura fiscal ni FEL.
- Movimientos, recepciones y ventas se incluyen en el respaldo JSON local.

## Anticipos, pagos y recibos

- Cada orden mantiene su total de referencia, pagos acumulados, saldo pendiente y saldo a favor.
- Se pueden registrar anticipos antes de definir el total final del servicio.
- Los cobros aceptan efectivo, tarjeta, transferencia, depósito bancario u otra forma de pago.
- Cada pago genera un recibo consecutivo `RP-AÑO-0001`, reimprimible en hoja carta.
- El recibo conserva el total y los saldos existentes al momento de emitirlo.
- Los pagos pueden anularse con un motivo; no se borran del historial y el saldo se recalcula.
- La constancia de entrega incluye el estado de cuenta de la orden.
- Los pagos forman parte del respaldo JSON local.

## Garantías y órdenes de regreso

- Las órdenes finalizadas o entregadas pueden recibir una garantía con vigencia, cobertura y exclusiones propias.
- Cada garantía genera un consecutivo `GT-AÑO-0001` y sus condiciones aparecen en la constancia de entrega.
- Un regreso reutiliza el cliente, equipo, ubicación y garantía de la orden original, pero abre una nueva orden de trabajo para conservar ambos historiales.
- Cada regreso genera un consecutivo `RG-AÑO-0001` y registra falla reportada, condición de recepción, accesorios y si ingresó dentro de la vigencia.
- La evaluación técnica determina si el regreso queda cubierto, no cubierto o pendiente; estar dentro del plazo no concede cobertura automáticamente.
- Los repuestos y la mano de obra de un regreso cubierto se conservan para control interno, mientras el total a cobrar al cliente queda en `Q0.00`.
- Garantías y regresos se incluyen en el respaldo JSON local (versión 13).

## Restauración y respaldos automáticos

- Geecko crea una instantánea SQLite automática por día de uso, sin depender de internet.
- Se conservan las 30 copias automáticas más recientes; los respaldos manuales no se eliminan mediante esa rotación.
- El centro de respaldos muestra fecha, tipo, tamaño, estado y cantidades de órdenes, clientes y productos de cada copia.
- Los respaldos se generan con `VACUUM INTO`, por lo que incluyen un estado consistente de la base aunque SQLite utilice el modo WAL.
- Antes de restaurar se verifica la estructura, la integridad de SQLite, las relaciones y la compatibilidad de versión.
- La restauración se prepara y migra en un archivo temporal antes de reemplazar la base activa.
- Siempre se crea una copia `geecko-pre-restauracion-...db` de los datos actuales; se conservan las 10 más recientes.
- La exportación JSON continúa disponible como formato legible para archivo o revisión, pero la restauración utiliza copias `.db` completas.

## Fotografías, QR, estadísticas y usuarios

- Cada orden admite evidencia JPEG, PNG o WebP clasificada como recepción, diagnóstico, reparación o entrega.
- Las imágenes se reducen antes de guardarse y quedan dentro de SQLite; por ello las copias restaurables `.db` incluyen también la evidencia fotográfica.
- Cada orden genera localmente un código QR con su número, visible en el detalle, en las cabeceras de documentos y en una etiqueta imprimible.
- El panel de estadísticas permite revisar 30, 90 o 365 días, o todo el historial: órdenes, ingresos registrados, tiempo promedio, garantías, estados, tendencia mensual y mano de obra por técnico.
- En el primer inicio se crea una cuenta administradora. Las contraseñas se almacenan como hashes Argon2, nunca como texto visible.
- Los perfiles locales son administrador, recepción y técnico. Los permisos se validan en la interfaz y nuevamente en los comandos Rust.
- El administrador puede crear, activar o desactivar cuentas, restablecer claves y consultar actividad reciente.
- El modo multiusuario esta pensado para varias cuentas que comparten esta misma instalacion y base local. No habilita acceso simultaneo desde otras computadoras ni requiere un servidor.
- Las cuentas, actividad y fotografías están incluidas en los respaldos SQLite. La exportación JSON es un resumen legible y no contiene contraseñas ni los bytes de las fotografías.

## Arquitectura

```text
Interfaz HTML/CSS/JavaScript
            │ IPC de Tauri
            ▼
      Comandos en Rust
            │                 HTTPS                 PostgreSQL 18
            ├── SQLite local ───────► API Rust ──────────┘
            └── cola y cursor ◄──── cambios incrementales
```

Los comandos de Rust están declarados en `src-tauri/src/lib.rs`, la persistencia en `src-tauri/src/database.rs` y las validaciones/tipos en `src-tauri/src/domain.rs`.

## Fuentes oficiales

- [Requisitos de Tauri para Windows](https://v2.tauri.app/start/prerequisites/)
- [Instalación oficial de Rust](https://www.rust-lang.org/tools/install/)
- [Microsoft C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
- [Microsoft Edge WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)
- [Instaladores de Windows con Tauri](https://v2.tauri.app/distribute/windows-installer/)
