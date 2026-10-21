use crate::domain::CommandError;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use geecko_sync_protocol::{
    EntityKind, MutationStatus, PullResponse, PushRequest, PushResponse, ServerChange,
    SyncMutation, MAX_PUSH_MUTATIONS, PROTOCOL_VERSION,
};
use reqwest::Url;
use rusqlite::{params, params_from_iter, types::ValueRef, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Number, Value};
use std::net::IpAddr;
use uuid::Uuid;

const CORE_ENTITIES: [(&str, &str); 7] = [
    ("customers", "customer"),
    ("assets", "asset"),
    ("inventory_items", "inventory_item"),
    ("work_orders", "work_order"),
    ("purchase_reports", "purchase_report"),
    ("purchase_receipts", "purchase_receipt"),
    ("sales", "sale"),
];

const CUSTOMER_COLUMNS: &[&str] = &[
    "name",
    "phone",
    "email",
    "address",
    "company",
    "tax_id",
    "notes",
    "created_at",
    "updated_at",
];

const ASSET_COLUMNS: &[&str] = &[
    "code",
    "name",
    "category",
    "location",
    "status",
    "manufacturer",
    "model",
    "serial_number",
    "created_at",
    "updated_at",
];

const INVENTORY_COLUMNS: &[&str] = &[
    "code",
    "name",
    "category",
    "quantity",
    "minimum_stock",
    "unit",
    "created_at",
    "updated_at",
];

const PURCHASE_REPORT_COLUMNS: &[&str] = &[
    "report_number",
    "supplier",
    "notes",
    "total_cost_cents",
    "created_at",
];

const PURCHASE_RECEIPT_COLUMNS: &[&str] = &[
    "receipt_number",
    "supplier",
    "notes",
    "total_cost_cents",
    "created_at",
];

const SALE_COLUMNS: &[&str] = &[
    "sale_number",
    "customer_name",
    "notes",
    "subtotal_cents",
    "discount_cents",
    "total_cents",
    "status",
    "cancellation_note",
    "created_at",
    "cancelled_at",
];

const ORDER_COLUMNS: &[&str] = &[
    "order_number",
    "title",
    "description",
    "type",
    "priority",
    "status",
    "location",
    "assigned_to",
    "requested_by",
    "scheduled_date",
    "estimated_hours",
    "safety_notes",
    "observations",
    "work_performed",
    "delivery_condition",
    "delivery_tested_by",
    "diagnosis",
    "recommendation",
    "quote_status",
    "quote_subtotal_cents",
    "quote_discount_cents",
    "quote_total_cents",
    "authorized_by",
    "authorization_method",
    "authorization_note",
    "authorization_at",
    "created_at",
    "updated_at",
    "completed_at",
];

struct ChildSpec {
    key: &'static str,
    table: &'static str,
    parent_column: &'static str,
    columns: &'static [&'static str],
}

const ORDER_CHILDREN: &[ChildSpec] = &[
    ChildSpec {
        key: "tasks",
        table: "work_order_tasks",
        parent_column: "work_order_id",
        columns: &["description", "completed", "completed_at", "position"],
    },
    ChildSpec {
        key: "events",
        table: "work_order_events",
        parent_column: "work_order_id",
        columns: &[
            "action",
            "previous_status",
            "new_status",
            "note",
            "created_at",
        ],
    },
    ChildSpec {
        key: "accessories",
        table: "work_order_accessories",
        parent_column: "work_order_id",
        columns: &["name", "position"],
    },
    ChildSpec {
        key: "quoteItems",
        table: "work_order_quote_items",
        parent_column: "work_order_id",
        columns: &[
            "kind",
            "description",
            "quantity",
            "unit_price_cents",
            "line_total_cents",
            "position",
        ],
    },
    ChildSpec {
        key: "parts",
        table: "work_order_parts",
        parent_column: "work_order_id",
        columns: &[
            "item_code",
            "item_name",
            "unit",
            "quantity",
            "unit_price_cents",
            "line_total_cents",
            "position",
        ],
    },
    ChildSpec {
        key: "labor",
        table: "work_order_labor",
        parent_column: "work_order_id",
        columns: &[
            "technician",
            "description",
            "hours",
            "hourly_rate_cents",
            "line_total_cents",
            "position",
        ],
    },
    ChildSpec {
        key: "payments",
        table: "order_payments",
        parent_column: "work_order_id",
        columns: &[
            "receipt_number",
            "payment_type",
            "method",
            "amount_cents",
            "reference",
            "received_by",
            "note",
            "status",
            "order_total_cents",
            "balance_before_cents",
            "balance_after_cents",
            "created_at",
            "cancelled_at",
            "cancellation_note",
        ],
    },
    ChildSpec {
        key: "warranties",
        table: "order_warranties",
        parent_column: "work_order_id",
        columns: &[
            "warranty_number",
            "duration_days",
            "starts_at",
            "expires_at",
            "coverage",
            "exclusions",
            "status",
            "created_at",
            "updated_at",
        ],
    },
    ChildSpec {
        key: "photos",
        table: "order_photos",
        parent_column: "work_order_id",
        columns: &[
            "file_name",
            "mime_type",
            "category",
            "caption",
            "data",
            "created_by",
            "created_at",
        ],
    },
];

const INVENTORY_CHILDREN: &[ChildSpec] = &[ChildSpec {
    key: "movements",
    table: "inventory_movements",
    parent_column: "inventory_item_id",
    columns: &[
        "item_code",
        "item_name",
        "unit",
        "movement_type",
        "quantity_delta",
        "balance_after",
        "reference_type",
        "reference_number",
        "note",
        "created_at",
    ],
}];

const PURCHASE_REPORT_CHILDREN: &[ChildSpec] = &[ChildSpec {
    key: "lines",
    table: "purchase_report_items",
    parent_column: "purchase_report_id",
    columns: &[
        "item_code",
        "item_name",
        "unit",
        "stock_quantity",
        "purchase_quantity",
        "received_quantity",
        "unit_cost_cents",
        "line_total_cents",
        "position",
    ],
}];

const PURCHASE_RECEIPT_CHILDREN: &[ChildSpec] = &[ChildSpec {
    key: "lines",
    table: "purchase_receipt_items",
    parent_column: "purchase_receipt_id",
    columns: &[
        "item_code",
        "item_name",
        "unit",
        "received_quantity",
        "unit_cost_cents",
        "line_total_cents",
        "position",
    ],
}];

const SALE_CHILDREN: &[ChildSpec] = &[ChildSpec {
    key: "lines",
    table: "sale_items",
    parent_column: "sale_id",
    columns: &[
        "item_code",
        "item_name",
        "unit",
        "quantity",
        "unit_price_cents",
        "line_total_cents",
        "position",
    ],
}];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSyncStatus {
    pub installation_id: String,
    pub configured: bool,
    pub server_url: String,
    pub device_id: String,
    pub device_prefix: String,
    pub token_configured: bool,
    pub pending_changes: i64,
    pub conflicts: i64,
    pub last_server_cursor: i64,
    pub last_synced_at: Option<String>,
    pub last_error: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ConfigureSyncInput {
    pub server_url: String,
    pub device_id: String,
    pub device_prefix: String,
    pub token: String,
}

#[derive(Debug, Clone)]
pub struct RemoteCredentials {
    pub server_url: String,
    pub device_id: Uuid,
    pub device_prefix: String,
    pub token: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunResult {
    pub pushed: usize,
    pub pulled: usize,
    pub remote_changes: usize,
    pub conflicts: usize,
    pub status: LocalSyncStatus,
}

pub fn initialize(connection: &Connection) -> Result<(), CommandError> {
    connection.pragma_update(None, "recursive_triggers", "OFF")?;
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS sync_settings (
          key TEXT PRIMARY KEY,
          value TEXT NOT NULL,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS sync_outbox (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          operation_uuid TEXT NOT NULL UNIQUE,
          entity_type TEXT NOT NULL,
          entity_uuid TEXT NOT NULL,
          local_id INTEGER,
          base_revision INTEGER NOT NULL DEFAULT 0,
          action TEXT NOT NULL DEFAULT 'upsert' CHECK (action IN ('upsert', 'delete')),
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          attempts INTEGER NOT NULL DEFAULT 0,
          last_error TEXT NOT NULL DEFAULT '',
          UNIQUE(entity_type, entity_uuid)
        );

        CREATE TABLE IF NOT EXISTS sync_state (
          scope TEXT PRIMARY KEY,
          server_cursor INTEGER NOT NULL DEFAULT 0,
          last_synced_at TEXT,
          last_error TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS sync_conflicts (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          entity_type TEXT NOT NULL,
          entity_uuid TEXT NOT NULL,
          local_revision INTEGER NOT NULL,
          server_revision INTEGER NOT NULL,
          local_payload TEXT NOT NULL,
          server_payload TEXT NOT NULL,
          detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          resolved_at TEXT,
          resolution TEXT NOT NULL DEFAULT ''
        );

        INSERT OR IGNORE INTO sync_settings (key, value) VALUES ('applying_remote', '0');
        INSERT OR IGNORE INTO sync_settings (key, value) VALUES ('server_url', '');
        INSERT OR IGNORE INTO sync_settings (key, value) VALUES ('server_device_id', '');
        INSERT OR IGNORE INTO sync_settings (key, value) VALUES ('device_prefix', '');
        INSERT OR IGNORE INTO sync_settings (key, value) VALUES ('server_token', '');
        INSERT OR IGNORE INTO sync_state (scope) VALUES ('main');
        "#,
    )?;

    if setting(connection, "installation_id")?.is_none() {
        set_setting(connection, "installation_id", &Uuid::new_v4().to_string())?;
    }
    let installation_id = setting(connection, "installation_id")?.unwrap_or_default();

    for (table, entity_type) in CORE_ENTITIES {
        ensure_column(connection, table, "sync_uuid", "TEXT NOT NULL DEFAULT ''")?;
        ensure_column(
            connection,
            table,
            "sync_revision",
            "INTEGER NOT NULL DEFAULT 0",
        )?;
        ensure_column(
            connection,
            table,
            "sync_origin_device_uuid",
            "TEXT NOT NULL DEFAULT ''",
        )?;
        ensure_column(connection, table, "sync_synced_at", "TEXT")?;
        ensure_column(connection, table, "sync_deleted_at", "TEXT")?;
        backfill_entity_ids(connection, table, &installation_id)?;
        connection.execute_batch(&format!(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_{table}_sync_uuid ON {table}(sync_uuid) WHERE sync_uuid != '';"
        ))?;
        seed_outbox(connection, table, entity_type)?;
        install_tracking_triggers(connection, table, entity_type)?;
    }
    install_child_triggers(connection, "work_orders", "work_order", ORDER_CHILDREN)?;
    install_child_triggers(
        connection,
        "inventory_items",
        "inventory_item",
        INVENTORY_CHILDREN,
    )?;
    install_child_triggers(
        connection,
        "purchase_reports",
        "purchase_report",
        PURCHASE_REPORT_CHILDREN,
    )?;
    install_child_triggers(
        connection,
        "purchase_receipts",
        "purchase_receipt",
        PURCHASE_RECEIPT_CHILDREN,
    )?;
    install_child_triggers(connection, "sales", "sale", SALE_CHILDREN)?;
    install_warranty_claim_triggers(connection)?;
    Ok(())
}

pub fn status(connection: &Connection) -> Result<LocalSyncStatus, CommandError> {
    let server_url = setting(connection, "server_url")?.unwrap_or_default();
    let device_id = setting(connection, "server_device_id")?.unwrap_or_default();
    let token_configured =
        setting(connection, "server_token")?.is_some_and(|value| !value.is_empty());
    let configured = !server_url.is_empty() && !device_id.is_empty() && token_configured;
    let (cursor, last_synced_at, last_error): (i64, Option<String>, String) = connection
        .query_row(
            "SELECT server_cursor, last_synced_at, last_error FROM sync_state WHERE scope = 'main'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
    Ok(LocalSyncStatus {
        installation_id: setting(connection, "installation_id")?.unwrap_or_default(),
        configured,
        server_url,
        device_id,
        device_prefix: setting(connection, "device_prefix")?.unwrap_or_default(),
        token_configured,
        pending_changes: connection
            .query_row("SELECT COUNT(*) FROM sync_outbox", [], |row| row.get(0))?,
        conflicts: connection.query_row(
            "SELECT COUNT(*) FROM sync_conflicts WHERE resolved_at IS NULL",
            [],
            |row| row.get(0),
        )?,
        last_server_cursor: cursor,
        last_synced_at,
        last_error,
    })
}

pub fn validate_configuration(
    input: ConfigureSyncInput,
) -> Result<RemoteCredentials, CommandError> {
    let server_url = input.server_url.trim().trim_end_matches('/').to_string();
    let parsed = Url::parse(&server_url)
        .map_err(|_| CommandError::message("La direcciÃ³n del servidor no es vÃ¡lida."))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(CommandError::message(
            "La direcciÃ³n debe comenzar con https:// (o http:// en una red privada).",
        ));
    }
    if parsed.scheme() == "http" && !is_private_host(parsed.host_str().unwrap_or_default()) {
        return Err(CommandError::message(
            "Las conexiones por internet deben usar HTTPS para proteger los datos y el token.",
        ));
    }
    let device_id = Uuid::parse_str(input.device_id.trim())
        .map_err(|_| CommandError::message("El identificador del dispositivo no es vÃ¡lido."))?;
    let device_prefix = input.device_prefix.trim().to_ascii_uppercase();
    if device_prefix.is_empty()
        || device_prefix.len() > 8
        || !device_prefix
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
    {
        return Err(CommandError::message(
            "El prefijo debe tener de 1 a 8 letras, nÃºmeros o guiones.",
        ));
    }
    let token = input.token.trim().to_string();
    if token.len() < 32 {
        return Err(CommandError::message(
            "El token del dispositivo estÃ¡ incompleto.",
        ));
    }
    Ok(RemoteCredentials {
        server_url,
        device_id,
        device_prefix,
        token,
    })
}

pub fn save_configuration(
    connection: &Connection,
    credentials: &RemoteCredentials,
) -> Result<LocalSyncStatus, CommandError> {
    set_setting(connection, "server_url", &credentials.server_url)?;
    set_setting(
        connection,
        "server_device_id",
        &credentials.device_id.to_string(),
    )?;
    set_setting(connection, "device_prefix", &credentials.device_prefix)?;
    set_setting(connection, "server_token", &credentials.token)?;
    connection.execute(
        "UPDATE sync_state SET last_error = '' WHERE scope = 'main'",
        [],
    )?;
    status(connection)
}

pub fn credentials(connection: &Connection) -> Result<Option<RemoteCredentials>, CommandError> {
    let server_url = setting(connection, "server_url")?.unwrap_or_default();
    let device_id = setting(connection, "server_device_id")?.unwrap_or_default();
    let device_prefix = setting(connection, "device_prefix")?.unwrap_or_default();
    let token = setting(connection, "server_token")?.unwrap_or_default();
    if server_url.is_empty() || device_id.is_empty() || token.is_empty() {
        return Ok(None);
    }
    let parsed_device_id = Uuid::parse_str(&device_id).map_err(|_| {
        CommandError::message("La configuraciÃ³n local de sincronizaciÃ³n estÃ¡ daÃ±ada.")
    })?;
    Ok(Some(RemoteCredentials {
        server_url,
        device_id: parsed_device_id,
        device_prefix,
        token,
    }))
}

pub fn prepare_push(connection: &Connection) -> Result<Option<PushRequest>, CommandError> {
    let credentials = match credentials(connection)? {
        Some(value) => value,
        None => return Ok(None),
    };
    let mut statement = connection.prepare(
        "SELECT operation_uuid, entity_type, entity_uuid, local_id, base_revision, action
         FROM sync_outbox
         ORDER BY CASE entity_type
           WHEN 'customer' THEN 1 WHEN 'asset' THEN 2 WHEN 'inventory_item' THEN 3
           WHEN 'work_order' THEN 4 WHEN 'purchase_report' THEN 5
           WHEN 'purchase_receipt' THEN 6 WHEN 'sale' THEN 7 ELSE 8 END, id
         LIMIT ?1",
    )?;
    let rows = statement.query_map([MAX_PUSH_MUTATIONS as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<i64>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;
    let timestamp: String =
        connection.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })?;
    let mut mutations = Vec::new();
    for row in rows {
        let (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action) = row?;
        let entity = parse_entity(&entity_type).ok_or_else(|| {
            CommandError::message("La cola contiene un tipo de dato desconocido.")
        })?;
        let deleted = action == "delete" || local_id.is_none();
        let payload = if deleted {
            Value::Object(Map::new())
        } else {
            snapshot_entity(connection, entity, local_id.unwrap())?
        };
        mutations.push(SyncMutation {
            operation_id: Uuid::parse_str(&operation_uuid).map_err(|_| {
                CommandError::message("La cola local contiene una operaciÃ³n daÃ±ada.")
            })?,
            entity,
            entity_id: Uuid::parse_str(&entity_uuid).map_err(|_| {
                CommandError::message("La cola local contiene una identidad daÃ±ada.")
            })?,
            base_revision,
            deleted,
            payload,
            client_timestamp: timestamp.clone(),
        });
    }
    if mutations.is_empty() {
        Ok(None)
    } else {
        Ok(Some(PushRequest {
            protocol_version: PROTOCOL_VERSION,
            device_id: credentials.device_id,
            mutations,
        }))
    }
}

pub fn apply_push_response(
    connection: &mut Connection,
    response: &PushResponse,
) -> Result<(usize, usize), CommandError> {
    if response.protocol_version != PROTOCOL_VERSION {
        return Err(CommandError::message(
            "El servidor usa una versiÃ³n de sincronizaciÃ³n incompatible.",
        ));
    }
    let transaction = connection.transaction()?;
    set_setting(&transaction, "applying_remote", "1")?;
    let mut applied = 0;
    let mut conflicts = 0;
    for result in &response.results {
        let operation_id = result.operation_id.to_string();
        let queued = transaction
            .query_row(
                "SELECT entity_type, entity_uuid, local_id, base_revision
                 FROM sync_outbox WHERE operation_uuid = ?1",
                [&operation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((entity_type, entity_uuid, local_id, base_revision)) = queued else {
            continue;
        };
        match result.status {
            MutationStatus::Applied | MutationStatus::Duplicate => {
                update_entity_revision(
                    &transaction,
                    &entity_type,
                    &entity_uuid,
                    result.server_revision,
                    true,
                )?;
                transaction.execute(
                    "DELETE FROM sync_outbox WHERE operation_uuid = ?1",
                    [&operation_id],
                )?;
                applied += 1;
            }
            MutationStatus::Conflict => {
                let local_payload =
                    if let (Some(id), Some(entity)) = (local_id, parse_entity(&entity_type)) {
                        snapshot_entity(&transaction, entity, id)?
                    } else {
                        Value::Object(Map::new())
                    };
                transaction.execute(
                    "INSERT INTO sync_conflicts
                       (entity_type, entity_uuid, local_revision, server_revision,
                        local_payload, server_payload, resolution)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'local_rebased')",
                    params![
                        entity_type,
                        entity_uuid,
                        base_revision,
                        result.server_revision,
                        local_payload.to_string(),
                        result
                            .server_record
                            .as_ref()
                            .unwrap_or(&Value::Object(Map::new()))
                            .to_string()
                    ],
                )?;
                update_entity_revision(
                    &transaction,
                    &entity_type,
                    &entity_uuid,
                    result.server_revision,
                    false,
                )?;
                transaction.execute(
                    "UPDATE sync_outbox
                     SET operation_uuid = ?1, base_revision = ?2, attempts = attempts + 1,
                         last_error = 'Conflicto reintentado con la revisiÃ³n actual del servidor',
                         updated_at = CURRENT_TIMESTAMP
                     WHERE operation_uuid = ?3",
                    params![
                        Uuid::new_v4().to_string(),
                        result.server_revision,
                        operation_id
                    ],
                )?;
                conflicts += 1;
            }
        }
    }
    set_setting(&transaction, "applying_remote", "0")?;
    transaction.commit()?;
    Ok((applied, conflicts))
}

pub fn apply_pull_response(
    connection: &mut Connection,
    device_id: Uuid,
    response: &PullResponse,
) -> Result<(usize, usize), CommandError> {
    if response.protocol_version != PROTOCOL_VERSION {
        return Err(CommandError::message(
            "El servidor usa una versiÃ³n de sincronizaciÃ³n incompatible.",
        ));
    }
    let transaction = connection.transaction()?;
    set_setting(&transaction, "applying_remote", "1")?;
    let mut applied = 0;
    let mut conflicts = 0;
    for change in &response.changes {
        let entity_type = change.entity.as_str();
        let entity_uuid = change.entity_id.to_string();
        let pending = transaction
            .query_row(
                "SELECT local_id, base_revision FROM sync_outbox
                 WHERE entity_type = ?1 AND entity_uuid = ?2",
                params![entity_type, entity_uuid],
                |row| Ok((row.get::<_, Option<i64>>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?;
        if change.origin_device_id == device_id {
            update_entity_revision(
                &transaction,
                entity_type,
                &entity_uuid,
                change.revision,
                pending.is_none(),
            )?;
            continue;
        }
        if let Some((local_id, local_revision)) = pending {
            let local_payload = if let Some(id) = local_id {
                snapshot_entity(&transaction, change.entity, id)?
            } else {
                Value::Object(Map::new())
            };
            transaction.execute(
                "INSERT INTO sync_conflicts
                   (entity_type, entity_uuid, local_revision, server_revision,
                    local_payload, server_payload, resolution)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'local_rebased')",
                params![
                    entity_type,
                    entity_uuid,
                    local_revision,
                    change.revision,
                    local_payload.to_string(),
                    change.payload.to_string()
                ],
            )?;
            update_entity_revision(
                &transaction,
                entity_type,
                &entity_uuid,
                change.revision,
                false,
            )?;
            transaction.execute(
                "UPDATE sync_outbox
                 SET operation_uuid = ?1, base_revision = ?2,
                     last_error = 'Cambio remoto recibido; se reenviarÃ¡ la versiÃ³n local',
                     updated_at = CURRENT_TIMESTAMP
                 WHERE entity_type = ?3 AND entity_uuid = ?4",
                params![
                    Uuid::new_v4().to_string(),
                    change.revision,
                    entity_type,
                    entity_uuid
                ],
            )?;
            conflicts += 1;
            continue;
        }
        apply_server_change(&transaction, change)?;
        applied += 1;
    }
    transaction.execute(
        "UPDATE sync_state
         SET server_cursor = ?1, last_synced_at = CURRENT_TIMESTAMP, last_error = ''
         WHERE scope = 'main'",
        [response.cursor],
    )?;
    set_setting(&transaction, "applying_remote", "0")?;
    transaction.commit()?;
    Ok((applied, conflicts))
}

pub fn cursor(connection: &Connection) -> Result<i64, CommandError> {
    connection
        .query_row(
            "SELECT server_cursor FROM sync_state WHERE scope = 'main'",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

pub fn record_error(connection: &Connection, error: &str) -> Result<(), CommandError> {
    connection.execute(
        "UPDATE sync_state SET last_error = ?1 WHERE scope = 'main'",
        [error],
    )?;
    Ok(())
}

fn is_private_host(host: &str) -> bool {
    if matches!(host, "localhost" | "127.0.0.1" | "::1") {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(|address| match address {
        IpAddr::V4(value) => value.is_private() || value.is_loopback(),
        IpAddr::V6(value) => value.is_loopback() || value.is_unique_local(),
    })
}

fn setting(connection: &Connection, key: &str) -> Result<Option<String>, CommandError> {
    Ok(connection
        .query_row(
            "SELECT value FROM sync_settings WHERE key = ?1",
            [key],
            |row| row.get(0),
        )
        .optional()?)
}

fn set_setting(connection: &Connection, key: &str, value: &str) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO sync_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = CURRENT_TIMESTAMP",
        params![key, value],
    )?;
    Ok(())
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), CommandError> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(());
        }
    }
    connection.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

fn backfill_entity_ids(
    connection: &Connection,
    table: &str,
    installation_id: &str,
) -> Result<(), CommandError> {
    let mut statement =
        connection.prepare(&format!("SELECT id FROM {table} WHERE sync_uuid = ''"))?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for id in ids {
        connection.execute(
            &format!(
                "UPDATE {table}
                 SET sync_uuid = ?1, sync_origin_device_uuid = ?2
                 WHERE id = ?3"
            ),
            params![Uuid::new_v4().to_string(), installation_id, id],
        )?;
    }
    Ok(())
}

fn sqlite_uuid_expression() -> &'static str {
    "lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-' ||
     lower(hex(randomblob(2))) || '-' || lower(hex(randomblob(2))) || '-' ||
     lower(hex(randomblob(6)))"
}

fn seed_outbox(
    connection: &Connection,
    table: &str,
    entity_type: &str,
) -> Result<(), CommandError> {
    connection.execute(
        &format!(
            "INSERT OR IGNORE INTO sync_outbox
               (operation_uuid, entity_type, entity_uuid, local_id, base_revision)
             SELECT {}, ?1, sync_uuid, id, sync_revision
             FROM {table}
             WHERE sync_uuid != '' AND sync_deleted_at IS NULL AND sync_synced_at IS NULL",
            sqlite_uuid_expression()
        ),
        [entity_type],
    )?;
    Ok(())
}

fn install_tracking_triggers(
    connection: &Connection,
    table: &str,
    entity_type: &str,
) -> Result<(), CommandError> {
    let uuid_expression = sqlite_uuid_expression();
    connection.execute_batch(&format!(
        r#"
        DROP TRIGGER IF EXISTS sync_{table}_after_insert;
        DROP TRIGGER IF EXISTS sync_{table}_after_update;
        DROP TRIGGER IF EXISTS sync_{table}_after_delete;

        CREATE TRIGGER sync_{table}_after_insert
        AFTER INSERT ON {table}
        WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
        BEGIN
          UPDATE {table}
          SET sync_uuid = CASE WHEN NEW.sync_uuid = '' THEN {uuid_expression} ELSE NEW.sync_uuid END,
              sync_origin_device_uuid = CASE
                WHEN NEW.sync_origin_device_uuid = ''
                THEN COALESCE((SELECT value FROM sync_settings WHERE key = 'installation_id'), '')
                ELSE NEW.sync_origin_device_uuid
              END
          WHERE id = NEW.id;

          INSERT INTO sync_outbox
            (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
          SELECT {uuid_expression}, '{entity_type}', sync_uuid, id, sync_revision, 'upsert'
          FROM {table} WHERE id = NEW.id
          ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
            operation_uuid = excluded.operation_uuid,
            local_id = excluded.local_id,
            action = 'upsert',
            updated_at = CURRENT_TIMESTAMP,
            attempts = 0,
            last_error = '';
        END;

        CREATE TRIGGER sync_{table}_after_update
        AFTER UPDATE ON {table}
        WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
             AND OLD.sync_uuid = NEW.sync_uuid
        BEGIN
          UPDATE {table} SET sync_synced_at = NULL WHERE id = NEW.id;

          INSERT INTO sync_outbox
            (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
          VALUES ({uuid_expression}, '{entity_type}', NEW.sync_uuid, NEW.id, NEW.sync_revision, 'upsert')
          ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
            operation_uuid = excluded.operation_uuid,
            local_id = excluded.local_id,
            action = 'upsert',
            updated_at = CURRENT_TIMESTAMP,
            attempts = 0,
            last_error = '';
        END;

        CREATE TRIGGER sync_{table}_after_delete
        AFTER DELETE ON {table}
        WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
             AND OLD.sync_uuid != ''
        BEGIN
          INSERT INTO sync_outbox
            (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
          VALUES ({uuid_expression}, '{entity_type}', OLD.sync_uuid, NULL, OLD.sync_revision, 'delete')
          ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
            operation_uuid = excluded.operation_uuid,
            local_id = NULL,
            base_revision = excluded.base_revision,
            action = 'delete',
            updated_at = CURRENT_TIMESTAMP,
            attempts = 0,
            last_error = '';
        END;
        "#
    ))?;
    Ok(())
}

fn install_child_triggers(
    connection: &Connection,
    parent_table: &str,
    entity_type: &str,
    children: &[ChildSpec],
) -> Result<(), CommandError> {
    let uuid_expression = sqlite_uuid_expression();
    for child in children {
        connection.execute_batch(&format!(
            r#"
            DROP TRIGGER IF EXISTS sync_{table}_parent_after_insert;
            DROP TRIGGER IF EXISTS sync_{table}_parent_after_update;
            DROP TRIGGER IF EXISTS sync_{table}_parent_after_delete;

            CREATE TRIGGER sync_{table}_parent_after_insert
            AFTER INSERT ON {table}
            WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
            BEGIN
              INSERT INTO sync_outbox
                (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
              SELECT {uuid_expression}, '{entity_type}', sync_uuid, id, sync_revision, 'upsert'
              FROM {parent_table} WHERE id = NEW.{parent_column}
              ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
                operation_uuid = excluded.operation_uuid, local_id = excluded.local_id,
                action = 'upsert', updated_at = CURRENT_TIMESTAMP, attempts = 0, last_error = '';
            END;

            CREATE TRIGGER sync_{table}_parent_after_update
            AFTER UPDATE ON {table}
            WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
            BEGIN
              INSERT INTO sync_outbox
                (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
              SELECT {uuid_expression}, '{entity_type}', sync_uuid, id, sync_revision, 'upsert'
              FROM {parent_table} WHERE id = NEW.{parent_column}
              ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
                operation_uuid = excluded.operation_uuid, local_id = excluded.local_id,
                action = 'upsert', updated_at = CURRENT_TIMESTAMP, attempts = 0, last_error = '';
            END;

            CREATE TRIGGER sync_{table}_parent_after_delete
            AFTER DELETE ON {table}
            WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
            BEGIN
              INSERT INTO sync_outbox
                (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
              SELECT {uuid_expression}, '{entity_type}', sync_uuid, id, sync_revision, 'upsert'
              FROM {parent_table} WHERE id = OLD.{parent_column}
              ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
                operation_uuid = excluded.operation_uuid, local_id = excluded.local_id,
                action = 'upsert', updated_at = CURRENT_TIMESTAMP, attempts = 0, last_error = '';
            END;
            "#,
            table = child.table,
            parent_column = child.parent_column,
        ))?;
    }
    Ok(())
}

fn install_warranty_claim_triggers(connection: &Connection) -> Result<(), CommandError> {
    let uuid_expression = sqlite_uuid_expression();
    connection.execute_batch(&format!(
        r#"
        DROP TRIGGER IF EXISTS sync_warranty_claims_parent_after_insert;
        DROP TRIGGER IF EXISTS sync_warranty_claims_parent_after_update;
        DROP TRIGGER IF EXISTS sync_warranty_claims_parent_after_delete;

        CREATE TRIGGER sync_warranty_claims_parent_after_insert
        AFTER INSERT ON warranty_claims
        WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
        BEGIN
          INSERT INTO sync_outbox
            (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
          SELECT {uuid_expression}, 'work_order', sync_uuid, id, sync_revision, 'upsert'
          FROM work_orders WHERE id IN (NEW.original_order_id, NEW.return_order_id)
          ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
            operation_uuid = excluded.operation_uuid, local_id = excluded.local_id,
            action = 'upsert', updated_at = CURRENT_TIMESTAMP, attempts = 0, last_error = '';
        END;

        CREATE TRIGGER sync_warranty_claims_parent_after_update
        AFTER UPDATE ON warranty_claims
        WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
        BEGIN
          INSERT INTO sync_outbox
            (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
          SELECT {uuid_expression}, 'work_order', sync_uuid, id, sync_revision, 'upsert'
          FROM work_orders WHERE id IN (NEW.original_order_id, NEW.return_order_id)
          ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
            operation_uuid = excluded.operation_uuid, local_id = excluded.local_id,
            action = 'upsert', updated_at = CURRENT_TIMESTAMP, attempts = 0, last_error = '';
        END;

        CREATE TRIGGER sync_warranty_claims_parent_after_delete
        AFTER DELETE ON warranty_claims
        WHEN COALESCE((SELECT value FROM sync_settings WHERE key = 'applying_remote'), '0') = '0'
        BEGIN
          INSERT INTO sync_outbox
            (operation_uuid, entity_type, entity_uuid, local_id, base_revision, action)
          SELECT {uuid_expression}, 'work_order', sync_uuid, id, sync_revision, 'upsert'
          FROM work_orders WHERE id IN (OLD.original_order_id, OLD.return_order_id)
          ON CONFLICT(entity_type, entity_uuid) DO UPDATE SET
            operation_uuid = excluded.operation_uuid, local_id = excluded.local_id,
            action = 'upsert', updated_at = CURRENT_TIMESTAMP, attempts = 0, last_error = '';
        END;
        "#
    ))?;
    Ok(())
}

fn snapshot_entity(
    connection: &Connection,
    entity: EntityKind,
    local_id: i64,
) -> Result<Value, CommandError> {
    let mut payload = Map::new();
    match entity {
        EntityKind::Customer => {
            payload.insert(
                "record".into(),
                Value::Object(snapshot_record(
                    connection,
                    "customers",
                    CUSTOMER_COLUMNS,
                    local_id,
                )?),
            );
        }
        EntityKind::Asset => {
            payload.insert(
                "record".into(),
                Value::Object(snapshot_record(
                    connection,
                    "assets",
                    ASSET_COLUMNS,
                    local_id,
                )?),
            );
        }
        EntityKind::InventoryItem => {
            payload.insert(
                "record".into(),
                Value::Object(snapshot_record(
                    connection,
                    "inventory_items",
                    INVENTORY_COLUMNS,
                    local_id,
                )?),
            );
            snapshot_aggregate_children(connection, &mut payload, INVENTORY_CHILDREN, local_id)?;
        }
        EntityKind::WorkOrder => {
            payload.insert(
                "record".into(),
                Value::Object(snapshot_record(
                    connection,
                    "work_orders",
                    ORDER_COLUMNS,
                    local_id,
                )?),
            );
            payload.insert(
                "assetUuid".into(),
                related_uuid(connection, "work_orders", "asset_id", "assets", local_id)?,
            );
            payload.insert(
                "customerUuid".into(),
                related_uuid(
                    connection,
                    "work_orders",
                    "customer_id",
                    "customers",
                    local_id,
                )?,
            );
            snapshot_aggregate_children(connection, &mut payload, ORDER_CHILDREN, local_id)?;
            payload.insert(
                "warrantyClaims".into(),
                Value::Array(snapshot_warranty_claims(connection, local_id)?),
            );
        }
        EntityKind::PurchaseReport => {
            payload.insert(
                "record".into(),
                Value::Object(snapshot_record(
                    connection,
                    "purchase_reports",
                    PURCHASE_REPORT_COLUMNS,
                    local_id,
                )?),
            );
            snapshot_aggregate_children(
                connection,
                &mut payload,
                PURCHASE_REPORT_CHILDREN,
                local_id,
            )?;
        }
        EntityKind::PurchaseReceipt => {
            payload.insert(
                "record".into(),
                Value::Object(snapshot_record(
                    connection,
                    "purchase_receipts",
                    PURCHASE_RECEIPT_COLUMNS,
                    local_id,
                )?),
            );
            payload.insert(
                "purchaseReportUuid".into(),
                related_uuid(
                    connection,
                    "purchase_receipts",
                    "purchase_report_id",
                    "purchase_reports",
                    local_id,
                )?,
            );
            snapshot_aggregate_children(
                connection,
                &mut payload,
                PURCHASE_RECEIPT_CHILDREN,
                local_id,
            )?;
        }
        EntityKind::Sale => {
            payload.insert(
                "record".into(),
                Value::Object(snapshot_record(
                    connection,
                    "sales",
                    SALE_COLUMNS,
                    local_id,
                )?),
            );
            snapshot_aggregate_children(connection, &mut payload, SALE_CHILDREN, local_id)?;
        }
        _ => {
            return Err(CommandError::message(
                "Esta entidad todavÃ­a no puede enviarse desde el cliente.",
            ));
        }
    }
    Ok(Value::Object(payload))
}

fn snapshot_aggregate_children(
    connection: &Connection,
    payload: &mut Map<String, Value>,
    children: &[ChildSpec],
    local_id: i64,
) -> Result<(), CommandError> {
    for child in children {
        payload.insert(
            child.key.into(),
            Value::Array(snapshot_children(connection, child, local_id)?),
        );
    }
    Ok(())
}

fn snapshot_record(
    connection: &Connection,
    table: &str,
    columns: &[&str],
    local_id: i64,
) -> Result<Map<String, Value>, CommandError> {
    let sql = format!("SELECT {} FROM {table} WHERE id = ?1", columns.join(", "));
    connection
        .query_row(&sql, [local_id], |row| {
            let mut record = Map::new();
            for (index, column) in columns.iter().enumerate() {
                record.insert((*column).into(), sql_value_to_json(row.get_ref(index)?));
            }
            Ok(record)
        })
        .map_err(Into::into)
}

fn related_uuid(
    connection: &Connection,
    source_table: &str,
    source_column: &str,
    target_table: &str,
    local_id: i64,
) -> Result<Value, CommandError> {
    let sql = format!(
        "SELECT target.sync_uuid FROM {source_table} source
         LEFT JOIN {target_table} target ON target.id = source.{source_column}
         WHERE source.id = ?1"
    );
    let value: Option<String> = connection.query_row(&sql, [local_id], |row| row.get(0))?;
    Ok(value.map(Value::String).unwrap_or(Value::Null))
}

fn snapshot_children(
    connection: &Connection,
    child: &ChildSpec,
    work_order_id: i64,
) -> Result<Vec<Value>, CommandError> {
    let sql = format!(
        "SELECT {} FROM {} WHERE {} = ?1 ORDER BY id",
        child.columns.join(", "),
        child.table,
        child.parent_column
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([work_order_id], |row| {
        let mut record = Map::new();
        for (index, column) in child.columns.iter().enumerate() {
            record.insert((*column).into(), sql_value_to_json(row.get_ref(index)?));
        }
        Ok(Value::Object(record))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn snapshot_warranty_claims(
    connection: &Connection,
    work_order_id: i64,
) -> Result<Vec<Value>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT wc.claim_number, wc.reported_issue, wc.received_condition,
                wc.decision, wc.resolution, wc.warranty_expires_at,
                wc.within_warranty, wc.created_at, wc.resolved_at,
                warranty.warranty_number, original.sync_uuid, returned.sync_uuid
         FROM warranty_claims wc
         JOIN order_warranties warranty ON warranty.id = wc.warranty_id
         JOIN work_orders original ON original.id = wc.original_order_id
         JOIN work_orders returned ON returned.id = wc.return_order_id
         WHERE wc.original_order_id = ?1 OR wc.return_order_id = ?1
         ORDER BY wc.id",
    )?;
    let names = [
        "claim_number",
        "reported_issue",
        "received_condition",
        "decision",
        "resolution",
        "warranty_expires_at",
        "within_warranty",
        "created_at",
        "resolved_at",
        "warranty_number",
        "original_order_uuid",
        "return_order_uuid",
    ];
    let rows = statement.query_map([work_order_id], |row| {
        let mut record = Map::new();
        for (index, name) in names.iter().enumerate() {
            record.insert((*name).into(), sql_value_to_json(row.get_ref(index)?));
        }
        Ok(Value::Object(record))
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn sql_value_to_json(value: ValueRef<'_>) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(value) => Value::Number(value.into()),
        ValueRef::Real(value) => Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ValueRef::Text(value) => Value::String(String::from_utf8_lossy(value).into_owned()),
        ValueRef::Blob(value) => Value::String(BASE64.encode(value)),
    }
}

fn apply_server_change(connection: &Connection, change: &ServerChange) -> Result<(), CommandError> {
    let entity_uuid = change.entity_id.to_string();
    let table = entity_table(change.entity)
        .ok_or_else(|| CommandError::message("El servidor enviÃ³ una entidad no compatible."))?;
    if change.deleted {
        connection.execute(
            &format!("DELETE FROM {table} WHERE sync_uuid = ?1"),
            [&entity_uuid],
        )?;
        return Ok(());
    }
    let payload = change
        .payload
        .as_object()
        .ok_or_else(|| CommandError::message("El servidor enviÃ³ un registro invÃ¡lido."))?;
    let record = payload
        .get("record")
        .and_then(Value::as_object)
        .ok_or_else(|| CommandError::message("El servidor enviÃ³ un registro incompleto."))?;
    let origin = change.origin_device_id.to_string();
    let local_id = match change.entity {
        EntityKind::Customer => upsert_record(
            connection,
            "customers",
            CUSTOMER_COLUMNS,
            &entity_uuid,
            change.revision,
            &origin,
            record,
            &[],
        )?,
        EntityKind::Asset => upsert_record(
            connection,
            "assets",
            ASSET_COLUMNS,
            &entity_uuid,
            change.revision,
            &origin,
            record,
            &[],
        )?,
        EntityKind::InventoryItem => upsert_record(
            connection,
            "inventory_items",
            INVENTORY_COLUMNS,
            &entity_uuid,
            change.revision,
            &origin,
            record,
            &[],
        )?,
        EntityKind::WorkOrder => {
            let asset_id = resolve_relation(connection, "assets", payload.get("assetUuid"))?;
            let customer_id =
                resolve_relation(connection, "customers", payload.get("customerUuid"))?;
            let relations = [
                ("asset_id", asset_id.map(Value::from).unwrap_or(Value::Null)),
                (
                    "customer_id",
                    customer_id.map(Value::from).unwrap_or(Value::Null),
                ),
            ];
            upsert_record(
                connection,
                "work_orders",
                ORDER_COLUMNS,
                &entity_uuid,
                change.revision,
                &origin,
                record,
                &relations,
            )?
        }
        EntityKind::PurchaseReport => upsert_record(
            connection,
            "purchase_reports",
            PURCHASE_REPORT_COLUMNS,
            &entity_uuid,
            change.revision,
            &origin,
            record,
            &[],
        )?,
        EntityKind::PurchaseReceipt => {
            let report_id = resolve_relation(
                connection,
                "purchase_reports",
                payload.get("purchaseReportUuid"),
            )?;
            let relations = [(
                "purchase_report_id",
                report_id.map(Value::from).unwrap_or(Value::Null),
            )];
            upsert_record(
                connection,
                "purchase_receipts",
                PURCHASE_RECEIPT_COLUMNS,
                &entity_uuid,
                change.revision,
                &origin,
                record,
                &relations,
            )?
        }
        EntityKind::Sale => upsert_record(
            connection,
            "sales",
            SALE_COLUMNS,
            &entity_uuid,
            change.revision,
            &origin,
            record,
            &[],
        )?,
        _ => {
            return Err(CommandError::message(
                "El servidor enviÃ³ una entidad que esta versiÃ³n aÃºn no admite.",
            ));
        }
    };
    if let Some(children) = entity_children(change.entity) {
        for child in children {
            replace_children(connection, child, local_id, payload.get(child.key))?;
        }
    }
    if change.entity == EntityKind::WorkOrder {
        replace_warranty_claims(connection, local_id, payload.get("warrantyClaims"))?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn upsert_record(
    connection: &Connection,
    table: &str,
    columns: &[&str],
    entity_uuid: &str,
    revision: i64,
    origin: &str,
    record: &Map<String, Value>,
    relations: &[(&str, Value)],
) -> Result<i64, CommandError> {
    let existing: Option<i64> = connection
        .query_row(
            &format!("SELECT id FROM {table} WHERE sync_uuid = ?1"),
            [entity_uuid],
            |row| row.get(0),
        )
        .optional()?;
    let mut names = columns.to_vec();
    names.extend(relations.iter().map(|(name, _)| *name));
    let mut values = columns
        .iter()
        .map(|column| json_to_sql_value(*column, record.get(*column).unwrap_or(&Value::Null)))
        .collect::<Result<Vec<_>, _>>()?;
    values.extend(
        relations
            .iter()
            .map(|(column, value)| json_to_sql_value(column, value))
            .collect::<Result<Vec<_>, _>>()?,
    );
    if let Some(id) = existing {
        let assignments = names
            .iter()
            .enumerate()
            .map(|(index, name)| format!("{name} = ?{}", index + 1))
            .collect::<Vec<_>>();
        values.push(rusqlite::types::Value::Integer(revision));
        values.push(rusqlite::types::Value::Text(origin.into()));
        values.push(rusqlite::types::Value::Integer(id));
        connection.execute(
            &format!(
                "UPDATE {table} SET {}, sync_revision = ?{}, sync_origin_device_uuid = ?{},
                 sync_synced_at = CURRENT_TIMESTAMP, sync_deleted_at = NULL WHERE id = ?{}",
                assignments.join(", "),
                names.len() + 1,
                names.len() + 2,
                names.len() + 3
            ),
            params_from_iter(values),
        )?;
        Ok(id)
    } else {
        let mut insert_names = names;
        insert_names.extend([
            "sync_uuid",
            "sync_revision",
            "sync_origin_device_uuid",
            "sync_synced_at",
        ]);
        values.push(rusqlite::types::Value::Text(entity_uuid.into()));
        values.push(rusqlite::types::Value::Integer(revision));
        values.push(rusqlite::types::Value::Text(origin.into()));
        values.push(rusqlite::types::Value::Text(chrono_free_timestamp(
            connection,
        )?));
        let placeholders = (1..=insert_names.len())
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>();
        connection.execute(
            &format!(
                "INSERT INTO {table} ({}) VALUES ({})",
                insert_names.join(", "),
                placeholders.join(", ")
            ),
            params_from_iter(values),
        )?;
        Ok(connection.last_insert_rowid())
    }
}

fn chrono_free_timestamp(connection: &Connection) -> Result<String, CommandError> {
    connection
        .query_row("SELECT CURRENT_TIMESTAMP", [], |row| row.get(0))
        .map_err(Into::into)
}

fn resolve_relation(
    connection: &Connection,
    table: &str,
    value: Option<&Value>,
) -> Result<Option<i64>, CommandError> {
    let Some(uuid) = value.and_then(Value::as_str) else {
        return Ok(None);
    };
    connection
        .query_row(
            &format!("SELECT id FROM {table} WHERE sync_uuid = ?1"),
            [uuid],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

fn replace_children(
    connection: &Connection,
    child: &ChildSpec,
    parent_id: i64,
    value: Option<&Value>,
) -> Result<(), CommandError> {
    connection.execute(
        &format!(
            "DELETE FROM {} WHERE {} = ?1",
            child.table, child.parent_column
        ),
        [parent_id],
    )?;
    let Some(rows) = value.and_then(Value::as_array) else {
        return Ok(());
    };
    let include_inventory_relation = matches!(
        child.table,
        "work_order_parts" | "purchase_report_items" | "purchase_receipt_items" | "sale_items"
    );
    let mut names = vec![child.parent_column];
    if include_inventory_relation {
        names.push("inventory_item_id");
    }
    names.extend(child.columns.iter().copied());
    let placeholders = (1..=names.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>();
    let sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        child.table,
        names.join(", "),
        placeholders.join(", ")
    );
    for row in rows {
        let record = row
            .as_object()
            .ok_or_else(|| CommandError::message("El servidor enviÃ³ un detalle invÃ¡lido."))?;
        let mut values = vec![rusqlite::types::Value::Integer(parent_id)];
        if include_inventory_relation {
            let inventory_id = record
                .get("item_code")
                .and_then(Value::as_str)
                .map(|code| {
                    connection
                        .query_row(
                            "SELECT id FROM inventory_items WHERE code = ?1 COLLATE NOCASE",
                            [code],
                            |row| row.get::<_, i64>(0),
                        )
                        .optional()
                })
                .transpose()?
                .flatten();
            values.push(
                inventory_id
                    .map(rusqlite::types::Value::Integer)
                    .unwrap_or(rusqlite::types::Value::Null),
            );
        }
        for column in child.columns {
            values.push(json_to_sql_value(
                column,
                record.get(*column).unwrap_or(&Value::Null),
            )?);
        }
        connection.execute(&sql, params_from_iter(values))?;
    }
    Ok(())
}

fn replace_warranty_claims(
    connection: &Connection,
    work_order_id: i64,
    value: Option<&Value>,
) -> Result<(), CommandError> {
    connection.execute(
        "DELETE FROM warranty_claims WHERE original_order_id = ?1 OR return_order_id = ?1",
        [work_order_id],
    )?;
    let Some(rows) = value.and_then(Value::as_array) else {
        return Ok(());
    };
    for row in rows {
        let record = row
            .as_object()
            .ok_or_else(|| CommandError::message("El servidor enviÃ³ una garantÃ­a invÃ¡lida."))?;
        let original_id =
            resolve_relation(connection, "work_orders", record.get("original_order_uuid"))?;
        let return_id =
            resolve_relation(connection, "work_orders", record.get("return_order_uuid"))?;
        let warranty_number = record
            .get("warranty_number")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let warranty_id: Option<i64> = connection
            .query_row(
                "SELECT id FROM order_warranties WHERE warranty_number = ?1",
                [warranty_number],
                |row| row.get(0),
            )
            .optional()?;
        let (Some(warranty_id), Some(original_id), Some(return_id)) =
            (warranty_id, original_id, return_id)
        else {
            continue;
        };
        let columns = [
            "claim_number",
            "reported_issue",
            "received_condition",
            "decision",
            "resolution",
            "warranty_expires_at",
            "within_warranty",
            "created_at",
            "resolved_at",
        ];
        let mut values = vec![
            rusqlite::types::Value::Integer(warranty_id),
            rusqlite::types::Value::Integer(original_id),
            rusqlite::types::Value::Integer(return_id),
        ];
        for column in columns {
            values.push(json_to_sql_value(
                column,
                record.get(column).unwrap_or(&Value::Null),
            )?);
        }
        connection.execute(
            "INSERT INTO warranty_claims
               (warranty_id, original_order_id, return_order_id, claim_number,
                reported_issue, received_condition, decision, resolution,
                warranty_expires_at, within_warranty, created_at, resolved_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params_from_iter(values),
        )?;
    }
    Ok(())
}

fn json_to_sql_value(column: &str, value: &Value) -> Result<rusqlite::types::Value, CommandError> {
    Ok(match value {
        Value::Null => rusqlite::types::Value::Null,
        Value::Bool(value) => rusqlite::types::Value::Integer(i64::from(*value)),
        Value::Number(value) if value.is_i64() => {
            rusqlite::types::Value::Integer(value.as_i64().unwrap_or_default())
        }
        Value::Number(value) => rusqlite::types::Value::Real(
            value
                .as_f64()
                .ok_or_else(|| CommandError::message("El servidor enviÃ³ un nÃºmero invÃ¡lido."))?,
        ),
        Value::String(value) if column == "data" => rusqlite::types::Value::Blob(
            BASE64
                .decode(value)
                .map_err(|_| CommandError::message("Una fotografÃ­a sincronizada estÃ¡ daÃ±ada."))?,
        ),
        Value::String(value) => rusqlite::types::Value::Text(value.clone()),
        _ => {
            return Err(CommandError::message(
                "El servidor enviÃ³ un valor incompatible con SQLite.",
            ));
        }
    })
}

fn update_entity_revision(
    connection: &Connection,
    entity_type: &str,
    entity_uuid: &str,
    revision: i64,
    synced: bool,
) -> Result<(), CommandError> {
    let Some(entity) = parse_entity(entity_type) else {
        return Ok(());
    };
    let Some(table) = entity_table(entity) else {
        return Ok(());
    };
    connection.execute(
        &format!(
            "UPDATE {table} SET sync_revision = ?1,
             sync_synced_at = CASE WHEN ?2 THEN CURRENT_TIMESTAMP ELSE NULL END
             WHERE sync_uuid = ?3"
        ),
        params![revision, synced, entity_uuid],
    )?;
    Ok(())
}

fn entity_table(entity: EntityKind) -> Option<&'static str> {
    match entity {
        EntityKind::Customer => Some("customers"),
        EntityKind::Asset => Some("assets"),
        EntityKind::InventoryItem => Some("inventory_items"),
        EntityKind::WorkOrder => Some("work_orders"),
        EntityKind::PurchaseReport => Some("purchase_reports"),
        EntityKind::PurchaseReceipt => Some("purchase_receipts"),
        EntityKind::Sale => Some("sales"),
        _ => None,
    }
}

fn entity_children(entity: EntityKind) -> Option<&'static [ChildSpec]> {
    match entity {
        EntityKind::InventoryItem => Some(INVENTORY_CHILDREN),
        EntityKind::WorkOrder => Some(ORDER_CHILDREN),
        EntityKind::PurchaseReport => Some(PURCHASE_REPORT_CHILDREN),
        EntityKind::PurchaseReceipt => Some(PURCHASE_RECEIPT_CHILDREN),
        EntityKind::Sale => Some(SALE_CHILDREN),
        _ => None,
    }
}

fn parse_entity(value: &str) -> Option<EntityKind> {
    match value {
        "customer" => Some(EntityKind::Customer),
        "asset" => Some(EntityKind::Asset),
        "inventory_item" => Some(EntityKind::InventoryItem),
        "work_order" => Some(EntityKind::WorkOrder),
        "purchase_report" => Some(EntityKind::PurchaseReport),
        "purchase_receipt" => Some(EntityKind::PurchaseReceipt),
        "sale" => Some(EntityKind::Sale),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE customers (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, phone TEXT DEFAULT '', email TEXT DEFAULT '', address TEXT DEFAULT '', company TEXT DEFAULT '', tax_id TEXT DEFAULT '', notes TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
                 CREATE TABLE assets (id INTEGER PRIMARY KEY AUTOINCREMENT, code TEXT UNIQUE, name TEXT NOT NULL, category TEXT DEFAULT '', location TEXT DEFAULT '', status TEXT DEFAULT 'operativo', manufacturer TEXT DEFAULT '', model TEXT DEFAULT '', serial_number TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
                 CREATE TABLE work_orders (id INTEGER PRIMARY KEY AUTOINCREMENT, order_number TEXT UNIQUE, title TEXT NOT NULL, description TEXT DEFAULT '', type TEXT DEFAULT 'correctivo', priority TEXT DEFAULT 'media', status TEXT DEFAULT 'solicitada', asset_id INTEGER, location TEXT DEFAULT '', assigned_to TEXT DEFAULT '', requested_by TEXT DEFAULT '', customer_id INTEGER, scheduled_date TEXT, estimated_hours REAL, safety_notes TEXT DEFAULT '', observations TEXT DEFAULT '', work_performed TEXT DEFAULT '', delivery_condition TEXT DEFAULT '', delivery_tested_by TEXT DEFAULT '', diagnosis TEXT DEFAULT '', recommendation TEXT DEFAULT '', quote_status TEXT DEFAULT 'draft', quote_subtotal_cents INTEGER DEFAULT 0, quote_discount_cents INTEGER DEFAULT 0, quote_total_cents INTEGER DEFAULT 0, authorized_by TEXT DEFAULT '', authorization_method TEXT DEFAULT '', authorization_note TEXT DEFAULT '', authorization_at TEXT, created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP, completed_at TEXT);
                 CREATE TABLE work_order_tasks (id INTEGER PRIMARY KEY, work_order_id INTEGER, description TEXT, completed INTEGER DEFAULT 0, completed_at TEXT, position INTEGER DEFAULT 0);
                 CREATE TABLE work_order_events (id INTEGER PRIMARY KEY, work_order_id INTEGER, action TEXT, previous_status TEXT, new_status TEXT, note TEXT, created_at TEXT DEFAULT CURRENT_TIMESTAMP);
                 CREATE TABLE work_order_accessories (id INTEGER PRIMARY KEY, work_order_id INTEGER, name TEXT, position INTEGER DEFAULT 0);
                 CREATE TABLE work_order_quote_items (id INTEGER PRIMARY KEY, work_order_id INTEGER, kind TEXT, description TEXT, quantity REAL, unit_price_cents INTEGER, line_total_cents INTEGER, position INTEGER);
                 CREATE TABLE work_order_parts (id INTEGER PRIMARY KEY, work_order_id INTEGER, inventory_item_id INTEGER, item_code TEXT, item_name TEXT, unit TEXT, quantity INTEGER, unit_price_cents INTEGER, line_total_cents INTEGER, position INTEGER);
                 CREATE TABLE work_order_labor (id INTEGER PRIMARY KEY, work_order_id INTEGER, technician TEXT, description TEXT, hours REAL, hourly_rate_cents INTEGER, line_total_cents INTEGER, position INTEGER);
                 CREATE TABLE order_payments (id INTEGER PRIMARY KEY, receipt_number TEXT UNIQUE, work_order_id INTEGER, payment_type TEXT, method TEXT, amount_cents INTEGER, reference TEXT, received_by TEXT, note TEXT, status TEXT, order_total_cents INTEGER, balance_before_cents INTEGER, balance_after_cents INTEGER, created_at TEXT, cancelled_at TEXT, cancellation_note TEXT);
                 CREATE TABLE order_warranties (id INTEGER PRIMARY KEY, warranty_number TEXT UNIQUE, work_order_id INTEGER UNIQUE, duration_days INTEGER, starts_at TEXT, expires_at TEXT, coverage TEXT, exclusions TEXT, status TEXT, created_at TEXT, updated_at TEXT);
                 CREATE TABLE warranty_claims (id INTEGER PRIMARY KEY, claim_number TEXT UNIQUE, warranty_id INTEGER, original_order_id INTEGER, return_order_id INTEGER UNIQUE, reported_issue TEXT, received_condition TEXT, decision TEXT, resolution TEXT, warranty_expires_at TEXT, within_warranty INTEGER, created_at TEXT, resolved_at TEXT);
                 CREATE TABLE order_photos (id INTEGER PRIMARY KEY, work_order_id INTEGER, file_name TEXT, mime_type TEXT, category TEXT, caption TEXT, data BLOB, created_by TEXT, created_at TEXT);
                 CREATE TABLE inventory_items (id INTEGER PRIMARY KEY AUTOINCREMENT, code TEXT UNIQUE, name TEXT, category TEXT, quantity INTEGER, minimum_stock INTEGER, unit TEXT, created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
                 CREATE TABLE inventory_movements (id INTEGER PRIMARY KEY, inventory_item_id INTEGER, item_code TEXT, item_name TEXT, unit TEXT, movement_type TEXT, quantity_delta INTEGER, balance_after INTEGER, reference_type TEXT, reference_id INTEGER, reference_number TEXT, note TEXT, created_at TEXT);
                 CREATE TABLE purchase_reports (id INTEGER PRIMARY KEY AUTOINCREMENT, report_number TEXT UNIQUE, supplier TEXT, notes TEXT, total_cost_cents INTEGER, created_at TEXT DEFAULT CURRENT_TIMESTAMP);
                 CREATE TABLE purchase_report_items (id INTEGER PRIMARY KEY, purchase_report_id INTEGER, inventory_item_id INTEGER, item_code TEXT, item_name TEXT, unit TEXT, stock_quantity INTEGER, purchase_quantity INTEGER, received_quantity INTEGER, unit_cost_cents INTEGER, line_total_cents INTEGER, position INTEGER);
                 CREATE TABLE purchase_receipts (id INTEGER PRIMARY KEY AUTOINCREMENT, receipt_number TEXT UNIQUE, purchase_report_id INTEGER, supplier TEXT, notes TEXT, total_cost_cents INTEGER, created_at TEXT DEFAULT CURRENT_TIMESTAMP);
                 CREATE TABLE purchase_receipt_items (id INTEGER PRIMARY KEY, purchase_receipt_id INTEGER, purchase_report_item_id INTEGER, inventory_item_id INTEGER, item_code TEXT, item_name TEXT, unit TEXT, received_quantity INTEGER, unit_cost_cents INTEGER, line_total_cents INTEGER, position INTEGER);
                 CREATE TABLE sales (id INTEGER PRIMARY KEY AUTOINCREMENT, sale_number TEXT UNIQUE, customer_name TEXT, notes TEXT, subtotal_cents INTEGER, discount_cents INTEGER, total_cents INTEGER, status TEXT, cancellation_note TEXT, created_at TEXT, cancelled_at TEXT);
                 CREATE TABLE sale_items (id INTEGER PRIMARY KEY, sale_id INTEGER, inventory_item_id INTEGER, item_code TEXT, item_name TEXT, unit TEXT, quantity INTEGER, unit_price_cents INTEGER, line_total_cents INTEGER, position INTEGER);
                 INSERT INTO customers (name) VALUES ('Cliente existente');",
            )
            .unwrap();
        initialize(&connection).unwrap();
        connection
    }

    #[test]
    fn assigns_ids_and_queues_existing_records() {
        let connection = database();
        let sync_uuid: String = connection
            .query_row("SELECT sync_uuid FROM customers WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(Uuid::parse_str(&sync_uuid).is_ok());
        let status = status(&connection).unwrap();
        assert_eq!(status.pending_changes, 1);
        assert!(Uuid::parse_str(&status.installation_id).is_ok());
    }

    #[test]
    fn collapses_repeated_local_updates_into_one_pending_change() {
        let connection = database();
        connection.execute("DELETE FROM sync_outbox", []).unwrap();
        connection
            .execute("UPDATE customers SET name = 'Uno' WHERE id = 1", [])
            .unwrap();
        connection
            .execute("UPDATE customers SET name = 'Dos' WHERE id = 1", [])
            .unwrap();
        assert_eq!(status(&connection).unwrap().pending_changes, 1);
    }

    #[test]
    fn tracks_new_records_with_a_valid_uuid() {
        let connection = database();
        connection.execute("DELETE FROM sync_outbox", []).unwrap();
        connection
            .execute(
                "INSERT INTO assets (code, name) VALUES ('EQ-1', 'PortÃ¡til')",
                [],
            )
            .unwrap();
        let sync_uuid: String = connection
            .query_row("SELECT sync_uuid FROM assets WHERE id = 1", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert!(Uuid::parse_str(&sync_uuid).is_ok());
        assert_eq!(status(&connection).unwrap().pending_changes, 1);
    }

    #[test]
    fn a_child_change_queues_its_parent_order() {
        let connection = database();
        connection
            .execute("INSERT INTO work_orders (title) VALUES ('Orden')", [])
            .unwrap();
        connection.execute("DELETE FROM sync_outbox", []).unwrap();
        connection
            .execute(
                "INSERT INTO work_order_tasks (work_order_id, description) VALUES (1, 'DiagnÃ³stico')",
                [],
            )
            .unwrap();
        let entity: String = connection
            .query_row("SELECT entity_type FROM sync_outbox", [], |row| row.get(0))
            .unwrap();
        assert_eq!(entity, "work_order");
    }
}
