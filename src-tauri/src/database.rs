use crate::domain::*;
use crate::sync;
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};
use std::path::Path;
use std::time::Duration;

pub const CURRENT_DATABASE_VERSION: i64 = 15;

const ORDER_SELECT: &str = r#"
  SELECT
    wo.id,
    wo.order_number,
    wo.title,
    wo.description,
    wo.type,
    wo.priority,
    wo.status,
    wo.asset_id,
    a.code,
    a.name,
    a.category,
    a.manufacturer,
    a.model,
    a.serial_number,
    COALESCE(NULLIF(wo.location, ''), a.location, ''),
    wo.assigned_to,
    COALESCE(NULLIF(c.name, ''), wo.requested_by, ''),
    wo.scheduled_date,
    wo.estimated_hours,
    wo.safety_notes,
    wo.observations,
    wo.work_performed,
    wo.delivery_condition,
    wo.delivery_tested_by,
    wo.created_at,
    wo.updated_at,
    wo.completed_at,
    (SELECT COUNT(*) FROM work_order_tasks t WHERE t.work_order_id = wo.id),
    (SELECT COUNT(*) FROM work_order_tasks t WHERE t.work_order_id = wo.id AND t.completed = 1),
    wo.customer_id,
    COALESCE(c.phone, ''),
    COALESCE(c.email, ''),
    COALESCE(c.address, ''),
    COALESCE(c.company, ''),
    COALESCE(c.tax_id, ''),
    wo.diagnosis,
    wo.recommendation,
    wo.quote_status,
    wo.quote_subtotal_cents,
    wo.quote_discount_cents,
    wo.quote_total_cents,
    wo.authorized_by,
    wo.authorization_method,
    wo.authorization_note,
    wo.authorization_at
  FROM work_orders wo
  LEFT JOIN assets a ON a.id = wo.asset_id
  LEFT JOIN customers c ON c.id = wo.customer_id
"#;

pub fn open(path: &Path) -> Result<Connection, CommandError> {
    let connection = Connection::open(path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "recursive_triggers", "OFF")?;
    connection.busy_timeout(Duration::from_secs(5))?;
    Ok(connection)
}

pub fn initialize(path: &Path) -> Result<(), CommandError> {
    let connection = open(path)?;
    let existing_version: i64 =
        connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if existing_version > CURRENT_DATABASE_VERSION {
        return Err(CommandError::message(format!(
            "La base de datos pertenece a una versión más reciente de Geecko ({existing_version})."
        )));
    }
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS customers (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          name TEXT NOT NULL,
          phone TEXT NOT NULL DEFAULT '',
          email TEXT NOT NULL DEFAULT '',
          address TEXT NOT NULL DEFAULT '',
          company TEXT NOT NULL DEFAULT '',
          tax_id TEXT NOT NULL DEFAULT '',
          notes TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS assets (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          code TEXT NOT NULL UNIQUE COLLATE NOCASE,
          name TEXT NOT NULL,
          category TEXT NOT NULL DEFAULT '',
          location TEXT NOT NULL,
          status TEXT NOT NULL DEFAULT 'operativo',
          manufacturer TEXT NOT NULL DEFAULT '',
          model TEXT NOT NULL DEFAULT '',
          serial_number TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS work_orders (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          order_number TEXT UNIQUE,
          title TEXT NOT NULL,
          description TEXT NOT NULL,
          type TEXT NOT NULL,
          priority TEXT NOT NULL,
          status TEXT NOT NULL DEFAULT 'solicitada',
          asset_id INTEGER REFERENCES assets(id) ON DELETE SET NULL,
          location TEXT NOT NULL DEFAULT '',
          assigned_to TEXT NOT NULL DEFAULT '',
          requested_by TEXT NOT NULL DEFAULT '',
          customer_id INTEGER REFERENCES customers(id) ON DELETE SET NULL,
          scheduled_date TEXT,
          estimated_hours REAL,
          safety_notes TEXT NOT NULL DEFAULT '',
          observations TEXT NOT NULL DEFAULT '',
          work_performed TEXT NOT NULL DEFAULT '',
          delivery_condition TEXT NOT NULL DEFAULT '',
          delivery_tested_by TEXT NOT NULL DEFAULT '',
          diagnosis TEXT NOT NULL DEFAULT '',
          recommendation TEXT NOT NULL DEFAULT '',
          quote_status TEXT NOT NULL DEFAULT 'draft',
          quote_subtotal_cents INTEGER NOT NULL DEFAULT 0,
          quote_discount_cents INTEGER NOT NULL DEFAULT 0,
          quote_total_cents INTEGER NOT NULL DEFAULT 0,
          authorized_by TEXT NOT NULL DEFAULT '',
          authorization_method TEXT NOT NULL DEFAULT '',
          authorization_note TEXT NOT NULL DEFAULT '',
          authorization_at TEXT,
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          completed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS work_order_tasks (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          work_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE CASCADE,
          description TEXT NOT NULL,
          completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
          completed_at TEXT,
          position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS work_order_events (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          work_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE CASCADE,
          action TEXT NOT NULL,
          previous_status TEXT,
          new_status TEXT,
          note TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS work_order_accessories (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          work_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE CASCADE,
          name TEXT NOT NULL,
          position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS work_order_quote_items (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          work_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE CASCADE,
          kind TEXT NOT NULL DEFAULT 'mano_obra',
          description TEXT NOT NULL,
          quantity REAL NOT NULL,
          unit_price_cents INTEGER NOT NULL,
          line_total_cents INTEGER NOT NULL,
          position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS work_order_parts (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          work_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE CASCADE,
          inventory_item_id INTEGER REFERENCES inventory_items(id) ON DELETE SET NULL,
          item_code TEXT NOT NULL,
          item_name TEXT NOT NULL,
          unit TEXT NOT NULL,
          quantity INTEGER NOT NULL CHECK (quantity > 0),
          unit_price_cents INTEGER NOT NULL DEFAULT 0,
          line_total_cents INTEGER NOT NULL DEFAULT 0,
          position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS work_order_labor (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          work_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE CASCADE,
          technician TEXT NOT NULL,
          description TEXT NOT NULL,
          hours REAL NOT NULL CHECK (hours > 0),
          hourly_rate_cents INTEGER NOT NULL DEFAULT 0,
          line_total_cents INTEGER NOT NULL DEFAULT 0,
          position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS order_payments (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          receipt_number TEXT UNIQUE,
          work_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE RESTRICT,
          payment_type TEXT NOT NULL,
          method TEXT NOT NULL,
          amount_cents INTEGER NOT NULL CHECK (amount_cents > 0),
          reference TEXT NOT NULL DEFAULT '',
          received_by TEXT NOT NULL,
          note TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL DEFAULT 'active',
          order_total_cents INTEGER NOT NULL DEFAULT 0,
          balance_before_cents INTEGER NOT NULL DEFAULT 0,
          balance_after_cents INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          cancelled_at TEXT,
          cancellation_note TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS order_warranties (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          warranty_number TEXT UNIQUE,
          work_order_id INTEGER NOT NULL UNIQUE REFERENCES work_orders(id) ON DELETE RESTRICT,
          duration_days INTEGER NOT NULL CHECK (duration_days > 0),
          starts_at TEXT NOT NULL,
          expires_at TEXT NOT NULL,
          coverage TEXT NOT NULL,
          exclusions TEXT NOT NULL DEFAULT '',
          status TEXT NOT NULL DEFAULT 'active',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS warranty_claims (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          claim_number TEXT UNIQUE,
          warranty_id INTEGER NOT NULL REFERENCES order_warranties(id) ON DELETE RESTRICT,
          original_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE RESTRICT,
          return_order_id INTEGER NOT NULL UNIQUE REFERENCES work_orders(id) ON DELETE RESTRICT,
          reported_issue TEXT NOT NULL,
          received_condition TEXT NOT NULL DEFAULT '',
          decision TEXT NOT NULL DEFAULT 'pending',
          resolution TEXT NOT NULL DEFAULT '',
          warranty_expires_at TEXT NOT NULL,
          within_warranty INTEGER NOT NULL DEFAULT 0 CHECK (within_warranty IN (0, 1)),
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          resolved_at TEXT
        );

        CREATE TABLE IF NOT EXISTS app_users (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          username TEXT NOT NULL UNIQUE COLLATE NOCASE,
          display_name TEXT NOT NULL,
          role TEXT NOT NULL CHECK (role IN ('admin', 'reception', 'technician')),
          password_hash TEXT NOT NULL,
          active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          last_login_at TEXT
        );

        CREATE TABLE IF NOT EXISTS user_activity (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          user_id INTEGER REFERENCES app_users(id) ON DELETE SET NULL,
          user_display_name TEXT NOT NULL,
          user_role TEXT NOT NULL,
          action TEXT NOT NULL,
          entity_type TEXT NOT NULL DEFAULT '',
          entity_id INTEGER,
          summary TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS order_photos (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          work_order_id INTEGER NOT NULL REFERENCES work_orders(id) ON DELETE CASCADE,
          file_name TEXT NOT NULL,
          mime_type TEXT NOT NULL,
          category TEXT NOT NULL,
          caption TEXT NOT NULL DEFAULT '',
          data BLOB NOT NULL,
          created_by TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS inventory_items (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          code TEXT NOT NULL UNIQUE COLLATE NOCASE,
          name TEXT NOT NULL,
          category TEXT NOT NULL DEFAULT '',
          quantity INTEGER NOT NULL DEFAULT 0 CHECK (quantity >= 0),
          minimum_stock INTEGER NOT NULL DEFAULT 0 CHECK (minimum_stock >= 0),
          unit TEXT NOT NULL DEFAULT 'unidad',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS purchase_reports (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          report_number TEXT UNIQUE,
          supplier TEXT NOT NULL DEFAULT '',
          notes TEXT NOT NULL DEFAULT '',
          total_cost_cents INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS purchase_report_items (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          purchase_report_id INTEGER NOT NULL REFERENCES purchase_reports(id) ON DELETE CASCADE,
          inventory_item_id INTEGER REFERENCES inventory_items(id) ON DELETE SET NULL,
          item_code TEXT NOT NULL,
          item_name TEXT NOT NULL,
          unit TEXT NOT NULL,
          stock_quantity INTEGER NOT NULL,
          purchase_quantity INTEGER NOT NULL,
          unit_cost_cents INTEGER NOT NULL,
          line_total_cents INTEGER NOT NULL,
          position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS purchase_receipts (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          receipt_number TEXT UNIQUE,
          purchase_report_id INTEGER NOT NULL REFERENCES purchase_reports(id) ON DELETE RESTRICT,
          supplier TEXT NOT NULL DEFAULT '',
          notes TEXT NOT NULL DEFAULT '',
          total_cost_cents INTEGER NOT NULL DEFAULT 0,
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS purchase_receipt_items (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          purchase_receipt_id INTEGER NOT NULL REFERENCES purchase_receipts(id) ON DELETE CASCADE,
          purchase_report_item_id INTEGER REFERENCES purchase_report_items(id) ON DELETE SET NULL,
          inventory_item_id INTEGER REFERENCES inventory_items(id) ON DELETE SET NULL,
          item_code TEXT NOT NULL,
          item_name TEXT NOT NULL,
          unit TEXT NOT NULL,
          received_quantity INTEGER NOT NULL CHECK (received_quantity > 0),
          unit_cost_cents INTEGER NOT NULL DEFAULT 0,
          line_total_cents INTEGER NOT NULL DEFAULT 0,
          position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS sales (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          sale_number TEXT UNIQUE,
          customer_name TEXT NOT NULL DEFAULT '',
          notes TEXT NOT NULL DEFAULT '',
          subtotal_cents INTEGER NOT NULL DEFAULT 0,
          discount_cents INTEGER NOT NULL DEFAULT 0,
          total_cents INTEGER NOT NULL DEFAULT 0,
          status TEXT NOT NULL DEFAULT 'completed',
          cancellation_note TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
          cancelled_at TEXT
        );

        CREATE TABLE IF NOT EXISTS sale_items (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
          inventory_item_id INTEGER REFERENCES inventory_items(id) ON DELETE SET NULL,
          item_code TEXT NOT NULL,
          item_name TEXT NOT NULL,
          unit TEXT NOT NULL,
          quantity INTEGER NOT NULL CHECK (quantity > 0),
          unit_price_cents INTEGER NOT NULL DEFAULT 0,
          line_total_cents INTEGER NOT NULL DEFAULT 0,
          position INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS inventory_movements (
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          inventory_item_id INTEGER REFERENCES inventory_items(id) ON DELETE SET NULL,
          item_code TEXT NOT NULL,
          item_name TEXT NOT NULL,
          unit TEXT NOT NULL,
          movement_type TEXT NOT NULL,
          quantity_delta INTEGER NOT NULL,
          balance_after INTEGER NOT NULL,
          reference_type TEXT NOT NULL DEFAULT '',
          reference_id INTEGER,
          reference_number TEXT NOT NULL DEFAULT '',
          note TEXT NOT NULL DEFAULT '',
          created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE INDEX IF NOT EXISTS idx_work_orders_status ON work_orders(status);
        CREATE INDEX IF NOT EXISTS idx_work_orders_priority ON work_orders(priority);
        CREATE INDEX IF NOT EXISTS idx_work_orders_scheduled_date ON work_orders(scheduled_date);
        CREATE INDEX IF NOT EXISTS idx_work_orders_asset_id ON work_orders(asset_id);
        CREATE INDEX IF NOT EXISTS idx_customers_name ON customers(name);
        CREATE INDEX IF NOT EXISTS idx_tasks_work_order_id ON work_order_tasks(work_order_id);
        CREATE INDEX IF NOT EXISTS idx_accessories_work_order_id ON work_order_accessories(work_order_id);
        CREATE INDEX IF NOT EXISTS idx_quote_items_work_order_id ON work_order_quote_items(work_order_id);
        CREATE INDEX IF NOT EXISTS idx_order_parts_work_order_id ON work_order_parts(work_order_id);
        CREATE INDEX IF NOT EXISTS idx_order_parts_inventory_item_id ON work_order_parts(inventory_item_id);
        CREATE INDEX IF NOT EXISTS idx_order_labor_work_order_id ON work_order_labor(work_order_id);
        CREATE INDEX IF NOT EXISTS idx_order_payments_order ON order_payments(work_order_id, id DESC);
        CREATE INDEX IF NOT EXISTS idx_warranty_claims_original ON warranty_claims(original_order_id, id DESC);
        CREATE INDEX IF NOT EXISTS idx_warranty_claims_return ON warranty_claims(return_order_id);
        CREATE INDEX IF NOT EXISTS idx_user_activity_created ON user_activity(id DESC);
        CREATE INDEX IF NOT EXISTS idx_order_photos_order ON order_photos(work_order_id, id DESC);
        CREATE INDEX IF NOT EXISTS idx_inventory_items_name ON inventory_items(name);
        CREATE INDEX IF NOT EXISTS idx_purchase_report_items_report ON purchase_report_items(purchase_report_id);
        CREATE INDEX IF NOT EXISTS idx_purchase_receipts_report ON purchase_receipts(purchase_report_id);
        CREATE INDEX IF NOT EXISTS idx_purchase_receipt_items_receipt ON purchase_receipt_items(purchase_receipt_id);
        CREATE INDEX IF NOT EXISTS idx_sale_items_sale ON sale_items(sale_id);
        CREATE INDEX IF NOT EXISTS idx_inventory_movements_item ON inventory_movements(inventory_item_id, id DESC);
        CREATE INDEX IF NOT EXISTS idx_inventory_movements_created ON inventory_movements(id DESC);
        "#,
    )?;
    ensure_column(
        &connection,
        "purchase_report_items",
        "received_quantity",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    connection.execute(
        "INSERT INTO inventory_movements (
           inventory_item_id, item_code, item_name, unit, movement_type,
           quantity_delta, balance_after, note
         )
         SELECT i.id, i.code, i.name, i.unit, 'opening_balance', i.quantity, i.quantity,
                'Saldo existente al habilitar el historial de movimientos'
         FROM inventory_items i
         WHERE i.quantity != 0
           AND NOT EXISTS (SELECT 1 FROM inventory_movements m WHERE m.inventory_item_id = i.id)",
        [],
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "observations",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "customer_id",
        "INTEGER REFERENCES customers(id) ON DELETE SET NULL",
    )?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_work_orders_customer_id ON work_orders(customer_id)",
        [],
    )?;
    connection.execute(
        "INSERT INTO customers (name)
         SELECT DISTINCT trim(wo.requested_by) FROM work_orders wo
         WHERE trim(wo.requested_by) != ''
           AND NOT EXISTS (
             SELECT 1 FROM customers c
             WHERE c.name = trim(wo.requested_by) COLLATE NOCASE
           )",
        [],
    )?;
    connection.execute(
        "UPDATE work_orders
         SET customer_id = (
           SELECT MIN(c.id) FROM customers c
           WHERE c.name = trim(work_orders.requested_by) COLLATE NOCASE
         )
         WHERE customer_id IS NULL AND trim(requested_by) != ''",
        [],
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "delivery_tested_by",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "work_performed",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "delivery_condition",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "diagnosis",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "recommendation",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "quote_status",
        "TEXT NOT NULL DEFAULT 'draft'",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "quote_subtotal_cents",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "quote_discount_cents",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "quote_total_cents",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "authorized_by",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "authorization_method",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        &connection,
        "work_orders",
        "authorization_note",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(&connection, "work_orders", "authorization_at", "TEXT")?;
    synchronize_all_asset_statuses(&connection)?;
    sync::initialize(&connection)?;
    connection.pragma_update(None, "user_version", CURRENT_DATABASE_VERSION)?;
    Ok(())
}

fn asset_status_for_order(order_status: &str) -> Option<&'static str> {
    match order_status {
        "solicitada" | "planificada" | "pendiente_autorizacion" | "en_progreso" | "en_espera" => {
            Some("mantenimiento")
        }
        "completada" => Some("listo"),
        "entregada" => Some("entregado"),
        "cancelada" => Some("detenido"),
        _ => None,
    }
}

fn synchronize_asset_status(connection: &Connection, asset_id: i64) -> Result<(), CommandError> {
    let latest_order_status: Option<String> = connection
        .query_row(
            "SELECT status FROM work_orders WHERE asset_id = ?1 ORDER BY id DESC LIMIT 1",
            [asset_id],
            |row| row.get(0),
        )
        .optional()?;
    let status = latest_order_status
        .and_then(|value| asset_status_for_order(&value))
        .unwrap_or("operativo");
    connection.execute(
        "UPDATE assets
         SET status = ?1, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?2 AND status != ?1",
        params![status, asset_id],
    )?;
    Ok(())
}

fn synchronize_all_asset_statuses(connection: &Connection) -> Result<(), CommandError> {
    let mut statement = connection
        .prepare("SELECT DISTINCT asset_id FROM work_orders WHERE asset_id IS NOT NULL")?;
    let asset_ids = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    for asset_id in asset_ids {
        synchronize_asset_status(connection, asset_id)?;
    }
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

fn local_document_number(
    connection: &Connection,
    kind: &str,
    year: Option<&str>,
    sequence: i64,
) -> String {
    let device_prefix = connection
        .query_row(
            "SELECT value FROM sync_settings WHERE key = 'device_prefix'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .ok()
        .flatten()
        .unwrap_or_default();
    match (year, device_prefix.is_empty()) {
        (Some(year), true) => format!("{kind}-{year}-{sequence:04}"),
        (Some(year), false) => format!("{kind}-{year}-{device_prefix}-{sequence:04}"),
        (None, true) => format!("{kind}-{sequence:04}"),
        (None, false) => format!("{kind}-{device_prefix}-{sequence:04}"),
    }
}

fn map_order(row: &Row<'_>) -> rusqlite::Result<WorkOrder> {
    Ok(WorkOrder {
        id: row.get(0)?,
        order_number: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        kind: row.get(4)?,
        priority: row.get(5)?,
        status: row.get(6)?,
        asset_id: row.get(7)?,
        asset_code: row.get(8)?,
        asset_name: row.get(9)?,
        asset_category: row.get(10)?,
        asset_manufacturer: row.get(11)?,
        asset_model: row.get(12)?,
        asset_serial_number: row.get(13)?,
        location: row.get(14)?,
        assigned_to: row.get(15)?,
        requested_by: row.get(16)?,
        scheduled_date: row.get(17)?,
        estimated_hours: row.get(18)?,
        safety_notes: row.get(19)?,
        observations: row.get(20)?,
        work_performed: row.get(21)?,
        delivery_condition: row.get(22)?,
        delivery_tested_by: row.get(23)?,
        created_at: row.get(24)?,
        updated_at: row.get(25)?,
        completed_at: row.get(26)?,
        task_count: row.get(27)?,
        completed_task_count: row.get(28)?,
        customer_id: row.get(29)?,
        customer_phone: row.get(30)?,
        customer_email: row.get(31)?,
        customer_address: row.get(32)?,
        customer_company: row.get(33)?,
        customer_tax_id: row.get(34)?,
        diagnosis: row.get(35)?,
        recommendation: row.get(36)?,
        quote_status: row.get(37)?,
        quote_subtotal: row.get::<_, i64>(38)? as f64 / 100.0,
        quote_discount: row.get::<_, i64>(39)? as f64 / 100.0,
        quote_total: row.get::<_, i64>(40)? as f64 / 100.0,
        authorized_by: row.get(41)?,
        authorization_method: row.get(42)?,
        authorization_note: row.get(43)?,
        authorization_at: row.get(44)?,
        accessories: Vec::new(),
        tasks: Vec::new(),
        events: Vec::new(),
        quote_items: Vec::new(),
        parts: Vec::new(),
        labor: Vec::new(),
        parts_total: 0.0,
        labor_total: 0.0,
        resources_total: 0.0,
        payments: Vec::new(),
        financial_total: 0.0,
        payments_total: 0.0,
        balance_due: 0.0,
        credit_balance: 0.0,
        warranty: None,
        warranty_claim: None,
        warranty_returns: Vec::new(),
        photos: Vec::new(),
    })
}

fn order_tasks(connection: &Connection, order_id: i64) -> Result<Vec<WorkOrderTask>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, description, completed, completed_at, position
         FROM work_order_tasks WHERE work_order_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map([order_id], |row| {
        Ok(WorkOrderTask {
            id: row.get(0)?,
            description: row.get(1)?,
            completed: row.get::<_, i64>(2)? == 1,
            completed_at: row.get(3)?,
            position: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn order_accessories(connection: &Connection, order_id: i64) -> Result<Vec<String>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT name FROM work_order_accessories WHERE work_order_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map([order_id], |row| row.get(0))?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn order_events(
    connection: &Connection,
    order_id: i64,
) -> Result<Vec<WorkOrderEvent>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, action, previous_status, new_status, note, created_at
         FROM work_order_events WHERE work_order_id = ?1 ORDER BY id DESC",
    )?;
    let rows = statement.query_map([order_id], |row| {
        Ok(WorkOrderEvent {
            id: row.get(0)?,
            action: row.get(1)?,
            previous_status: row.get(2)?,
            new_status: row.get(3)?,
            note: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn order_quote_items(
    connection: &Connection,
    order_id: i64,
) -> Result<Vec<QuoteItem>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, kind, description, quantity, unit_price_cents, line_total_cents, position
         FROM work_order_quote_items WHERE work_order_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map([order_id], |row| {
        Ok(QuoteItem {
            id: row.get(0)?,
            kind: row.get(1)?,
            description: row.get(2)?,
            quantity: row.get(3)?,
            unit_price: row.get::<_, i64>(4)? as f64 / 100.0,
            line_total: row.get::<_, i64>(5)? as f64 / 100.0,
            position: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn order_parts(connection: &Connection, order_id: i64) -> Result<Vec<WorkOrderPart>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, inventory_item_id, item_code, item_name, unit, quantity,
                unit_price_cents, line_total_cents, position
         FROM work_order_parts WHERE work_order_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map([order_id], |row| {
        Ok(WorkOrderPart {
            id: row.get(0)?,
            inventory_item_id: row.get(1)?,
            item_code: row.get(2)?,
            item_name: row.get(3)?,
            unit: row.get(4)?,
            quantity: row.get(5)?,
            unit_price: row.get::<_, i64>(6)? as f64 / 100.0,
            line_total: row.get::<_, i64>(7)? as f64 / 100.0,
            position: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn order_labor(
    connection: &Connection,
    order_id: i64,
) -> Result<Vec<WorkOrderLabor>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, technician, description, hours, hourly_rate_cents,
                line_total_cents, position
         FROM work_order_labor WHERE work_order_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map([order_id], |row| {
        Ok(WorkOrderLabor {
            id: row.get(0)?,
            technician: row.get(1)?,
            description: row.get(2)?,
            hours: row.get(3)?,
            hourly_rate: row.get::<_, i64>(4)? as f64 / 100.0,
            line_total: row.get::<_, i64>(5)? as f64 / 100.0,
            position: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn order_payments(
    connection: &Connection,
    order_id: i64,
) -> Result<Vec<OrderPayment>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, receipt_number, payment_type, method, amount_cents / 100.0,
                reference, received_by, note, status, order_total_cents / 100.0,
                balance_before_cents / 100.0, balance_after_cents / 100.0,
                created_at, cancelled_at, cancellation_note
         FROM order_payments WHERE work_order_id = ?1 ORDER BY id DESC",
    )?;
    let rows = statement.query_map([order_id], |row| {
        Ok(OrderPayment {
            id: row.get(0)?,
            receipt_number: row.get(1)?,
            payment_type: row.get(2)?,
            method: row.get(3)?,
            amount: row.get(4)?,
            reference: row.get(5)?,
            received_by: row.get(6)?,
            note: row.get(7)?,
            status: row.get(8)?,
            order_total: row.get(9)?,
            balance_before: row.get(10)?,
            balance_after: row.get(11)?,
            created_at: row.get(12)?,
            cancelled_at: row.get(13)?,
            cancellation_note: row.get(14)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn order_warranty(
    connection: &Connection,
    order_id: i64,
) -> Result<Option<OrderWarranty>, CommandError> {
    connection
        .query_row(
            "SELECT id, warranty_number, duration_days, starts_at, expires_at,
                    coverage, exclusions,
                    CASE WHEN status = 'active' AND expires_at < date('now', 'localtime')
                         THEN 'expired' ELSE status END,
                    created_at, updated_at
             FROM order_warranties WHERE work_order_id = ?1",
            [order_id],
            |row| {
                Ok(OrderWarranty {
                    id: row.get(0)?,
                    warranty_number: row.get(1)?,
                    duration_days: row.get(2)?,
                    starts_at: row.get(3)?,
                    expires_at: row.get(4)?,
                    coverage: row.get(5)?,
                    exclusions: row.get(6)?,
                    status: row.get(7)?,
                    created_at: row.get(8)?,
                    updated_at: row.get(9)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn map_warranty_claim(row: &Row<'_>) -> rusqlite::Result<WarrantyClaim> {
    Ok(WarrantyClaim {
        id: row.get(0)?,
        claim_number: row.get(1)?,
        warranty_id: row.get(2)?,
        original_order_id: row.get(3)?,
        original_order_number: row.get(4)?,
        return_order_id: row.get(5)?,
        return_order_number: row.get(6)?,
        reported_issue: row.get(7)?,
        received_condition: row.get(8)?,
        decision: row.get(9)?,
        resolution: row.get(10)?,
        warranty_expires_at: row.get(11)?,
        within_warranty: row.get::<_, i64>(12)? == 1,
        created_at: row.get(13)?,
        resolved_at: row.get(14)?,
    })
}

const WARRANTY_CLAIM_SELECT: &str = r#"
  SELECT wc.id, wc.claim_number, wc.warranty_id, wc.original_order_id,
         COALESCE(original.order_number, ''), wc.return_order_id,
         COALESCE(returned.order_number, ''), wc.reported_issue,
         wc.received_condition, wc.decision, wc.resolution,
         wc.warranty_expires_at, wc.within_warranty, wc.created_at, wc.resolved_at
  FROM warranty_claims wc
  LEFT JOIN work_orders original ON original.id = wc.original_order_id
  LEFT JOIN work_orders returned ON returned.id = wc.return_order_id
"#;

fn warranty_claim_for_return(
    connection: &Connection,
    order_id: i64,
) -> Result<Option<WarrantyClaim>, CommandError> {
    connection
        .query_row(
            &format!("{WARRANTY_CLAIM_SELECT} WHERE wc.return_order_id = ?1"),
            [order_id],
            map_warranty_claim,
        )
        .optional()
        .map_err(Into::into)
}

fn warranty_returns(
    connection: &Connection,
    order_id: i64,
) -> Result<Vec<WarrantyClaim>, CommandError> {
    let mut statement = connection.prepare(&format!(
        "{WARRANTY_CLAIM_SELECT} WHERE wc.original_order_id = ?1 ORDER BY wc.id DESC"
    ))?;
    let rows = statement.query_map([order_id], map_warranty_claim)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn order_photos(connection: &Connection, order_id: i64) -> Result<Vec<OrderPhoto>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, work_order_id, file_name, mime_type, category, caption,
                length(data), created_by, created_at
         FROM order_photos WHERE work_order_id = ?1 ORDER BY id DESC",
    )?;
    let rows = statement.query_map([order_id], |row| {
        Ok(OrderPhoto {
            id: row.get(0)?,
            work_order_id: row.get(1)?,
            file_name: row.get(2)?,
            mime_type: row.get(3)?,
            category: row.get(4)?,
            caption: row.get(5)?,
            size_bytes: row.get(6)?,
            created_by: row.get(7)?,
            created_at: row.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_order(
    connection: &Connection,
    id: i64,
    details: bool,
) -> Result<Option<WorkOrder>, CommandError> {
    let sql = format!("{ORDER_SELECT} WHERE wo.id = ?1");
    let mut order = connection.query_row(&sql, [id], map_order).optional()?;
    if details {
        if let Some(item) = order.as_mut() {
            item.tasks = order_tasks(connection, id)?;
            item.accessories = order_accessories(connection, id)?;
            item.events = order_events(connection, id)?;
            item.quote_items = order_quote_items(connection, id)?;
            item.parts = order_parts(connection, id)?;
            item.labor = order_labor(connection, id)?;
            item.parts_total = item.parts.iter().map(|part| part.line_total).sum();
            item.labor_total = item.labor.iter().map(|line| line.line_total).sum();
            item.resources_total = item.parts_total + item.labor_total;
            item.payments = order_payments(connection, id)?;
            item.warranty = order_warranty(connection, id)?;
            item.warranty_claim = warranty_claim_for_return(connection, id)?;
            item.warranty_returns = warranty_returns(connection, id)?;
            item.photos = order_photos(connection, id)?;
            item.financial_total = if item
                .warranty_claim
                .as_ref()
                .is_some_and(|claim| claim.decision == "covered")
            {
                0.0
            } else if item.resources_total > 0.0 {
                item.resources_total
            } else {
                item.quote_total
            };
            item.payments_total = item
                .payments
                .iter()
                .filter(|payment| payment.status == "active")
                .map(|payment| payment.amount)
                .sum();
            item.balance_due = (item.financial_total - item.payments_total).max(0.0);
            item.credit_balance = (item.payments_total - item.financial_total).max(0.0);
        }
    }
    Ok(order)
}

pub fn create_order(
    connection: &mut Connection,
    data: ValidatedOrder,
) -> Result<WorkOrder, CommandError> {
    if data.kind == "garantia" {
        return Err(CommandError::message(
            "Los regresos por garantía deben crearse desde la orden original.",
        ));
    }
    let transaction = connection.transaction()?;
    let mut customer_id = data.customer_id;
    let customer_name = if let Some(customer) = data.incoming_customer.as_ref() {
        transaction.execute(
            "INSERT INTO customers (name, phone, email, address, company, tax_id, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &customer.name,
                &customer.phone,
                &customer.email,
                &customer.address,
                &customer.company,
                &customer.tax_id,
                &customer.notes
            ],
        )?;
        customer_id = Some(transaction.last_insert_rowid());
        customer.name.clone()
    } else if let Some(id) = customer_id {
        get_customer(&transaction, id)?
            .map(|customer| customer.name)
            .ok_or_else(|| CommandError::message("El cliente seleccionado ya no existe."))?
    } else {
        data.requested_by.clone()
    };
    let mut asset_id = data.asset_id;
    if let Some(asset) = data.incoming_asset.as_ref() {
        let code = if asset.code.is_empty() {
            let mut sequence: i64 = transaction.query_row(
                "SELECT COALESCE(MAX(id), 0) + 1 FROM assets",
                [],
                |row| row.get(0),
            )?;
            loop {
                let candidate = local_document_number(&transaction, "EQ", None, sequence);
                let exists: bool = transaction.query_row(
                    "SELECT EXISTS(SELECT 1 FROM assets WHERE code = ?1 COLLATE NOCASE)",
                    [&candidate],
                    |row| row.get(0),
                )?;
                if !exists {
                    break candidate;
                }
                sequence += 1;
            }
        } else {
            asset.code.clone()
        };
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM assets WHERE code = ?1 COLLATE NOCASE)",
            [&code],
            |row| row.get(0),
        )?;
        if exists {
            let mut fields = std::collections::BTreeMap::new();
            fields.insert(
                "assetCode".into(),
                "Ya existe un equipo con ese código.".into(),
            );
            return Err(CommandError {
                error: "El código interno ya está en uso.".into(),
                fields,
            });
        }
        transaction.execute(
            "INSERT INTO assets (code, name, category, location, status, manufacturer, model, serial_number)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                code,
                &asset.name,
                &asset.category,
                &asset.location,
                &asset.status,
                &asset.manufacturer,
                &asset.model,
                &asset.serial_number
            ],
        )?;
        asset_id = Some(transaction.last_insert_rowid());
    }
    transaction.execute(
        r#"INSERT INTO work_orders (
          title, description, type, priority, status, asset_id, location,
          assigned_to, requested_by, customer_id, scheduled_date, estimated_hours, safety_notes, observations
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)"#,
        params![
            data.title,
            data.description,
            data.kind,
            data.priority,
            data.status,
            asset_id,
            data.location,
            data.assigned_to,
            customer_name,
            customer_id,
            data.scheduled_date,
            data.estimated_hours,
            data.safety_notes,
            data.observations
        ],
    )?;
    let id = transaction.last_insert_rowid();
    let year: String =
        transaction.query_row("SELECT strftime('%Y', 'now', 'localtime')", [], |row| {
            row.get(0)
        })?;
    let order_number = local_document_number(&transaction, "GM", Some(&year), id);
    transaction.execute(
        "UPDATE work_orders SET order_number = ?1 WHERE id = ?2",
        params![order_number, id],
    )?;
    if let Some(asset_id) = asset_id {
        synchronize_asset_status(&transaction, asset_id)?;
    }
    for (position, task) in data.tasks.iter().enumerate() {
        transaction.execute(
            "INSERT INTO work_order_tasks (work_order_id, description, position) VALUES (?1, ?2, ?3)",
            params![id, task, position as i64],
        )?;
    }
    for (position, accessory) in data.accessories.iter().enumerate() {
        transaction.execute(
            "INSERT INTO work_order_accessories (work_order_id, name, position) VALUES (?1, ?2, ?3)",
            params![id, accessory, position as i64],
        )?;
    }
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, new_status, note)
         VALUES (?1, 'created', ?2, 'Orden creada')",
        params![id, data.status],
    )?;
    transaction.commit()?;
    get_order(connection, id, true)?.ok_or_else(CommandError::internal)
}

pub fn list_orders(
    connection: &Connection,
    filters: &OrderFilters,
) -> Result<Vec<WorkOrder>, CommandError> {
    let sql = format!(
        r#"{ORDER_SELECT}
        WHERE (?1 = '' OR ?1 = 'todos' OR wo.status = ?1)
          AND (?2 = '' OR ?2 = 'todas' OR wo.priority = ?2)
          AND (?3 = '' OR
            wo.order_number LIKE '%' || ?3 || '%' OR
            wo.title LIKE '%' || ?3 || '%' OR
            wo.description LIKE '%' || ?3 || '%' OR
            wo.observations LIKE '%' || ?3 || '%' OR
            wo.work_performed LIKE '%' || ?3 || '%' OR
            wo.diagnosis LIKE '%' || ?3 || '%' OR
            wo.recommendation LIKE '%' || ?3 || '%' OR
            wo.assigned_to LIKE '%' || ?3 || '%' OR
            c.name LIKE '%' || ?3 || '%' OR
            c.phone LIKE '%' || ?3 || '%' OR
            c.email LIKE '%' || ?3 || '%' OR
            a.name LIKE '%' || ?3 || '%' OR
            a.code LIKE '%' || ?3 || '%' OR
            wo.location LIKE '%' || ?3 || '%' OR
            EXISTS (SELECT 1 FROM work_order_accessories wa
                    WHERE wa.work_order_id = wo.id AND wa.name LIKE '%' || ?3 || '%') OR
            EXISTS (SELECT 1 FROM work_order_quote_items qi
                    WHERE qi.work_order_id = wo.id AND qi.description LIKE '%' || ?3 || '%') OR
            EXISTS (SELECT 1 FROM work_order_parts op
                    WHERE op.work_order_id = wo.id
                      AND (op.item_code LIKE '%' || ?3 || '%' OR op.item_name LIKE '%' || ?3 || '%')) OR
            EXISTS (SELECT 1 FROM work_order_labor ol
                    WHERE ol.work_order_id = wo.id
                      AND (ol.technician LIKE '%' || ?3 || '%' OR ol.description LIKE '%' || ?3 || '%')) OR
            EXISTS (SELECT 1 FROM warranty_claims wc
                    WHERE (wc.original_order_id = wo.id OR wc.return_order_id = wo.id)
                      AND (wc.claim_number LIKE '%' || ?3 || '%' OR
                           wc.reported_issue LIKE '%' || ?3 || '%' OR
                           wc.resolution LIKE '%' || ?3 || '%')))
        ORDER BY
          CASE wo.priority WHEN 'critica' THEN 1 WHEN 'alta' THEN 2 WHEN 'media' THEN 3 ELSE 4 END,
          CASE WHEN wo.scheduled_date IS NULL THEN 1 ELSE 0 END,
          wo.scheduled_date,
          wo.id DESC
        LIMIT 500"#
    );
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(
        params![filters.status, filters.priority, filters.search],
        map_order,
    )?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn status_transition_allowed(current: &str, next: &str) -> bool {
    current == next
        || matches!(
            (current, next),
            ("solicitada", "planificada")
                | ("solicitada", "en_progreso")
                | ("solicitada", "en_espera")
                | ("solicitada", "pendiente_autorizacion")
                | ("solicitada", "cancelada")
                | ("planificada", "solicitada")
                | ("planificada", "en_progreso")
                | ("planificada", "en_espera")
                | ("planificada", "pendiente_autorizacion")
                | ("planificada", "cancelada")
                | ("en_progreso", "planificada")
                | ("en_progreso", "en_espera")
                | ("en_progreso", "pendiente_autorizacion")
                | ("en_progreso", "completada")
                | ("en_progreso", "cancelada")
                | ("en_espera", "planificada")
                | ("en_espera", "en_progreso")
                | ("en_espera", "pendiente_autorizacion")
                | ("en_espera", "cancelada")
                | ("pendiente_autorizacion", "cancelada")
                | ("completada", "en_progreso")
                | ("completada", "entregada")
                | ("cancelada", "solicitada")
        )
}

fn status_label(status: &str) -> &str {
    match status {
        "solicitada" => "Recibida",
        "planificada" => "En diagnóstico",
        "pendiente_autorizacion" => "Pendiente de autorización",
        "en_progreso" => "En reparación",
        "en_espera" => "En espera",
        "completada" => "Lista para entregar",
        "entregada" => "Entregada",
        "cancelada" => "Cancelada",
        _ => status,
    }
}

fn validate_status_transition(
    connection: &Connection,
    order: &WorkOrder,
    next: &str,
    work_performed: &str,
    delivery_condition: &str,
    delivery_tested_by: &str,
) -> Result<(), CommandError> {
    if !status_transition_allowed(&order.status, next) {
        return Err(CommandError::message(format!(
            "No se puede cambiar la orden de '{}' a '{}'. Usa el siguiente paso del flujo.",
            status_label(&order.status),
            status_label(next)
        )));
    }
    if next == "pendiente_autorizacion" && order.quote_status != "pending" {
        return Err(CommandError::message(
            "Usa 'Solicitar autorización' dentro de Diagnóstico y cotización.",
        ));
    }
    if next == "en_progreso" && order.quote_total > 0.0 && order.quote_status != "approved" {
        return Err(CommandError::message(
            "La cotización debe estar aprobada antes de iniciar la reparación.",
        ));
    }
    if next == "completada" && order.status != "completada" {
        if order.quote_total > 0.0 && order.quote_status != "approved" {
            return Err(CommandError::message(
                "La cotización debe estar aprobada antes de cerrar el trabajo.",
            ));
        }
        let pending: i64 = connection.query_row(
            "SELECT COUNT(*) FROM work_order_tasks WHERE work_order_id = ?1 AND completed = 0",
            [order.id],
            |row| row.get(0),
        )?;
        if pending > 0 {
            return Err(CommandError::message(
                "Completa todas las tareas antes de marcar la orden como lista para entregar.",
            ));
        }
        if work_performed.trim().is_empty() {
            return Err(CommandError::message(
                "Registra el trabajo realizado antes de marcar la orden como lista para entregar.",
            ));
        }
    }
    if next == "entregada" && order.status != "entregada" {
        if work_performed.trim().is_empty()
            || delivery_condition.trim().is_empty()
            || delivery_tested_by.trim().is_empty()
        {
            return Err(CommandError::message(
                "Completa el trabajo realizado, el estado de entrega y quién probó el equipo antes de entregarlo.",
            ));
        }
    }
    Ok(())
}

pub fn update_order(
    connection: &mut Connection,
    id: i64,
    patch: ValidatedOrderPatch,
) -> Result<Option<WorkOrder>, CommandError> {
    let current = match get_order(connection, id, false)? {
        Some(order) => order,
        None => return Ok(None),
    };
    let ValidatedOrderPatch {
        title,
        description,
        kind,
        priority,
        status,
        asset_id,
        customer_id,
        location,
        assigned_to,
        requested_by,
        scheduled_date,
        estimated_hours,
        safety_notes,
        observations,
        accessories,
        tasks,
        work_performed,
        delivery_condition,
        delivery_tested_by,
    } = patch;

    if kind.as_deref() == Some("garantia") && current.kind != "garantia" {
        return Err(CommandError::message(
            "Los regresos por garantía deben crearse desde la orden original.",
        ));
    }
    if current.kind == "garantia" && kind.as_deref().is_some_and(|value| value != "garantia") {
        return Err(CommandError::message(
            "Una orden de regreso debe conservar su tipo para mantener la trazabilidad.",
        ));
    }

    if matches!(
        current.status.as_str(),
        "pendiente_autorizacion" | "completada" | "entregada" | "cancelada"
    ) {
        if let Some(replacement_tasks) = tasks.as_ref() {
            let current_tasks = order_tasks(connection, id)?
                .into_iter()
                .map(|task| task.description)
                .collect::<Vec<_>>();
            if &current_tasks != replacement_tasks {
                return Err(CommandError::message(
                    "Reabre la orden antes de cambiar su lista de trabajo.",
                ));
            }
        }
    }

    if let Some(Some(asset_id)) = asset_id {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM assets WHERE id = ?1)",
            [asset_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(CommandError::message(
                "El equipo seleccionado ya no existe.",
            ));
        }
    }
    let customer_name = if let Some(customer_id) = customer_id {
        Some(
            get_customer(connection, customer_id)?
                .map(|customer| customer.name)
                .ok_or_else(|| CommandError::message("El cliente seleccionado ya no existe."))?,
        )
    } else {
        None
    };
    let effective_work = work_performed
        .as_deref()
        .unwrap_or(current.work_performed.as_str());
    let effective_condition = delivery_condition
        .as_deref()
        .unwrap_or(current.delivery_condition.as_str());
    let effective_tester = delivery_tested_by
        .as_deref()
        .unwrap_or(current.delivery_tested_by.as_str());
    if let Some(next) = status.as_deref() {
        validate_status_transition(
            connection,
            &current,
            next,
            effective_work,
            effective_condition,
            effective_tester,
        )?;
    }

    let next_asset_id = asset_id.unwrap_or(current.asset_id);
    let mut sets: Vec<&str> = Vec::new();
    let mut values: Vec<Box<dyn ToSql>> = Vec::new();
    macro_rules! add_value {
        ($field:expr, $column:literal) => {
            if let Some(value) = $field {
                sets.push(concat!($column, " = ?"));
                values.push(Box::new(value));
            }
        };
    }
    add_value!(title, "title");
    add_value!(description, "description");
    add_value!(kind, "type");
    add_value!(priority, "priority");
    add_value!(asset_id, "asset_id");
    add_value!(location, "location");
    add_value!(assigned_to, "assigned_to");
    if let Some(customer_id) = customer_id {
        sets.push("customer_id = ?");
        values.push(Box::new(customer_id));
        sets.push("requested_by = ?");
        values.push(Box::new(customer_name.unwrap_or_default()));
    } else {
        add_value!(requested_by, "requested_by");
    }
    add_value!(scheduled_date, "scheduled_date");
    add_value!(estimated_hours, "estimated_hours");
    add_value!(safety_notes, "safety_notes");
    add_value!(observations, "observations");
    add_value!(work_performed, "work_performed");
    add_value!(delivery_condition, "delivery_condition");
    add_value!(delivery_tested_by, "delivery_tested_by");

    let new_status = status;
    if let Some(status) = new_status.as_ref() {
        sets.push("status = ?");
        values.push(Box::new(status.clone()));
        let is_finished = matches!(status.as_str(), "completada" | "entregada");
        let was_finished = matches!(current.status.as_str(), "completada" | "entregada");
        if is_finished && !was_finished {
            sets.push("completed_at = CURRENT_TIMESTAMP");
        } else if !is_finished {
            sets.push("completed_at = NULL");
        }
    }

    let has_related_changes = accessories.is_some() || tasks.is_some();
    if sets.is_empty() && !has_related_changes {
        return get_order(connection, id, true);
    }
    let transaction = connection.transaction()?;
    if !sets.is_empty() {
        sets.push("updated_at = CURRENT_TIMESTAMP");
        let placeholders = sets
            .iter()
            .scan(0usize, |parameter, set| {
                if set.contains(" = ?") {
                    *parameter += 1;
                    Some(set.replacen('?', &format!("?{}", *parameter), 1))
                } else {
                    Some((*set).to_owned())
                }
            })
            .collect::<Vec<_>>();
        let id_parameter = values.len() + 1;
        let sql = format!(
            "UPDATE work_orders SET {} WHERE id = ?{id_parameter}",
            placeholders.join(", ")
        );
        values.push(Box::new(id));
        let parameters: Vec<&dyn ToSql> = values.iter().map(|value| value.as_ref()).collect();
        transaction.execute(&sql, parameters.as_slice())?;
    } else {
        transaction.execute(
            "UPDATE work_orders SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
            [id],
        )?;
    }

    if let Some(accessories) = accessories {
        transaction.execute(
            "DELETE FROM work_order_accessories WHERE work_order_id = ?1",
            [id],
        )?;
        for (position, accessory) in accessories.iter().enumerate() {
            transaction.execute(
                "INSERT INTO work_order_accessories (work_order_id, name, position) VALUES (?1, ?2, ?3)",
                params![id, accessory, position as i64],
            )?;
        }
    }
    if let Some(tasks) = tasks {
        let previous = order_tasks(&transaction, id)?;
        transaction.execute(
            "DELETE FROM work_order_tasks WHERE work_order_id = ?1",
            [id],
        )?;
        for (position, description) in tasks.iter().enumerate() {
            let matched = previous
                .iter()
                .find(|task| task.description.eq_ignore_ascii_case(description));
            let completed = matched.is_some_and(|task| task.completed);
            let completed_at = matched.and_then(|task| task.completed_at.as_deref());
            transaction.execute(
                "INSERT INTO work_order_tasks
                   (work_order_id, description, completed, completed_at, position)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    id,
                    description,
                    completed as i64,
                    completed_at,
                    position as i64
                ],
            )?;
        }
    }

    if current.asset_id != next_asset_id || new_status.is_some() {
        if let Some(old_asset_id) = current.asset_id {
            synchronize_asset_status(&transaction, old_asset_id)?;
        }
        if let Some(new_asset_id) = next_asset_id {
            if Some(new_asset_id) != current.asset_id {
                synchronize_asset_status(&transaction, new_asset_id)?;
            }
        }
    }
    if let Some(status) = new_status {
        if status != current.status {
            transaction.execute(
                "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
                 VALUES (?1, 'status_changed', ?2, ?3, 'Estado actualizado')",
                params![id, current.status, status],
            )?;
        } else {
            transaction.execute(
                "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
                 VALUES (?1, 'updated', ?2, ?2, 'Orden actualizada')",
                params![id, current.status],
            )?;
        }
    } else {
        transaction.execute(
            "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
             VALUES (?1, 'updated', ?2, ?2, 'Datos de la orden actualizados')",
            params![id, current.status],
        )?;
    }
    transaction.commit()?;
    get_order(connection, id, true)
}

pub fn save_diagnosis_quote(
    connection: &mut Connection,
    id: i64,
    quote: ValidatedDiagnosisQuote,
) -> Result<Option<WorkOrder>, CommandError> {
    let current = match get_order(connection, id, false)? {
        Some(order) => order,
        None => return Ok(None),
    };
    if matches!(
        current.status.as_str(),
        "completada" | "entregada" | "cancelada"
    ) {
        return Err(CommandError::message(
            "Reabre la orden antes de modificar su diagnóstico o cotización.",
        ));
    }
    let subtotal_cents = quote
        .items
        .iter()
        .try_fold(0_i64, |total, item| {
            total.checked_add(item.line_total_cents)
        })
        .ok_or_else(|| CommandError::message("El total de la cotización es demasiado alto."))?;
    if quote.discount_cents > subtotal_cents {
        return Err(CommandError::message(
            "El descuento no puede ser mayor que el subtotal.",
        ));
    }
    let total_cents = subtotal_cents - quote.discount_cents;
    let reset_status = current.quote_status != "draft";
    let next_order_status = if current.status == "pendiente_autorizacion"
        || (current.status == "en_progreso" && total_cents > 0)
    {
        "planificada"
    } else {
        current.status.as_str()
    };
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE work_orders
         SET diagnosis = ?1, recommendation = ?2, quote_status = 'draft',
             quote_subtotal_cents = ?3, quote_discount_cents = ?4, quote_total_cents = ?5,
             authorized_by = '', authorization_method = '', authorization_note = '',
             authorization_at = NULL, status = ?6, completed_at = CASE WHEN ?6 = 'completada' THEN completed_at ELSE NULL END,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = ?7",
        params![
            quote.diagnosis,
            quote.recommendation,
            subtotal_cents,
            quote.discount_cents,
            total_cents,
            next_order_status,
            id
        ],
    )?;
    transaction.execute(
        "DELETE FROM work_order_quote_items WHERE work_order_id = ?1",
        [id],
    )?;
    for (position, item) in quote.items.iter().enumerate() {
        transaction.execute(
            "INSERT INTO work_order_quote_items
               (work_order_id, kind, description, quantity, unit_price_cents, line_total_cents, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                item.kind,
                item.description,
                item.quantity,
                item.unit_price_cents,
                item.line_total_cents,
                position as i64
            ],
        )?;
    }
    let note = if reset_status {
        "Cotización modificada; se requiere una nueva autorización"
    } else {
        "Diagnóstico y cotización guardados"
    };
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, 'quote_saved', ?2, ?3, ?4)",
        params![id, current.status, next_order_status, note],
    )?;
    if let Some(asset_id) = current.asset_id {
        synchronize_asset_status(&transaction, asset_id)?;
    }
    transaction.commit()?;
    get_order(connection, id, true)
}

pub fn set_quote_decision(
    connection: &mut Connection,
    id: i64,
    decision: ValidatedQuoteDecision,
) -> Result<Option<WorkOrder>, CommandError> {
    let current = match get_order(connection, id, true)? {
        Some(order) => order,
        None => return Ok(None),
    };
    if matches!(
        current.status.as_str(),
        "completada" | "entregada" | "cancelada"
    ) {
        return Err(CommandError::message(
            "Reabre la orden antes de cambiar la autorización de la cotización.",
        ));
    }
    if decision.decision == "pending" {
        if current.diagnosis.trim().len() < 5
            || current.quote_items.is_empty()
            || current.quote_total <= 0.0
        {
            return Err(CommandError::message(
                "Guarda un diagnóstico y una cotización con total mayor que cero antes de solicitar autorización.",
            ));
        }
    } else if current.quote_status != "pending" {
        return Err(CommandError::message(
            "Primero marca la cotización como pendiente de autorización.",
        ));
    }

    let next_order_status = match decision.decision.as_str() {
        "pending" | "rejected" => "pendiente_autorizacion",
        "approved" => "en_progreso",
        _ => unreachable!(),
    };
    let action = match decision.decision.as_str() {
        "pending" => "quote_requested",
        "approved" => "quote_approved",
        "rejected" => "quote_rejected",
        _ => unreachable!(),
    };
    let event_note = match decision.decision.as_str() {
        "pending" => "Cotización enviada a autorización".to_owned(),
        "approved" => format!(
            "Autorizada por {} vía {}{}",
            decision.authorized_by,
            decision.method,
            if decision.note.is_empty() {
                String::new()
            } else {
                format!(": {}", decision.note)
            }
        ),
        "rejected" => format!(
            "Rechazada por {} vía {}{}",
            decision.authorized_by,
            decision.method,
            if decision.note.is_empty() {
                String::new()
            } else {
                format!(": {}", decision.note)
            }
        ),
        _ => unreachable!(),
    };
    let transaction = connection.transaction()?;
    if decision.decision == "pending" {
        transaction.execute(
            "UPDATE work_orders SET quote_status = 'pending', status = ?1,
             authorized_by = '', authorization_method = '', authorization_note = '', authorization_at = NULL,
             completed_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![next_order_status, id],
        )?;
    } else {
        transaction.execute(
            "UPDATE work_orders SET quote_status = ?1, status = ?2, authorized_by = ?3,
             authorization_method = ?4, authorization_note = ?5, authorization_at = CURRENT_TIMESTAMP,
             completed_at = NULL, updated_at = CURRENT_TIMESTAMP WHERE id = ?6",
            params![
                decision.decision,
                next_order_status,
                decision.authorized_by,
                decision.method,
                decision.note,
                id
            ],
        )?;
    }
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, action, current.status, next_order_status, event_note],
    )?;
    if let Some(asset_id) = current.asset_id {
        synchronize_asset_status(&transaction, asset_id)?;
    }
    transaction.commit()?;
    get_order(connection, id, true)
}

pub fn save_order_resources(
    connection: &mut Connection,
    id: i64,
    resources: ValidatedOrderResources,
) -> Result<Option<WorkOrder>, CommandError> {
    let current = match get_order(connection, id, true)? {
        Some(order) => order,
        None => return Ok(None),
    };
    if matches!(
        current.status.as_str(),
        "pendiente_autorizacion" | "completada" | "entregada" | "cancelada"
    ) {
        return Err(CommandError::message(
            "Los consumos sólo pueden modificarse mientras la orden está activa y autorizada.",
        ));
    }

    let parts_total_cents = resources
        .parts
        .iter()
        .try_fold(0_i64, |total, part| {
            total.checked_add(part.line_total_cents)
        })
        .ok_or_else(|| CommandError::message("El total de repuestos es demasiado alto."))?;
    let labor_total_cents = resources
        .labor
        .iter()
        .try_fold(0_i64, |total, line| {
            total.checked_add(line.line_total_cents)
        })
        .ok_or_else(|| CommandError::message("El total de mano de obra es demasiado alto."))?;
    let part_count = resources.parts.len();
    let labor_count = resources.labor.len();
    let mut previous_quantities = std::collections::BTreeMap::new();
    for previous in &current.parts {
        if let Some(item_id) = previous.inventory_item_id {
            previous_quantities.insert(item_id, previous.quantity);
        }
    }
    let new_quantities = resources
        .parts
        .iter()
        .map(|part| (part.inventory_item_id, part.quantity))
        .collect::<std::collections::BTreeMap<_, _>>();
    let transaction = connection.transaction()?;

    for previous in &current.parts {
        if let Some(inventory_item_id) = previous.inventory_item_id {
            transaction.execute(
                "UPDATE inventory_items
                 SET quantity = quantity + ?1, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?2",
                params![previous.quantity, inventory_item_id],
            )?;
        }
    }
    transaction.execute(
        "DELETE FROM work_order_parts WHERE work_order_id = ?1",
        [id],
    )?;
    transaction.execute(
        "DELETE FROM work_order_labor WHERE work_order_id = ?1",
        [id],
    )?;

    for (position, part) in resources.parts.iter().enumerate() {
        let inventory = transaction
            .query_row(
                "SELECT code, name, unit, quantity FROM inventory_items WHERE id = ?1",
                [part.inventory_item_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let (code, name, unit, available) = inventory.ok_or_else(|| {
            CommandError::message("Uno de los repuestos seleccionados ya no existe.")
        })?;
        if available < part.quantity {
            return Err(CommandError::message(format!(
                "No hay existencia suficiente de '{}'. Disponible: {} {}.",
                name, available, unit
            )));
        }
        transaction.execute(
            "UPDATE inventory_items
             SET quantity = quantity - ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2 AND quantity >= ?1",
            params![part.quantity, part.inventory_item_id],
        )?;
        transaction.execute(
            "INSERT INTO work_order_parts
               (work_order_id, inventory_item_id, item_code, item_name, unit, quantity,
                unit_price_cents, line_total_cents, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id,
                part.inventory_item_id,
                code,
                name,
                unit,
                part.quantity,
                part.unit_price_cents,
                part.line_total_cents,
                position as i64
            ],
        )?;
    }
    for (position, line) in resources.labor.iter().enumerate() {
        transaction.execute(
            "INSERT INTO work_order_labor
               (work_order_id, technician, description, hours, hourly_rate_cents,
                line_total_cents, position)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                line.technician,
                line.description,
                line.hours,
                line.hourly_rate_cents,
                line.line_total_cents,
                position as i64
            ],
        )?;
    }
    let movement_item_ids = previous_quantities
        .keys()
        .chain(new_quantities.keys())
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    for item_id in movement_item_ids {
        let previous_quantity = previous_quantities.get(&item_id).copied().unwrap_or(0);
        let new_quantity = new_quantities.get(&item_id).copied().unwrap_or(0);
        let stock_delta = previous_quantity - new_quantity;
        if stock_delta != 0 {
            record_inventory_movement(
                &transaction,
                item_id,
                "work_order_adjustment",
                stock_delta,
                "work_order",
                Some(id),
                &current.order_number,
                "Ajuste de repuestos utilizados en la orden",
            )?;
        }
    }
    let total = (parts_total_cents + labor_total_cents) as f64 / 100.0;
    let note = format!(
        "Consumos actualizados: {part_count} repuestos, {labor_count} trabajos, total Q{total:.2}"
    );
    transaction.execute(
        "UPDATE work_orders SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [id],
    )?;
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, 'resources_saved', ?2, ?2, ?3)",
        params![id, current.status, note],
    )?;
    transaction.commit()?;
    get_order(connection, id, true)
}

fn get_order_payment(
    connection: &Connection,
    order_id: i64,
    payment_id: i64,
) -> Result<Option<OrderPayment>, CommandError> {
    Ok(order_payments(connection, order_id)?
        .into_iter()
        .find(|payment| payment.id == payment_id))
}

pub fn create_order_payment(
    connection: &mut Connection,
    order_id: i64,
    payment: ValidatedOrderPayment,
) -> Result<Option<OrderPayment>, CommandError> {
    let order = match get_order(connection, order_id, true)? {
        Some(order) => order,
        None => return Ok(None),
    };
    if order.status == "cancelada" {
        return Err(CommandError::message(
            "No se pueden registrar pagos en una orden cancelada.",
        ));
    }
    if order
        .warranty_claim
        .as_ref()
        .is_some_and(|claim| claim.decision == "covered")
    {
        return Err(CommandError::message(
            "Esta orden está cubierta por garantía y no tiene saldo por cobrar.",
        ));
    }
    let order_total_cents = (order.financial_total * 100.0).round() as i64;
    let payments_total_cents = (order.payments_total * 100.0).round() as i64;
    let balance_before_cents = order_total_cents.saturating_sub(payments_total_cents);
    if order_total_cents > 0 && payment.amount_cents > balance_before_cents {
        return Err(CommandError::message(format!(
            "El pago supera el saldo pendiente de Q{:.2}.",
            balance_before_cents as f64 / 100.0
        )));
    }
    let balance_after_cents = balance_before_cents.saturating_sub(payment.amount_cents);
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO order_payments (
           work_order_id, payment_type, method, amount_cents, reference,
           received_by, note, order_total_cents, balance_before_cents,
           balance_after_cents
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            order_id,
            payment.payment_type,
            payment.method,
            payment.amount_cents,
            payment.reference,
            payment.received_by,
            payment.note,
            order_total_cents,
            balance_before_cents,
            balance_after_cents
        ],
    )?;
    let payment_id = transaction.last_insert_rowid();
    let year: String =
        transaction.query_row("SELECT strftime('%Y', 'now', 'localtime')", [], |row| {
            row.get(0)
        })?;
    let receipt_number = local_document_number(&transaction, "RP", Some(&year), payment_id);
    transaction.execute(
        "UPDATE order_payments SET receipt_number = ?1 WHERE id = ?2",
        params![receipt_number, payment_id],
    )?;
    transaction.execute(
        "UPDATE work_orders SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [order_id],
    )?;
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, 'payment_received', ?2, ?2, ?3)",
        params![
            order_id,
            order.status,
            format!(
                "{} por Q{:.2} · {}",
                if payment.payment_type == "advance" {
                    "Anticipo recibido"
                } else {
                    "Pago recibido"
                },
                payment.amount_cents as f64 / 100.0,
                receipt_number
            )
        ],
    )?;
    transaction.commit()?;
    get_order_payment(connection, order_id, payment_id)
}

pub fn cancel_order_payment(
    connection: &mut Connection,
    order_id: i64,
    payment_id: i64,
    note: String,
) -> Result<Option<OrderPayment>, CommandError> {
    let order = match get_order(connection, order_id, true)? {
        Some(order) => order,
        None => return Ok(None),
    };
    let current = match order
        .payments
        .iter()
        .find(|payment| payment.id == payment_id)
    {
        Some(payment) => payment,
        None => return Ok(None),
    };
    if current.status == "cancelled" {
        return Err(CommandError::message("El pago ya está anulado."));
    }
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE order_payments
         SET status = 'cancelled', cancelled_at = CURRENT_TIMESTAMP, cancellation_note = ?1
         WHERE id = ?2 AND work_order_id = ?3 AND status = 'active'",
        params![note, payment_id, order_id],
    )?;
    transaction.execute(
        "UPDATE work_orders SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [order_id],
    )?;
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, 'payment_cancelled', ?2, ?2, ?3)",
        params![
            order_id,
            order.status,
            format!("Pago anulado: {} · {}", current.receipt_number, note)
        ],
    )?;
    transaction.commit()?;
    get_order_payment(connection, order_id, payment_id)
}

pub fn save_order_warranty(
    connection: &mut Connection,
    order_id: i64,
    warranty: ValidatedWarranty,
) -> Result<Option<OrderWarranty>, CommandError> {
    let order = match get_order(connection, order_id, true)? {
        Some(order) => order,
        None => return Ok(None),
    };
    if !matches!(order.status.as_str(), "completada" | "entregada") {
        return Err(CommandError::message(
            "La garantía se define cuando la reparación está lista o entregada.",
        ));
    }
    let expires_at: String = connection.query_row(
        "SELECT date(?1, ?2)",
        params![
            warranty.starts_at,
            format!("+{} days", warranty.duration_days)
        ],
        |row| row.get(0),
    )?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO order_warranties (
           work_order_id, duration_days, starts_at, expires_at, coverage, exclusions
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(work_order_id) DO UPDATE SET
           duration_days = excluded.duration_days,
           starts_at = excluded.starts_at,
           expires_at = excluded.expires_at,
           coverage = excluded.coverage,
           exclusions = excluded.exclusions,
           status = 'active',
           updated_at = CURRENT_TIMESTAMP",
        params![
            order_id,
            warranty.duration_days,
            warranty.starts_at,
            expires_at,
            warranty.coverage,
            warranty.exclusions
        ],
    )?;
    let (warranty_id, current_number): (i64, Option<String>) = transaction.query_row(
        "SELECT id, warranty_number FROM order_warranties WHERE work_order_id = ?1",
        [order_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let warranty_number = if current_number.as_deref().unwrap_or("").is_empty() {
        let year: String =
            transaction.query_row("SELECT strftime('%Y', 'now', 'localtime')", [], |row| {
                row.get(0)
            })?;
        let number = local_document_number(&transaction, "GT", Some(&year), warranty_id);
        transaction.execute(
            "UPDATE order_warranties SET warranty_number = ?1 WHERE id = ?2",
            params![number, warranty_id],
        )?;
        number
    } else {
        current_number.unwrap_or_default()
    };
    transaction.execute(
        "UPDATE work_orders SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [order_id],
    )?;
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, 'warranty_saved', ?2, ?2, ?3)",
        params![
            order_id,
            order.status,
            format!("Garantía {} vigente hasta {}", warranty_number, expires_at)
        ],
    )?;
    transaction.commit()?;
    order_warranty(connection, order_id)
}

pub fn create_warranty_return(
    connection: &mut Connection,
    original_order_id: i64,
    data: ValidatedWarrantyReturn,
) -> Result<Option<WorkOrder>, CommandError> {
    let original = match get_order(connection, original_order_id, true)? {
        Some(order) => order,
        None => return Ok(None),
    };
    let warranty = original
        .warranty
        .as_ref()
        .ok_or_else(|| CommandError::message("Esta orden no tiene una garantía registrada."))?;
    if warranty.status == "void" {
        return Err(CommandError::message(
            "La garantía de esta orden está anulada.",
        ));
    }
    let asset_id = original.asset_id.ok_or_else(|| {
        CommandError::message("La orden original ya no tiene un equipo relacionado.")
    })?;
    let customer_id = original.customer_id.ok_or_else(|| {
        CommandError::message("La orden original ya no tiene un cliente relacionado.")
    })?;
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO work_orders (
           title, description, type, priority, status, asset_id, location,
           assigned_to, requested_by, customer_id, observations
         ) VALUES (?1, ?2, 'garantia', ?3, 'solicitada', ?4, ?5, '', ?6, ?7, ?8)",
        params![
            format!(
                "Regreso por garantía · {}",
                original.asset_name.as_deref().unwrap_or(&original.title)
            ),
            data.reported_issue,
            data.priority,
            asset_id,
            original.location,
            original.requested_by,
            customer_id,
            data.received_condition
        ],
    )?;
    let return_order_id = transaction.last_insert_rowid();
    let year: String =
        transaction.query_row("SELECT strftime('%Y', 'now', 'localtime')", [], |row| {
            row.get(0)
        })?;
    let return_order_number =
        local_document_number(&transaction, "GM", Some(&year), return_order_id);
    transaction.execute(
        "UPDATE work_orders SET order_number = ?1 WHERE id = ?2",
        params![return_order_number, return_order_id],
    )?;
    for (position, task) in [
        "Verificar la falla reportada",
        "Evaluar la cobertura de garantía",
        "Realizar pruebas finales",
    ]
    .iter()
    .enumerate()
    {
        transaction.execute(
            "INSERT INTO work_order_tasks (work_order_id, description, position)
             VALUES (?1, ?2, ?3)",
            params![return_order_id, task, position as i64],
        )?;
    }
    for (position, accessory) in data.accessories.iter().enumerate() {
        transaction.execute(
            "INSERT INTO work_order_accessories (work_order_id, name, position)
             VALUES (?1, ?2, ?3)",
            params![return_order_id, accessory, position as i64],
        )?;
    }
    let within_warranty: bool = transaction.query_row(
        "SELECT date('now', 'localtime') BETWEEN date(?1) AND date(?2)",
        params![warranty.starts_at, warranty.expires_at],
        |row| row.get(0),
    )?;
    transaction.execute(
        "INSERT INTO warranty_claims (
           warranty_id, original_order_id, return_order_id, reported_issue,
           received_condition, warranty_expires_at, within_warranty
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            warranty.id,
            original_order_id,
            return_order_id,
            data.reported_issue,
            data.received_condition,
            warranty.expires_at,
            within_warranty as i64
        ],
    )?;
    let claim_id = transaction.last_insert_rowid();
    let claim_number = local_document_number(&transaction, "RG", Some(&year), claim_id);
    transaction.execute(
        "UPDATE warranty_claims SET claim_number = ?1 WHERE id = ?2",
        params![claim_number, claim_id],
    )?;
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, new_status, note)
         VALUES (?1, 'warranty_return_created', 'solicitada', ?2)",
        params![
            return_order_id,
            format!(
                "Regreso {} vinculado con {}",
                claim_number, original.order_number
            )
        ],
    )?;
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, 'warranty_return_received', ?2, ?2, ?3)",
        params![
            original_order_id,
            original.status,
            format!("Equipo recibido nuevamente como {}", claim_number)
        ],
    )?;
    synchronize_asset_status(&transaction, asset_id)?;
    transaction.commit()?;
    get_order(connection, return_order_id, true)
}

pub fn resolve_warranty_claim(
    connection: &mut Connection,
    return_order_id: i64,
    decision: ValidatedWarrantyDecision,
) -> Result<Option<WorkOrder>, CommandError> {
    let order = match get_order(connection, return_order_id, true)? {
        Some(order) => order,
        None => return Ok(None),
    };
    let claim = order.warranty_claim.as_ref().ok_or_else(|| {
        CommandError::message("Esta orden no corresponde a un regreso por garantía.")
    })?;
    let action = match decision.decision.as_str() {
        "covered" => "warranty_covered",
        "not_covered" => "warranty_not_covered",
        _ => "warranty_pending",
    };
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE warranty_claims
         SET decision = ?1, resolution = ?2,
             resolved_at = CASE WHEN ?1 = 'pending' THEN NULL ELSE CURRENT_TIMESTAMP END
         WHERE id = ?3",
        params![decision.decision, decision.resolution, claim.id],
    )?;
    transaction.execute(
        "UPDATE work_orders SET updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [return_order_id],
    )?;
    transaction.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, ?2, ?3, ?3, ?4)",
        params![
            return_order_id,
            action,
            order.status,
            if decision.resolution.is_empty() {
                "Cobertura pendiente de evaluación".to_owned()
            } else {
                decision.resolution
            }
        ],
    )?;
    transaction.commit()?;
    get_order(connection, return_order_id, true)
}

pub fn add_order_photo(
    connection: &Connection,
    order_id: i64,
    photo: ValidatedOrderPhoto,
    created_by: &str,
) -> Result<Option<OrderPhoto>, CommandError> {
    let order = match get_order(connection, order_id, false)? {
        Some(order) => order,
        None => return Ok(None),
    };
    connection.execute(
        "INSERT INTO order_photos
           (work_order_id, file_name, mime_type, category, caption, data, created_by)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            order_id,
            photo.file_name,
            photo.mime_type,
            photo.category,
            photo.caption,
            photo.data,
            created_by
        ],
    )?;
    let photo_id = connection.last_insert_rowid();
    connection.execute(
        "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
         VALUES (?1, 'photo_added', ?2, ?2, ?3)",
        params![
            order_id,
            order.status,
            format!("Fotografía agregada por {created_by}")
        ],
    )?;
    Ok(order_photos(connection, order_id)?
        .into_iter()
        .find(|item| item.id == photo_id))
}

pub fn get_order_photo_data(
    connection: &Connection,
    photo_id: i64,
) -> Result<Option<Vec<u8>>, CommandError> {
    connection
        .query_row(
            "SELECT data FROM order_photos WHERE id = ?1",
            [photo_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
}

pub fn delete_order_photo(
    connection: &Connection,
    order_id: i64,
    photo_id: i64,
    deleted_by: &str,
) -> Result<bool, CommandError> {
    let order = match get_order(connection, order_id, false)? {
        Some(order) => order,
        None => return Ok(false),
    };
    let deleted = connection.execute(
        "DELETE FROM order_photos WHERE id = ?1 AND work_order_id = ?2",
        params![photo_id, order_id],
    )?;
    if deleted > 0 {
        connection.execute(
            "INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
             VALUES (?1, 'photo_deleted', ?2, ?2, ?3)",
            params![
                order_id,
                order.status,
                format!("Fotografía eliminada por {deleted_by}")
            ],
        )?;
    }
    Ok(deleted > 0)
}

pub fn set_task_completed(
    connection: &Connection,
    order_id: i64,
    task_id: i64,
    completed: bool,
) -> Result<Option<WorkOrder>, CommandError> {
    let order_status: Option<String> = connection
        .query_row(
            "SELECT status FROM work_orders WHERE id = ?1",
            [order_id],
            |row| row.get(0),
        )
        .optional()?;
    if order_status.as_deref().is_some_and(|status| {
        matches!(
            status,
            "pendiente_autorizacion" | "completada" | "entregada" | "cancelada"
        )
    }) {
        return Err(CommandError::message(
            "La lista de trabajo no se puede modificar en el estado actual.",
        ));
    }
    let changes = connection.execute(
        "UPDATE work_order_tasks
         SET completed = ?1, completed_at = CASE WHEN ?1 = 1 THEN CURRENT_TIMESTAMP ELSE NULL END
         WHERE id = ?2 AND work_order_id = ?3",
        params![completed as i64, task_id, order_id],
    )?;
    if changes == 0 {
        return Ok(None);
    }
    connection.execute(
        "INSERT INTO work_order_events (work_order_id, action, note)
         VALUES (?1, ?2, 'Lista de trabajo actualizada')",
        params![
            order_id,
            if completed {
                "task_completed"
            } else {
                "task_reopened"
            }
        ],
    )?;
    get_order(connection, order_id, true)
}

fn map_asset(row: &Row<'_>) -> rusqlite::Result<Asset> {
    Ok(Asset {
        id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        category: row.get(3)?,
        location: row.get(4)?,
        status: row.get(5)?,
        manufacturer: row.get(6)?,
        model: row.get(7)?,
        serial_number: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        order_count: row.get(11)?,
        open_order_count: row.get(12)?,
        customer_id: row.get(13)?,
        customer_name: row.get(14)?,
    })
}

pub fn list_assets(connection: &Connection) -> Result<Vec<Asset>, CommandError> {
    let mut statement = connection.prepare(
        r#"SELECT a.id, a.code, a.name, a.category, a.location, a.status, a.manufacturer,
                  a.model, a.serial_number, a.created_at, a.updated_at,
                  COUNT(wo.id),
                   COALESCE(SUM(CASE WHEN wo.status NOT IN ('entregada', 'cancelada') THEN 1 ELSE 0 END), 0),
                   (SELECT wo2.customer_id FROM work_orders wo2
                      WHERE wo2.asset_id = a.id AND wo2.customer_id IS NOT NULL
                      ORDER BY wo2.id DESC LIMIT 1),
                   COALESCE((SELECT c.name FROM work_orders wo2
                      JOIN customers c ON c.id = wo2.customer_id
                      WHERE wo2.asset_id = a.id
                      ORDER BY wo2.id DESC LIMIT 1), '')
           FROM assets a
           LEFT JOIN work_orders wo ON wo.asset_id = a.id
           GROUP BY a.id
           ORDER BY a.name COLLATE NOCASE"#,
    )?;
    let rows = statement.query_map([], map_asset)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn create_asset(connection: &Connection, data: ValidatedAsset) -> Result<Asset, CommandError> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM assets WHERE code = ?1 COLLATE NOCASE)",
        [&data.code],
        |row| row.get(0),
    )?;
    if exists {
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("code".into(), "Ya existe un equipo con ese código.".into());
        return Err(CommandError {
            error: "Ya existe un equipo con ese código.".into(),
            fields,
        });
    }
    connection.execute(
        "INSERT INTO assets (code, name, category, location, status, manufacturer, model, serial_number)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            data.code,
            data.name,
            data.category,
            data.location,
            data.status,
            data.manufacturer,
            data.model,
            data.serial_number
        ],
    )?;
    let id = connection.last_insert_rowid();
    let assets = list_assets(connection)?;
    assets
        .into_iter()
        .find(|asset| asset.id == id)
        .ok_or_else(CommandError::internal)
}

fn map_customer(row: &Row<'_>) -> rusqlite::Result<Customer> {
    Ok(Customer {
        id: row.get(0)?,
        name: row.get(1)?,
        phone: row.get(2)?,
        email: row.get(3)?,
        address: row.get(4)?,
        company: row.get(5)?,
        tax_id: row.get(6)?,
        notes: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        order_count: row.get(10)?,
        active_order_count: row.get(11)?,
        equipment_count: row.get(12)?,
        last_service_at: row.get(13)?,
    })
}

const CUSTOMER_SELECT: &str = r#"
  SELECT c.id, c.name, c.phone, c.email, c.address, c.company, c.tax_id, c.notes,
         c.created_at, c.updated_at,
         (SELECT COUNT(*) FROM work_orders wo WHERE wo.customer_id = c.id),
         (SELECT COUNT(*) FROM work_orders wo WHERE wo.customer_id = c.id
            AND wo.status NOT IN ('entregada', 'cancelada')),
         (SELECT COUNT(DISTINCT wo.asset_id) FROM work_orders wo
            WHERE wo.customer_id = c.id AND wo.asset_id IS NOT NULL),
         (SELECT MAX(wo.created_at) FROM work_orders wo WHERE wo.customer_id = c.id)
  FROM customers c
"#;

pub fn list_customers(connection: &Connection) -> Result<Vec<Customer>, CommandError> {
    let sql = format!("{CUSTOMER_SELECT} ORDER BY c.name COLLATE NOCASE, c.id");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], map_customer)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn get_customer(connection: &Connection, id: i64) -> Result<Option<Customer>, CommandError> {
    let sql = format!("{CUSTOMER_SELECT} WHERE c.id = ?1");
    connection
        .query_row(&sql, [id], map_customer)
        .optional()
        .map_err(Into::into)
}

pub fn create_customer(
    connection: &Connection,
    data: ValidatedCustomer,
) -> Result<Customer, CommandError> {
    connection.execute(
        "INSERT INTO customers (name, phone, email, address, company, tax_id, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            data.name,
            data.phone,
            data.email,
            data.address,
            data.company,
            data.tax_id,
            data.notes
        ],
    )?;
    get_customer(connection, connection.last_insert_rowid())?.ok_or_else(CommandError::internal)
}

pub fn update_customer(
    connection: &Connection,
    id: i64,
    data: ValidatedCustomer,
) -> Result<Option<Customer>, CommandError> {
    let changes = connection.execute(
        "UPDATE customers SET name = ?1, phone = ?2, email = ?3, address = ?4,
                company = ?5, tax_id = ?6, notes = ?7, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?8",
        params![
            data.name,
            data.phone,
            data.email,
            data.address,
            data.company,
            data.tax_id,
            data.notes,
            id
        ],
    )?;
    if changes == 0 {
        return Ok(None);
    }
    get_customer(connection, id)
}

fn map_inventory_item(row: &Row<'_>) -> rusqlite::Result<InventoryItem> {
    Ok(InventoryItem {
        id: row.get(0)?,
        code: row.get(1)?,
        name: row.get(2)?,
        category: row.get(3)?,
        quantity: row.get(4)?,
        minimum_stock: row.get(5)?,
        unit: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        order_usage_count: row.get(9)?,
        consumed_quantity: row.get(10)?,
    })
}

pub fn list_inventory_items(connection: &Connection) -> Result<Vec<InventoryItem>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT i.id, i.code, i.name, i.category, i.quantity, i.minimum_stock, i.unit,
                i.created_at, i.updated_at,
                (SELECT COUNT(DISTINCT op.work_order_id) FROM work_order_parts op WHERE op.inventory_item_id = i.id),
                COALESCE((SELECT SUM(op.quantity) FROM work_order_parts op WHERE op.inventory_item_id = i.id), 0)
         FROM inventory_items i ORDER BY i.name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], map_inventory_item)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn get_inventory_item(
    connection: &Connection,
    id: i64,
) -> Result<Option<InventoryItem>, CommandError> {
    connection
        .query_row(
            "SELECT i.id, i.code, i.name, i.category, i.quantity, i.minimum_stock, i.unit,
                    i.created_at, i.updated_at,
                    (SELECT COUNT(DISTINCT op.work_order_id) FROM work_order_parts op WHERE op.inventory_item_id = i.id),
                    COALESCE((SELECT SUM(op.quantity) FROM work_order_parts op WHERE op.inventory_item_id = i.id), 0)
             FROM inventory_items i WHERE i.id = ?1",
            [id],
            map_inventory_item,
        )
        .optional()
        .map_err(Into::into)
}

fn inventory_code_exists(
    connection: &Connection,
    code: &str,
    except_id: Option<i64>,
) -> Result<bool, CommandError> {
    Ok(connection.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM inventory_items
           WHERE code = ?1 COLLATE NOCASE AND (?2 IS NULL OR id != ?2)
         )",
        params![code, except_id],
        |row| row.get(0),
    )?)
}

fn duplicate_inventory_code_error() -> CommandError {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert(
        "code".into(),
        "Ya existe un producto con ese código.".into(),
    );
    CommandError {
        error: "El código de inventario ya está en uso.".into(),
        fields,
    }
}

fn record_inventory_movement(
    connection: &Connection,
    inventory_item_id: i64,
    movement_type: &str,
    quantity_delta: i64,
    reference_type: &str,
    reference_id: Option<i64>,
    reference_number: &str,
    note: &str,
) -> Result<(), CommandError> {
    let changes = connection.execute(
        "INSERT INTO inventory_movements (
           inventory_item_id, item_code, item_name, unit, movement_type,
           quantity_delta, balance_after, reference_type, reference_id,
           reference_number, note
         )
         SELECT id, code, name, unit, ?2, ?3, quantity, ?4, ?5, ?6, ?7
         FROM inventory_items WHERE id = ?1",
        params![
            inventory_item_id,
            movement_type,
            quantity_delta,
            reference_type,
            reference_id,
            reference_number,
            note
        ],
    )?;
    if changes == 0 {
        return Err(CommandError::message(
            "No se pudo registrar el movimiento porque el producto ya no existe.",
        ));
    }
    Ok(())
}

pub fn create_inventory_item(
    connection: &mut Connection,
    data: ValidatedInventoryItem,
) -> Result<InventoryItem, CommandError> {
    let code = if data.code.is_empty() {
        let mut sequence: i64 = connection.query_row(
            "SELECT COALESCE(MAX(id), 0) + 1 FROM inventory_items",
            [],
            |row| row.get(0),
        )?;
        loop {
            let candidate = local_document_number(connection, "INV", None, sequence);
            if !inventory_code_exists(connection, &candidate, None)? {
                break candidate;
            }
            sequence += 1;
        }
    } else {
        data.code
    };
    if inventory_code_exists(connection, &code, None)? {
        return Err(duplicate_inventory_code_error());
    }
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO inventory_items (code, name, category, quantity, minimum_stock, unit)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            code,
            data.name,
            data.category,
            data.quantity,
            data.minimum_stock,
            data.unit
        ],
    )?;
    let item_id = transaction.last_insert_rowid();
    if data.quantity > 0 {
        record_inventory_movement(
            &transaction,
            item_id,
            "initial_stock",
            data.quantity,
            "manual",
            None,
            "",
            "Existencia inicial registrada",
        )?;
    }
    transaction.commit()?;
    get_inventory_item(connection, item_id)?.ok_or_else(CommandError::internal)
}

pub fn update_inventory_item(
    connection: &mut Connection,
    id: i64,
    data: ValidatedInventoryItem,
) -> Result<Option<InventoryItem>, CommandError> {
    let previous = match get_inventory_item(connection, id)? {
        Some(item) => item,
        None => return Ok(None),
    };
    let code = if data.code.is_empty() {
        local_document_number(connection, "INV", None, id)
    } else {
        data.code
    };
    if inventory_code_exists(connection, &code, Some(id))? {
        return Err(duplicate_inventory_code_error());
    }
    let transaction = connection.transaction()?;
    transaction.execute(
        "UPDATE inventory_items SET code = ?1, name = ?2, category = ?3, quantity = ?4,
                minimum_stock = ?5, unit = ?6, updated_at = CURRENT_TIMESTAMP
         WHERE id = ?7",
        params![
            code,
            data.name,
            data.category,
            data.quantity,
            data.minimum_stock,
            data.unit,
            id
        ],
    )?;
    let stock_delta = data.quantity - previous.quantity;
    if stock_delta != 0 {
        record_inventory_movement(
            &transaction,
            id,
            "manual_adjustment",
            stock_delta,
            "manual",
            None,
            "",
            "Ajuste manual de existencias",
        )?;
    }
    transaction.commit()?;
    get_inventory_item(connection, id)
}

fn purchase_report_lines(
    connection: &Connection,
    report_id: i64,
) -> Result<Vec<PurchaseReportLine>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, inventory_item_id, item_code, item_name, unit, stock_quantity,
                purchase_quantity, received_quantity,
                purchase_quantity - received_quantity,
                unit_cost_cents / 100.0, line_total_cents / 100.0
         FROM purchase_report_items WHERE purchase_report_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map([report_id], |row| {
        Ok(PurchaseReportLine {
            id: row.get(0)?,
            inventory_item_id: row.get(1)?,
            item_code: row.get(2)?,
            item_name: row.get(3)?,
            unit: row.get(4)?,
            stock_quantity: row.get(5)?,
            purchase_quantity: row.get(6)?,
            received_quantity: row.get(7)?,
            remaining_quantity: row.get(8)?,
            unit_cost: row.get(9)?,
            line_total: row.get(10)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_purchase_report(
    connection: &Connection,
    id: i64,
) -> Result<Option<PurchaseReport>, CommandError> {
    let mut report = connection
        .query_row(
            "SELECT id, report_number, supplier, notes, total_cost_cents / 100.0, created_at
             FROM purchase_reports WHERE id = ?1",
            [id],
            |row| {
                Ok(PurchaseReport {
                    id: row.get(0)?,
                    report_number: row.get(1)?,
                    supplier: row.get(2)?,
                    notes: row.get(3)?,
                    total_cost: row.get(4)?,
                    created_at: row.get(5)?,
                    status: String::new(),
                    ordered_quantity: 0,
                    received_quantity: 0,
                    lines: Vec::new(),
                })
            },
        )
        .optional()?;
    if let Some(item) = report.as_mut() {
        item.lines = purchase_report_lines(connection, id)?;
        item.ordered_quantity = item.lines.iter().map(|line| line.purchase_quantity).sum();
        item.received_quantity = item.lines.iter().map(|line| line.received_quantity).sum();
        item.status = if item.received_quantity == 0 {
            "pending"
        } else if item.received_quantity >= item.ordered_quantity {
            "received"
        } else {
            "partial"
        }
        .into();
    }
    Ok(report)
}

pub fn list_purchase_reports(connection: &Connection) -> Result<Vec<PurchaseReport>, CommandError> {
    let mut statement = connection.prepare("SELECT id FROM purchase_reports ORDER BY id DESC")?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let mut reports = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(report) = get_purchase_report(connection, id)? {
            reports.push(report);
        }
    }
    Ok(reports)
}

pub fn create_purchase_report(
    connection: &mut Connection,
    data: ValidatedPurchaseReport,
) -> Result<PurchaseReport, CommandError> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO purchase_reports (supplier, notes) VALUES (?1, ?2)",
        params![data.supplier, data.notes],
    )?;
    let report_id = transaction.last_insert_rowid();
    let year: String =
        transaction.query_row("SELECT strftime('%Y', 'now', 'localtime')", [], |row| {
            row.get(0)
        })?;
    let report_number = local_document_number(&transaction, "RC", Some(&year), report_id);
    let mut total_cost_cents = 0i64;

    for (position, line) in data.lines.iter().enumerate() {
        let item = get_inventory_item(&transaction, line.item_id)?.ok_or_else(|| {
            CommandError::message("Uno de los productos ya no existe en el inventario.")
        })?;
        let line_total_cents = line
            .unit_cost_cents
            .checked_mul(line.purchase_quantity)
            .ok_or_else(|| CommandError::message("El total del reporte es demasiado grande."))?;
        total_cost_cents = total_cost_cents
            .checked_add(line_total_cents)
            .ok_or_else(|| CommandError::message("El total del reporte es demasiado grande."))?;
        transaction.execute(
            "INSERT INTO purchase_report_items (
               purchase_report_id, inventory_item_id, item_code, item_name, unit,
               stock_quantity, purchase_quantity, unit_cost_cents, line_total_cents, position
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                report_id,
                item.id,
                item.code,
                item.name,
                item.unit,
                item.quantity,
                line.purchase_quantity,
                line.unit_cost_cents,
                line_total_cents,
                position as i64
            ],
        )?;
    }
    transaction.execute(
        "UPDATE purchase_reports SET report_number = ?1, total_cost_cents = ?2 WHERE id = ?3",
        params![report_number, total_cost_cents, report_id],
    )?;
    transaction.commit()?;
    get_purchase_report(connection, report_id)?.ok_or_else(CommandError::internal)
}

fn purchase_receipt_lines(
    connection: &Connection,
    receipt_id: i64,
) -> Result<Vec<PurchaseReceiptLine>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, inventory_item_id, item_code, item_name, unit, received_quantity,
                unit_cost_cents / 100.0, line_total_cents / 100.0
         FROM purchase_receipt_items WHERE purchase_receipt_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map([receipt_id], |row| {
        Ok(PurchaseReceiptLine {
            id: row.get(0)?,
            inventory_item_id: row.get(1)?,
            item_code: row.get(2)?,
            item_name: row.get(3)?,
            unit: row.get(4)?,
            received_quantity: row.get(5)?,
            unit_cost: row.get(6)?,
            line_total: row.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn get_purchase_receipt(
    connection: &Connection,
    id: i64,
) -> Result<Option<PurchaseReceipt>, CommandError> {
    let mut receipt = connection
        .query_row(
            "SELECT pr.id, pr.receipt_number, pr.purchase_report_id,
                    COALESCE(r.report_number, ''), pr.supplier, pr.notes,
                    pr.total_cost_cents / 100.0, pr.created_at
             FROM purchase_receipts pr
             LEFT JOIN purchase_reports r ON r.id = pr.purchase_report_id
             WHERE pr.id = ?1",
            [id],
            |row| {
                Ok(PurchaseReceipt {
                    id: row.get(0)?,
                    receipt_number: row.get(1)?,
                    purchase_report_id: row.get(2)?,
                    report_number: row.get(3)?,
                    supplier: row.get(4)?,
                    notes: row.get(5)?,
                    total_cost: row.get(6)?,
                    created_at: row.get(7)?,
                    lines: Vec::new(),
                })
            },
        )
        .optional()?;
    if let Some(item) = receipt.as_mut() {
        item.lines = purchase_receipt_lines(connection, id)?;
    }
    Ok(receipt)
}

pub fn list_purchase_receipts(
    connection: &Connection,
) -> Result<Vec<PurchaseReceipt>, CommandError> {
    let mut statement = connection.prepare("SELECT id FROM purchase_receipts ORDER BY id DESC")?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let mut receipts = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(receipt) = get_purchase_receipt(connection, id)? {
            receipts.push(receipt);
        }
    }
    Ok(receipts)
}

pub fn receive_purchase(
    connection: &mut Connection,
    report_id: i64,
    data: ValidatedPurchaseReceipt,
) -> Result<PurchaseReceipt, CommandError> {
    let report = get_purchase_report(connection, report_id)?
        .ok_or_else(|| CommandError::message("No encontramos ese reporte de compras."))?;
    if report.status == "received" {
        return Err(CommandError::message(
            "Este reporte ya fue recibido por completo.",
        ));
    }
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO purchase_receipts (purchase_report_id, supplier, notes)
         VALUES (?1, ?2, ?3)",
        params![report_id, report.supplier, data.notes],
    )?;
    let receipt_id = transaction.last_insert_rowid();
    let year: String =
        transaction.query_row("SELECT strftime('%Y', 'now', 'localtime')", [], |row| {
            row.get(0)
        })?;
    let receipt_number = local_document_number(&transaction, "EN", Some(&year), receipt_id);
    let mut total_cost_cents = 0_i64;

    for (position, line) in data.lines.iter().enumerate() {
        let report_line = transaction
            .query_row(
                "SELECT inventory_item_id, item_code, item_name, unit,
                        purchase_quantity, received_quantity, unit_cost_cents
                 FROM purchase_report_items
                 WHERE id = ?1 AND purchase_report_id = ?2",
                params![line.purchase_report_line_id, report_id],
                |row| {
                    Ok((
                        row.get::<_, Option<i64>>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                CommandError::message("Uno de los productos no pertenece al reporte.")
            })?;
        let (inventory_item_id, code, name, unit, ordered, already_received, unit_cost_cents) =
            report_line;
        let remaining = ordered - already_received;
        if line.received_quantity > remaining {
            return Err(CommandError::message(format!(
                "La recepción de '{}' supera lo pendiente. Faltan {} {}.",
                name, remaining, unit
            )));
        }
        let item_id = inventory_item_id.ok_or_else(|| {
            CommandError::message(format!(
                "El producto '{}' ya no existe en inventario.",
                name
            ))
        })?;
        let line_total_cents = unit_cost_cents
            .checked_mul(line.received_quantity)
            .ok_or_else(|| {
                CommandError::message("El total de la recepción es demasiado grande.")
            })?;
        total_cost_cents = total_cost_cents
            .checked_add(line_total_cents)
            .ok_or_else(|| {
                CommandError::message("El total de la recepción es demasiado grande.")
            })?;
        transaction.execute(
            "UPDATE purchase_report_items
             SET received_quantity = received_quantity + ?1 WHERE id = ?2",
            params![line.received_quantity, line.purchase_report_line_id],
        )?;
        transaction.execute(
            "UPDATE inventory_items
             SET quantity = quantity + ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
            params![line.received_quantity, item_id],
        )?;
        transaction.execute(
            "INSERT INTO purchase_receipt_items (
               purchase_receipt_id, purchase_report_item_id, inventory_item_id,
               item_code, item_name, unit, received_quantity, unit_cost_cents,
               line_total_cents, position
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                receipt_id,
                line.purchase_report_line_id,
                item_id,
                code,
                name,
                unit,
                line.received_quantity,
                unit_cost_cents,
                line_total_cents,
                position as i64
            ],
        )?;
        record_inventory_movement(
            &transaction,
            item_id,
            "purchase_receipt",
            line.received_quantity,
            "purchase_receipt",
            Some(receipt_id),
            &receipt_number,
            &format!("Recepción del reporte {}", report.report_number),
        )?;
    }
    transaction.execute(
        "UPDATE purchase_receipts
         SET receipt_number = ?1, total_cost_cents = ?2 WHERE id = ?3",
        params![receipt_number, total_cost_cents, receipt_id],
    )?;
    transaction.commit()?;
    get_purchase_receipt(connection, receipt_id)?.ok_or_else(CommandError::internal)
}

pub fn list_inventory_movements(
    connection: &Connection,
) -> Result<Vec<InventoryMovement>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, inventory_item_id, item_code, item_name, unit, movement_type,
                quantity_delta, balance_after, reference_type, reference_id,
                reference_number, note, created_at
         FROM inventory_movements ORDER BY id DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(InventoryMovement {
            id: row.get(0)?,
            inventory_item_id: row.get(1)?,
            item_code: row.get(2)?,
            item_name: row.get(3)?,
            unit: row.get(4)?,
            movement_type: row.get(5)?,
            quantity_delta: row.get(6)?,
            balance_after: row.get(7)?,
            reference_type: row.get(8)?,
            reference_id: row.get(9)?,
            reference_number: row.get(10)?,
            note: row.get(11)?,
            created_at: row.get(12)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn sale_lines(connection: &Connection, sale_id: i64) -> Result<Vec<SaleLine>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, inventory_item_id, item_code, item_name, unit, quantity,
                unit_price_cents / 100.0, line_total_cents / 100.0
         FROM sale_items WHERE sale_id = ?1 ORDER BY position, id",
    )?;
    let rows = statement.query_map([sale_id], |row| {
        Ok(SaleLine {
            id: row.get(0)?,
            inventory_item_id: row.get(1)?,
            item_code: row.get(2)?,
            item_name: row.get(3)?,
            unit: row.get(4)?,
            quantity: row.get(5)?,
            unit_price: row.get(6)?,
            line_total: row.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get_sale(connection: &Connection, id: i64) -> Result<Option<Sale>, CommandError> {
    let mut sale = connection
        .query_row(
            "SELECT id, sale_number, customer_name, notes, subtotal_cents / 100.0,
                    discount_cents / 100.0, total_cents / 100.0, status,
                    cancellation_note, created_at, cancelled_at
             FROM sales WHERE id = ?1",
            [id],
            |row| {
                Ok(Sale {
                    id: row.get(0)?,
                    sale_number: row.get(1)?,
                    customer_name: row.get(2)?,
                    notes: row.get(3)?,
                    subtotal: row.get(4)?,
                    discount: row.get(5)?,
                    total: row.get(6)?,
                    status: row.get(7)?,
                    cancellation_note: row.get(8)?,
                    created_at: row.get(9)?,
                    cancelled_at: row.get(10)?,
                    lines: Vec::new(),
                })
            },
        )
        .optional()?;
    if let Some(item) = sale.as_mut() {
        item.lines = sale_lines(connection, id)?;
    }
    Ok(sale)
}

pub fn list_sales(connection: &Connection) -> Result<Vec<Sale>, CommandError> {
    let mut statement = connection.prepare("SELECT id FROM sales ORDER BY id DESC")?;
    let ids = statement
        .query_map([], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    let mut sales = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(sale) = get_sale(connection, id)? {
            sales.push(sale);
        }
    }
    Ok(sales)
}

pub fn create_sale(connection: &mut Connection, data: ValidatedSale) -> Result<Sale, CommandError> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO sales (customer_name, notes, discount_cents) VALUES (?1, ?2, ?3)",
        params![data.customer_name, data.notes, data.discount_cents],
    )?;
    let sale_id = transaction.last_insert_rowid();
    let year: String =
        transaction.query_row("SELECT strftime('%Y', 'now', 'localtime')", [], |row| {
            row.get(0)
        })?;
    let sale_number = local_document_number(&transaction, "VT", Some(&year), sale_id);
    let mut subtotal_cents = 0_i64;
    for (position, line) in data.lines.iter().enumerate() {
        let item = get_inventory_item(&transaction, line.inventory_item_id)?
            .ok_or_else(|| CommandError::message("Uno de los productos ya no existe."))?;
        if item.quantity < line.quantity {
            return Err(CommandError::message(format!(
                "No hay existencia suficiente de '{}'. Disponible: {} {}.",
                item.name, item.quantity, item.unit
            )));
        }
        let line_total_cents = line
            .unit_price_cents
            .checked_mul(line.quantity)
            .ok_or_else(|| CommandError::message("El total de la venta es demasiado grande."))?;
        subtotal_cents = subtotal_cents
            .checked_add(line_total_cents)
            .ok_or_else(|| CommandError::message("El total de la venta es demasiado grande."))?;
        transaction.execute(
            "UPDATE inventory_items
             SET quantity = quantity - ?1, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?2 AND quantity >= ?1",
            params![line.quantity, item.id],
        )?;
        transaction.execute(
            "INSERT INTO sale_items (
               sale_id, inventory_item_id, item_code, item_name, unit,
               quantity, unit_price_cents, line_total_cents, position
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                sale_id,
                item.id,
                item.code,
                item.name,
                item.unit,
                line.quantity,
                line.unit_price_cents,
                line_total_cents,
                position as i64
            ],
        )?;
        record_inventory_movement(
            &transaction,
            item.id,
            "sale",
            -line.quantity,
            "sale",
            Some(sale_id),
            &sale_number,
            "Venta directa de producto",
        )?;
    }
    if data.discount_cents > subtotal_cents {
        return Err(CommandError::message(
            "El descuento no puede ser mayor que el subtotal de la venta.",
        ));
    }
    let total_cents = subtotal_cents - data.discount_cents;
    transaction.execute(
        "UPDATE sales SET sale_number = ?1, subtotal_cents = ?2, total_cents = ?3 WHERE id = ?4",
        params![sale_number, subtotal_cents, total_cents, sale_id],
    )?;
    transaction.commit()?;
    get_sale(connection, sale_id)?.ok_or_else(CommandError::internal)
}

pub fn cancel_sale(
    connection: &mut Connection,
    id: i64,
    note: String,
) -> Result<Option<Sale>, CommandError> {
    let sale = match get_sale(connection, id)? {
        Some(sale) => sale,
        None => return Ok(None),
    };
    if sale.status == "cancelled" {
        return Err(CommandError::message("La venta ya está anulada."));
    }
    let transaction = connection.transaction()?;
    for line in &sale.lines {
        if let Some(item_id) = line.inventory_item_id {
            transaction.execute(
                "UPDATE inventory_items
                 SET quantity = quantity + ?1, updated_at = CURRENT_TIMESTAMP WHERE id = ?2",
                params![line.quantity, item_id],
            )?;
            record_inventory_movement(
                &transaction,
                item_id,
                "sale_cancelled",
                line.quantity,
                "sale",
                Some(id),
                &sale.sale_number,
                "Existencias restituidas por anulación de venta",
            )?;
        }
    }
    transaction.execute(
        "UPDATE sales SET status = 'cancelled', cancellation_note = ?1,
                cancelled_at = CURRENT_TIMESTAMP WHERE id = ?2",
        params![note, id],
    )?;
    transaction.commit()?;
    get_sale(connection, id)
}

pub fn get_statistics(
    connection: &Connection,
    range_days: i64,
) -> Result<WorkshopStatistics, CommandError> {
    let modifier = format!("-{range_days} days");
    let order_filter = "(?1 = 0 OR datetime(created_at) >= datetime('now', 'localtime', ?2))";
    let orders_received: i64 = connection.query_row(
        &format!("SELECT COUNT(*) FROM work_orders WHERE {order_filter}"),
        params![range_days, modifier],
        |row| row.get(0),
    )?;
    let orders_delivered: i64 = connection.query_row(
        &format!("SELECT COUNT(*) FROM work_orders WHERE status = 'entregada' AND {order_filter}"),
        params![range_days, modifier],
        |row| row.get(0),
    )?;
    let open_orders: i64 = connection.query_row(
        "SELECT COUNT(*) FROM work_orders WHERE status NOT IN ('entregada', 'cancelada')",
        [],
        |row| row.get(0),
    )?;
    let service_revenue_cents: i64 = connection.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0) FROM order_payments
         WHERE status = 'active'
           AND (?1 = 0 OR datetime(created_at) >= datetime('now', 'localtime', ?2))",
        params![range_days, modifier],
        |row| row.get(0),
    )?;
    let sales_revenue_cents: i64 = connection.query_row(
        "SELECT COALESCE(SUM(total_cents), 0) FROM sales
         WHERE status = 'completed'
           AND (?1 = 0 OR datetime(created_at) >= datetime('now', 'localtime', ?2))",
        params![range_days, modifier],
        |row| row.get(0),
    )?;
    let average_turnaround_days: f64 = connection.query_row(
        "SELECT COALESCE(AVG(MAX(julianday(COALESCE(completed_at, updated_at)) - julianday(created_at), 0)), 0)
         FROM work_orders
         WHERE status IN ('completada', 'entregada')
           AND (?1 = 0 OR datetime(created_at) >= datetime('now', 'localtime', ?2))",
        params![range_days, modifier],
        |row| row.get(0),
    )?;
    let warranty_returns: i64 = connection.query_row(
        "SELECT COUNT(*) FROM warranty_claims wc
         JOIN work_orders wo ON wo.id = wc.return_order_id
         WHERE (?1 = 0 OR datetime(wo.created_at) >= datetime('now', 'localtime', ?2))",
        params![range_days, modifier],
        |row| row.get(0),
    )?;
    let warranty_covered: i64 = connection.query_row(
        "SELECT COUNT(*) FROM warranty_claims wc
         JOIN work_orders wo ON wo.id = wc.return_order_id
         WHERE wc.decision = 'covered'
           AND (?1 = 0 OR datetime(wo.created_at) >= datetime('now', 'localtime', ?2))",
        params![range_days, modifier],
        |row| row.get(0),
    )?;

    let mut status_statement = connection.prepare(
        "SELECT status, COUNT(*) FROM work_orders
         WHERE (?1 = 0 OR datetime(created_at) >= datetime('now', 'localtime', ?2))
         GROUP BY status ORDER BY COUNT(*) DESC",
    )?;
    let by_status = status_statement
        .query_map(params![range_days, modifier], |row| {
            Ok(StatisticsStatus {
                status: row.get(0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut monthly_statement = connection.prepare(
        "WITH RECURSIVE months(month, step) AS (
           SELECT strftime('%Y-%m', 'now', 'localtime', 'start of month', '-5 months'), 0
           UNION ALL
           SELECT strftime('%Y-%m', date(month || '-01', '+1 month')), step + 1
           FROM months WHERE step < 5
         )
         SELECT month,
                (SELECT COUNT(*) FROM work_orders wo WHERE strftime('%Y-%m', wo.created_at) = month),
                (SELECT COALESCE(SUM(amount_cents), 0) FROM order_payments op
                 WHERE op.status = 'active' AND strftime('%Y-%m', op.created_at) = month),
                (SELECT COALESCE(SUM(total_cents), 0) FROM sales s
                 WHERE s.status = 'completed' AND strftime('%Y-%m', s.created_at) = month)
         FROM months ORDER BY month",
    )?;
    let monthly = monthly_statement
        .query_map([], |row| {
            Ok(MonthlyStatistic {
                month: row.get(0)?,
                order_count: row.get(1)?,
                service_revenue: row.get::<_, i64>(2)? as f64 / 100.0,
                sales_revenue: row.get::<_, i64>(3)? as f64 / 100.0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut technician_statement = connection.prepare(
        "SELECT CASE WHEN trim(ol.technician) = '' THEN 'Sin asignar' ELSE ol.technician END,
                COUNT(*), COALESCE(SUM(ol.hours), 0), COALESCE(SUM(ol.line_total_cents), 0)
         FROM work_order_labor ol
         JOIN work_orders wo ON wo.id = ol.work_order_id
         WHERE (?1 = 0 OR datetime(wo.created_at) >= datetime('now', 'localtime', ?2))
         GROUP BY lower(trim(ol.technician))
         ORDER BY SUM(ol.hours) DESC, COUNT(*) DESC
         LIMIT 10",
    )?;
    let technicians = technician_statement
        .query_map(params![range_days, modifier], |row| {
            Ok(TechnicianStatistic {
                technician: row.get(0)?,
                jobs: row.get(1)?,
                hours: row.get(2)?,
                labor_total: row.get::<_, i64>(3)? as f64 / 100.0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(WorkshopStatistics {
        range_days,
        orders_received,
        orders_delivered,
        open_orders,
        service_revenue: service_revenue_cents as f64 / 100.0,
        sales_revenue: sales_revenue_cents as f64 / 100.0,
        average_turnaround_days,
        warranty_returns,
        warranty_covered,
        by_status,
        monthly,
        technicians,
    })
}

pub fn get_dashboard(connection: &Connection) -> Result<Dashboard, CommandError> {
    let totals = connection.query_row(
        r#"SELECT
          COALESCE(SUM(CASE WHEN status NOT IN ('entregada', 'cancelada') THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN priority = 'critica' AND status NOT IN ('completada', 'entregada', 'cancelada') THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN scheduled_date < date('now', 'localtime') AND status NOT IN ('completada', 'entregada', 'cancelada') THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN scheduled_date = date('now', 'localtime') AND status NOT IN ('completada', 'entregada', 'cancelada') THEN 1 ELSE 0 END), 0),
          COALESCE(SUM(CASE WHEN completed_at >= date('now', 'start of month', 'localtime') THEN 1 ELSE 0 END), 0),
          COUNT(*)
        FROM work_orders"#,
        [],
        |row| {
            Ok(DashboardTotals {
                active_orders: row.get(0)?,
                critical_open: row.get(1)?,
                overdue: row.get(2)?,
                due_today: row.get(3)?,
                completed_this_month: row.get(4)?,
                total_orders: row.get(5)?,
            })
        },
    )?;

    let mut status_statement =
        connection.prepare("SELECT status, COUNT(*) FROM work_orders GROUP BY status")?;
    let by_status = status_statement
        .query_map([], |row| {
            Ok(StatusCount {
                status: row.get(0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut priority_statement = connection.prepare(
        "SELECT priority, COUNT(*) FROM work_orders
         WHERE status NOT IN ('completada', 'entregada', 'cancelada') GROUP BY priority",
    )?;
    let by_priority = priority_statement
        .query_map([], |row| {
            Ok(PriorityCount {
                priority: row.get(0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let sql = format!(
        r#"{ORDER_SELECT}
        WHERE wo.status NOT IN ('completada', 'entregada', 'cancelada')
        ORDER BY
          CASE WHEN wo.scheduled_date IS NULL THEN 1 ELSE 0 END,
          wo.scheduled_date,
          CASE wo.priority WHEN 'critica' THEN 1 WHEN 'alta' THEN 2 WHEN 'media' THEN 3 ELSE 4 END
        LIMIT 6"#
    );
    let mut upcoming_statement = connection.prepare(&sql)?;
    let upcoming = upcoming_statement
        .query_map([], map_order)?
        .collect::<Result<Vec<_>, _>>()?;
    let generated_at =
        connection.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })?;
    Ok(Dashboard {
        totals,
        by_status,
        by_priority,
        upcoming,
        generated_at,
    })
}

pub fn export_all(connection: &Connection) -> Result<Backup, CommandError> {
    let assets = list_assets(connection)?;
    let customers = list_customers(connection)?;
    let inventory_items = list_inventory_items(connection)?;
    let purchase_reports = list_purchase_reports(connection)?;
    let purchase_receipts = list_purchase_receipts(connection)?;
    let inventory_movements = list_inventory_movements(connection)?;
    let sales = list_sales(connection)?;
    let summaries = list_orders(connection, &OrderFilters::default())?;
    let mut work_orders = Vec::with_capacity(summaries.len());
    for summary in summaries {
        if let Some(order) = get_order(connection, summary.id, true)? {
            work_orders.push(order);
        }
    }
    let exported_at =
        connection.query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })?;
    Ok(Backup {
        format: "geecko-maintenance-backup".into(),
        version: 13,
        exported_at,
        assets,
        customers,
        work_orders,
        inventory_items,
        purchase_reports,
        purchase_receipts,
        inventory_movements,
        sales,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn memory_database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        connection.execute_batch(
            "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT, phone TEXT DEFAULT '', email TEXT DEFAULT '', address TEXT DEFAULT '', company TEXT DEFAULT '', tax_id TEXT DEFAULT '', notes TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE assets (id INTEGER PRIMARY KEY, code TEXT UNIQUE, name TEXT, category TEXT DEFAULT '', location TEXT, status TEXT, manufacturer TEXT DEFAULT '', model TEXT DEFAULT '', serial_number TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE work_orders (id INTEGER PRIMARY KEY, order_number TEXT UNIQUE, title TEXT, description TEXT, type TEXT, priority TEXT, status TEXT, asset_id INTEGER REFERENCES assets(id), location TEXT DEFAULT '', assigned_to TEXT DEFAULT '', requested_by TEXT DEFAULT '', customer_id INTEGER REFERENCES customers(id), scheduled_date TEXT, estimated_hours REAL, safety_notes TEXT DEFAULT '', observations TEXT DEFAULT '', work_performed TEXT DEFAULT '', delivery_condition TEXT DEFAULT '', delivery_tested_by TEXT DEFAULT '', diagnosis TEXT DEFAULT '', recommendation TEXT DEFAULT '', quote_status TEXT DEFAULT 'draft', quote_subtotal_cents INTEGER DEFAULT 0, quote_discount_cents INTEGER DEFAULT 0, quote_total_cents INTEGER DEFAULT 0, authorized_by TEXT DEFAULT '', authorization_method TEXT DEFAULT '', authorization_note TEXT DEFAULT '', authorization_at TEXT, created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP, completed_at TEXT);
             CREATE TABLE work_order_tasks (id INTEGER PRIMARY KEY, work_order_id INTEGER REFERENCES work_orders(id), description TEXT, completed INTEGER DEFAULT 0, completed_at TEXT, position INTEGER DEFAULT 0);
             CREATE TABLE work_order_events (id INTEGER PRIMARY KEY, work_order_id INTEGER REFERENCES work_orders(id), action TEXT, previous_status TEXT, new_status TEXT, note TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE work_order_accessories (id INTEGER PRIMARY KEY, work_order_id INTEGER REFERENCES work_orders(id), name TEXT, position INTEGER DEFAULT 0);
             CREATE TABLE work_order_quote_items (id INTEGER PRIMARY KEY, work_order_id INTEGER REFERENCES work_orders(id), kind TEXT, description TEXT, quantity REAL, unit_price_cents INTEGER, line_total_cents INTEGER, position INTEGER DEFAULT 0);
             CREATE TABLE inventory_items (id INTEGER PRIMARY KEY, code TEXT UNIQUE COLLATE NOCASE, name TEXT, category TEXT DEFAULT '', quantity INTEGER DEFAULT 0, minimum_stock INTEGER DEFAULT 0, unit TEXT DEFAULT 'unidad', created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE work_order_parts (id INTEGER PRIMARY KEY, work_order_id INTEGER REFERENCES work_orders(id), inventory_item_id INTEGER REFERENCES inventory_items(id), item_code TEXT, item_name TEXT, unit TEXT, quantity INTEGER, unit_price_cents INTEGER DEFAULT 0, line_total_cents INTEGER DEFAULT 0, position INTEGER DEFAULT 0);
             CREATE TABLE work_order_labor (id INTEGER PRIMARY KEY, work_order_id INTEGER REFERENCES work_orders(id), technician TEXT, description TEXT, hours REAL, hourly_rate_cents INTEGER DEFAULT 0, line_total_cents INTEGER DEFAULT 0, position INTEGER DEFAULT 0);
             CREATE TABLE order_payments (id INTEGER PRIMARY KEY, receipt_number TEXT UNIQUE, work_order_id INTEGER REFERENCES work_orders(id), payment_type TEXT, method TEXT, amount_cents INTEGER, reference TEXT DEFAULT '', received_by TEXT, note TEXT DEFAULT '', status TEXT DEFAULT 'active', order_total_cents INTEGER DEFAULT 0, balance_before_cents INTEGER DEFAULT 0, balance_after_cents INTEGER DEFAULT 0, created_at TEXT DEFAULT CURRENT_TIMESTAMP, cancelled_at TEXT, cancellation_note TEXT DEFAULT '');
             CREATE TABLE order_warranties (id INTEGER PRIMARY KEY, warranty_number TEXT UNIQUE, work_order_id INTEGER UNIQUE REFERENCES work_orders(id), duration_days INTEGER, starts_at TEXT, expires_at TEXT, coverage TEXT, exclusions TEXT DEFAULT '', status TEXT DEFAULT 'active', created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE warranty_claims (id INTEGER PRIMARY KEY, claim_number TEXT UNIQUE, warranty_id INTEGER REFERENCES order_warranties(id), original_order_id INTEGER REFERENCES work_orders(id), return_order_id INTEGER UNIQUE REFERENCES work_orders(id), reported_issue TEXT, received_condition TEXT DEFAULT '', decision TEXT DEFAULT 'pending', resolution TEXT DEFAULT '', warranty_expires_at TEXT, within_warranty INTEGER DEFAULT 0, created_at TEXT DEFAULT CURRENT_TIMESTAMP, resolved_at TEXT);
             CREATE TABLE app_users (id INTEGER PRIMARY KEY, username TEXT UNIQUE COLLATE NOCASE, display_name TEXT, role TEXT, password_hash TEXT, active INTEGER DEFAULT 1, created_at TEXT DEFAULT CURRENT_TIMESTAMP, last_login_at TEXT);
             CREATE TABLE user_activity (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES app_users(id), user_display_name TEXT, user_role TEXT, action TEXT, entity_type TEXT DEFAULT '', entity_id INTEGER, summary TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE order_photos (id INTEGER PRIMARY KEY, work_order_id INTEGER REFERENCES work_orders(id), file_name TEXT, mime_type TEXT, category TEXT, caption TEXT DEFAULT '', data BLOB, created_by TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE purchase_reports (id INTEGER PRIMARY KEY, report_number TEXT UNIQUE, supplier TEXT DEFAULT '', notes TEXT DEFAULT '', total_cost_cents INTEGER DEFAULT 0, created_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE purchase_report_items (id INTEGER PRIMARY KEY, purchase_report_id INTEGER REFERENCES purchase_reports(id), inventory_item_id INTEGER REFERENCES inventory_items(id), item_code TEXT, item_name TEXT, unit TEXT, stock_quantity INTEGER, purchase_quantity INTEGER, received_quantity INTEGER DEFAULT 0, unit_cost_cents INTEGER, line_total_cents INTEGER, position INTEGER DEFAULT 0);
             CREATE TABLE purchase_receipts (id INTEGER PRIMARY KEY, receipt_number TEXT UNIQUE, purchase_report_id INTEGER REFERENCES purchase_reports(id), supplier TEXT DEFAULT '', notes TEXT DEFAULT '', total_cost_cents INTEGER DEFAULT 0, created_at TEXT DEFAULT CURRENT_TIMESTAMP);
             CREATE TABLE purchase_receipt_items (id INTEGER PRIMARY KEY, purchase_receipt_id INTEGER REFERENCES purchase_receipts(id), purchase_report_item_id INTEGER REFERENCES purchase_report_items(id), inventory_item_id INTEGER REFERENCES inventory_items(id), item_code TEXT, item_name TEXT, unit TEXT, received_quantity INTEGER, unit_cost_cents INTEGER, line_total_cents INTEGER, position INTEGER DEFAULT 0);
             CREATE TABLE sales (id INTEGER PRIMARY KEY, sale_number TEXT UNIQUE, customer_name TEXT DEFAULT '', notes TEXT DEFAULT '', subtotal_cents INTEGER DEFAULT 0, discount_cents INTEGER DEFAULT 0, total_cents INTEGER DEFAULT 0, status TEXT DEFAULT 'completed', cancellation_note TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP, cancelled_at TEXT);
             CREATE TABLE sale_items (id INTEGER PRIMARY KEY, sale_id INTEGER REFERENCES sales(id), inventory_item_id INTEGER REFERENCES inventory_items(id), item_code TEXT, item_name TEXT, unit TEXT, quantity INTEGER, unit_price_cents INTEGER, line_total_cents INTEGER, position INTEGER DEFAULT 0);
             CREATE TABLE inventory_movements (id INTEGER PRIMARY KEY, inventory_item_id INTEGER REFERENCES inventory_items(id), item_code TEXT, item_name TEXT, unit TEXT, movement_type TEXT, quantity_delta INTEGER, balance_after INTEGER, reference_type TEXT DEFAULT '', reference_id INTEGER, reference_number TEXT DEFAULT '', note TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP);"
        ).unwrap();
        connection
    }

    #[test]
    fn creates_order_with_tasks_and_number() {
        let mut connection = memory_database();
        let order = create_order(
            &mut connection,
            ValidatedOrder {
                title: "Diagnóstico de portátil".into(),
                description: "El equipo no enciende al conectarlo".into(),
                kind: "preventivo".into(),
                priority: "alta".into(),
                status: "solicitada".into(),
                asset_id: None,
                incoming_asset: Some(ValidatedAsset {
                    code: String::new(),
                    name: "Portátil de recepción".into(),
                    category: "Portátil".into(),
                    location: "Cliente particular".into(),
                    status: "mantenimiento".into(),
                    manufacturer: "Lenovo".into(),
                    model: "ThinkPad T14".into(),
                    serial_number: "ABC123".into(),
                }),
                customer_id: None,
                incoming_customer: Some(ValidatedCustomer {
                    name: "Carlos Méndez".into(),
                    phone: "5555-0101".into(),
                    email: "carlos@example.com".into(),
                    address: String::new(),
                    company: String::new(),
                    tax_id: String::new(),
                    notes: String::new(),
                }),
                location: "Cliente particular".into(),
                assigned_to: "Técnico".into(),
                requested_by: "Cliente".into(),
                scheduled_date: None,
                estimated_hours: Some(2.0),
                safety_notes: String::new(),
                observations: "Carcasa con desgaste visible".into(),
                accessories: vec!["Cargador".into(), "Mouse".into()],
                tasks: vec![
                    "Realizar diagnóstico".into(),
                    "Ejecutar pruebas finales".into(),
                ],
            },
        )
        .unwrap();
        assert!(order.order_number.starts_with("GM-"));
        assert_eq!(order.task_count, 2);
        assert_eq!(order.accessories, vec!["Cargador", "Mouse"]);
        assert_eq!(order.observations, "Carcasa con desgaste visible");
        assert_eq!(order.asset_category.as_deref(), Some("Portátil"));
        assert_eq!(order.requested_by, "Carlos Méndez");
        assert_eq!(order.customer_phone, "5555-0101");
        assert_eq!(order.events.len(), 1);

        let quoted = save_diagnosis_quote(
            &mut connection,
            order.id,
            ValidatedDiagnosisQuote {
                diagnosis: "El conector de carga está dañado.".into(),
                recommendation: "Reemplazar el conector y realizar pruebas.".into(),
                discount_cents: 5_000,
                items: vec![ValidatedQuoteItem {
                    kind: "mano_obra".into(),
                    description: "Reemplazo de conector".into(),
                    quantity: 1.0,
                    unit_price_cents: 35_000,
                    line_total_cents: 35_000,
                }],
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(quoted.quote_status, "draft");
        assert_eq!(quoted.quote_total, 300.0);
        assert_eq!(quoted.quote_items.len(), 1);

        let pending = set_quote_decision(
            &mut connection,
            order.id,
            ValidatedQuoteDecision {
                decision: "pending".into(),
                method: String::new(),
                authorized_by: String::new(),
                note: String::new(),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(pending.status, "pendiente_autorizacion");
        assert_eq!(pending.quote_status, "pending");
        assert!(set_task_completed(&connection, order.id, pending.tasks[0].id, true).is_err());

        let approved = set_quote_decision(
            &mut connection,
            order.id,
            ValidatedQuoteDecision {
                decision: "approved".into(),
                method: "whatsapp".into(),
                authorized_by: "Carlos Méndez".into(),
                note: "Autorizó el total cotizado".into(),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(approved.status, "en_progreso");
        assert_eq!(approved.quote_status, "approved");
        assert_eq!(approved.authorization_method, "whatsapp");

        let inventory = create_inventory_item(
            &mut connection,
            ValidatedInventoryItem {
                code: String::new(),
                name: "SSD NVMe 1 TB".into(),
                category: "Almacenamiento".into(),
                quantity: 5,
                minimum_stock: 2,
                unit: "unidad".into(),
            },
        )
        .unwrap();
        assert_eq!(inventory.code, "INV-0001");

        let resources = save_order_resources(
            &mut connection,
            order.id,
            ValidatedOrderResources {
                parts: vec![ValidatedOrderPart {
                    inventory_item_id: inventory.id,
                    quantity: 2,
                    unit_price_cents: 25_000,
                    line_total_cents: 50_000,
                }],
                labor: vec![ValidatedOrderLabor {
                    technician: "Técnico".into(),
                    description: "Instalación y pruebas".into(),
                    hours: 1.5,
                    hourly_rate_cents: 10_000,
                    line_total_cents: 15_000,
                }],
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(resources.parts.len(), 1);
        assert_eq!(resources.labor.len(), 1);
        assert_eq!(resources.resources_total, 650.0);
        let consumed_inventory = list_inventory_items(&connection).unwrap().remove(0);
        assert_eq!(consumed_inventory.quantity, 3);
        assert_eq!(consumed_inventory.order_usage_count, 1);
        assert_eq!(consumed_inventory.consumed_quantity, 2);

        let insufficient = save_order_resources(
            &mut connection,
            order.id,
            ValidatedOrderResources {
                parts: vec![ValidatedOrderPart {
                    inventory_item_id: inventory.id,
                    quantity: 10,
                    unit_price_cents: 25_000,
                    line_total_cents: 250_000,
                }],
                labor: Vec::new(),
            },
        );
        assert!(insufficient.is_err());
        assert_eq!(list_inventory_items(&connection).unwrap()[0].quantity, 3);
        assert_eq!(
            get_order(&connection, order.id, true)
                .unwrap()
                .unwrap()
                .parts[0]
                .quantity,
            2
        );

        let corrected = save_order_resources(
            &mut connection,
            order.id,
            ValidatedOrderResources {
                parts: vec![ValidatedOrderPart {
                    inventory_item_id: inventory.id,
                    quantity: 1,
                    unit_price_cents: 25_000,
                    line_total_cents: 25_000,
                }],
                labor: vec![ValidatedOrderLabor {
                    technician: "Técnico".into(),
                    description: "Instalación y pruebas".into(),
                    hours: 1.5,
                    hourly_rate_cents: 10_000,
                    line_total_cents: 15_000,
                }],
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(corrected.parts[0].quantity, 1);
        assert_eq!(corrected.resources_total, 400.0);
        assert_eq!(list_inventory_items(&connection).unwrap()[0].quantity, 4);

        let advance = create_order_payment(
            &mut connection,
            order.id,
            ValidatedOrderPayment {
                payment_type: "advance".into(),
                method: "cash".into(),
                amount_cents: 10_000,
                reference: String::new(),
                received_by: "Recepción".into(),
                note: "Anticipo inicial".into(),
            },
        )
        .unwrap()
        .unwrap();
        assert!(advance.receipt_number.starts_with("RP-"));
        let with_advance = get_order(&connection, order.id, true).unwrap().unwrap();
        assert_eq!(with_advance.financial_total, 400.0);
        assert_eq!(with_advance.payments_total, 100.0);
        assert_eq!(with_advance.balance_due, 300.0);
        assert!(create_order_payment(
            &mut connection,
            order.id,
            ValidatedOrderPayment {
                payment_type: "payment".into(),
                method: "card".into(),
                amount_cents: 35_000,
                reference: String::new(),
                received_by: "Recepción".into(),
                note: String::new(),
            },
        )
        .is_err());
        let final_payment = create_order_payment(
            &mut connection,
            order.id,
            ValidatedOrderPayment {
                payment_type: "payment".into(),
                method: "transfer".into(),
                amount_cents: 30_000,
                reference: "TRX-01".into(),
                received_by: "Recepción".into(),
                note: String::new(),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(final_payment.balance_after, 0.0);
        let paid = get_order(&connection, order.id, true).unwrap().unwrap();
        assert_eq!(paid.payments_total, 400.0);
        assert_eq!(paid.balance_due, 0.0);
        let cancelled_payment = cancel_order_payment(
            &mut connection,
            order.id,
            advance.id,
            "Registro duplicado".into(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(cancelled_payment.status, "cancelled");
        let corrected_balance = get_order(&connection, order.id, true).unwrap().unwrap();
        assert_eq!(corrected_balance.payments_total, 300.0);
        assert_eq!(corrected_balance.balance_due, 100.0);

        let invalid_jump = update_order(
            &mut connection,
            order.id,
            ValidatedOrderPatch {
                status: Some("entregada".into()),
                ..ValidatedOrderPatch::default()
            },
        );
        assert!(invalid_jump.is_err());

        let started = update_order(
            &mut connection,
            order.id,
            ValidatedOrderPatch {
                status: Some("en_progreso".into()),
                ..ValidatedOrderPatch::default()
            },
        )
        .unwrap()
        .unwrap();
        let incomplete = update_order(
            &mut connection,
            order.id,
            ValidatedOrderPatch {
                status: Some("completada".into()),
                work_performed: Some("Diagnóstico terminado".into()),
                ..ValidatedOrderPatch::default()
            },
        );
        assert!(incomplete.is_err());
        for task in started.tasks {
            set_task_completed(&connection, order.id, task.id, true).unwrap();
        }

        let delivered = update_order(
            &mut connection,
            order.id,
            ValidatedOrderPatch {
                work_performed: Some("Se reemplazó el conector de carga.".into()),
                delivery_condition: Some("reparado".into()),
                delivery_tested_by: Some("cliente".into()),
                ..ValidatedOrderPatch::default()
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(delivered.delivery_condition, "reparado");
        assert_eq!(delivered.delivery_tested_by, "cliente");
        assert!(delivered.work_performed.contains("conector de carga"));

        let ready = update_order(
            &mut connection,
            order.id,
            ValidatedOrderPatch {
                status: Some("completada".into()),
                ..ValidatedOrderPatch::default()
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(ready.status, "completada");
        let ready_asset = list_assets(&connection).unwrap().remove(0);
        assert_eq!(ready_asset.status, "listo");
        assert_eq!(ready_asset.open_order_count, 1);

        connection
            .execute(
                "UPDATE assets SET status = 'mantenimiento' WHERE id = ?1",
                [ready_asset.id],
            )
            .unwrap();
        synchronize_all_asset_statuses(&connection).unwrap();
        assert_eq!(list_assets(&connection).unwrap().remove(0).status, "listo");

        let handed_over = update_order(
            &mut connection,
            order.id,
            ValidatedOrderPatch {
                status: Some("entregada".into()),
                ..ValidatedOrderPatch::default()
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(handed_over.status, "entregada");
        let handed_over_asset = list_assets(&connection).unwrap().remove(0);
        assert_eq!(handed_over_asset.status, "entregado");
        assert_eq!(handed_over_asset.open_order_count, 0);

        let today: String = connection
            .query_row("SELECT date('now', 'localtime')", [], |row| row.get(0))
            .unwrap();
        let warranty = save_order_warranty(
            &mut connection,
            order.id,
            ValidatedWarranty {
                starts_at: today,
                duration_days: 90,
                coverage: "Reparación realizada y repuesto instalado".into(),
                exclusions: "Daño físico o humedad".into(),
            },
        )
        .unwrap()
        .unwrap();
        assert!(warranty.warranty_number.starts_with("GT-"));
        let warranty_return = create_warranty_return(
            &mut connection,
            order.id,
            ValidatedWarrantyReturn {
                reported_issue: "El conector volvió a presentar una falla".into(),
                received_condition: "Sin daños nuevos visibles".into(),
                priority: "baja".into(),
                accessories: vec!["Cargador".into()],
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(warranty_return.kind, "garantia");
        assert!(
            warranty_return
                .warranty_claim
                .as_ref()
                .unwrap()
                .within_warranty
        );
        let return_with_cost = save_order_resources(
            &mut connection,
            warranty_return.id,
            ValidatedOrderResources {
                parts: Vec::new(),
                labor: vec![ValidatedOrderLabor {
                    technician: "Técnico".into(),
                    description: "Revisión de garantía".into(),
                    hours: 1.0,
                    hourly_rate_cents: 10_000,
                    line_total_cents: 10_000,
                }],
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(return_with_cost.financial_total, 100.0);
        let covered_return = resolve_warranty_claim(
            &mut connection,
            warranty_return.id,
            ValidatedWarrantyDecision {
                decision: "covered".into(),
                resolution: "La falla corresponde al trabajo garantizado".into(),
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(covered_return.financial_total, 0.0);
        assert_eq!(covered_return.warranty_claim.unwrap().decision, "covered");
        assert!(create_order_payment(
            &mut connection,
            warranty_return.id,
            ValidatedOrderPayment {
                payment_type: "payment".into(),
                method: "cash".into(),
                amount_cents: 10_000,
                reference: String::new(),
                received_by: "Recepción".into(),
                note: String::new(),
            },
        )
        .is_err());
        let original_with_return = get_order(&connection, order.id, true).unwrap().unwrap();
        assert_eq!(original_with_return.warranty_returns.len(), 1);

        let report = create_purchase_report(
            &mut connection,
            ValidatedPurchaseReport {
                supplier: "Proveedor local".into(),
                notes: "Compra de reposiciÃ³n".into(),
                lines: vec![ValidatedPurchaseReportLine {
                    item_id: inventory.id,
                    purchase_quantity: 4,
                    unit_cost_cents: 39_995,
                }],
            },
        )
        .unwrap();
        assert!(report.report_number.starts_with("RC-"));
        assert_eq!(report.total_cost, 1_599.80);
        assert_eq!(report.lines[0].stock_quantity, 4);
        assert_eq!(report.lines[0].purchase_quantity, 4);
        assert_eq!(report.status, "pending");

        let receipt = receive_purchase(
            &mut connection,
            report.id,
            ValidatedPurchaseReceipt {
                notes: "Primera entrega".into(),
                lines: vec![ValidatedPurchaseReceiptLine {
                    purchase_report_line_id: report.lines[0].id,
                    received_quantity: 2,
                }],
            },
        )
        .unwrap();
        assert!(receipt.receipt_number.starts_with("EN-"));
        assert_eq!(receipt.lines[0].received_quantity, 2);
        assert_eq!(list_inventory_items(&connection).unwrap()[0].quantity, 6);
        let partial_report = get_purchase_report(&connection, report.id)
            .unwrap()
            .unwrap();
        assert_eq!(partial_report.status, "partial");
        assert_eq!(partial_report.lines[0].remaining_quantity, 2);

        let sale = create_sale(
            &mut connection,
            ValidatedSale {
                customer_name: "Cliente de mostrador".into(),
                notes: String::new(),
                discount_cents: 1_000,
                lines: vec![ValidatedSaleLine {
                    inventory_item_id: inventory.id,
                    quantity: 2,
                    unit_price_cents: 45_000,
                }],
            },
        )
        .unwrap();
        assert!(sale.sale_number.starts_with("VT-"));
        assert_eq!(sale.total, 890.0);
        assert_eq!(list_inventory_items(&connection).unwrap()[0].quantity, 4);
        let cancelled = cancel_sale(&mut connection, sale.id, "Cambio del cliente".into())
            .unwrap()
            .unwrap();
        assert_eq!(cancelled.status, "cancelled");
        assert_eq!(list_inventory_items(&connection).unwrap()[0].quantity, 6);
        assert!(list_inventory_movements(&connection).unwrap().len() >= 5);

        let photo = add_order_photo(
            &connection,
            order.id,
            ValidatedOrderPhoto {
                file_name: "recepcion.jpg".into(),
                mime_type: "image/jpeg".into(),
                category: "recepcion".into(),
                caption: "Estado al recibir".into(),
                data: vec![0xff, 0xd8, 0xff, 0xd9],
            },
            "Tecnico de prueba",
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            get_order_photo_data(&connection, photo.id)
                .unwrap()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            get_order(&connection, order.id, true)
                .unwrap()
                .unwrap()
                .photos
                .len(),
            1
        );

        let statistics = get_statistics(&connection, 0).unwrap();
        assert!(statistics.orders_received >= 2);
        assert!(statistics.orders_delivered >= 1);

        let backup = export_all(&connection).unwrap();
        assert_eq!(backup.version, 13);
        assert_eq!(backup.customers.len(), 1);
        assert_eq!(backup.inventory_items.len(), 1);
        assert_eq!(backup.purchase_reports.len(), 1);
        assert_eq!(backup.purchase_receipts.len(), 1);
        assert_eq!(backup.sales.len(), 1);
        assert_eq!(backup.work_orders[0].payments.len(), 2);
    }

    #[test]
    fn migrates_existing_order_clients_without_losing_their_names() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "geecko-customer-migration-{}-{suffix}.db",
            std::process::id()
        ));
        {
            let legacy = Connection::open(&path).unwrap();
            legacy
                .execute_batch(
                    "CREATE TABLE assets (id INTEGER PRIMARY KEY, code TEXT UNIQUE, name TEXT, category TEXT DEFAULT '', location TEXT, status TEXT, manufacturer TEXT DEFAULT '', model TEXT DEFAULT '', serial_number TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP);
                     CREATE TABLE work_orders (id INTEGER PRIMARY KEY, order_number TEXT UNIQUE, title TEXT, description TEXT, type TEXT, priority TEXT, status TEXT, asset_id INTEGER REFERENCES assets(id), location TEXT DEFAULT '', assigned_to TEXT DEFAULT '', requested_by TEXT DEFAULT '', scheduled_date TEXT, estimated_hours REAL, safety_notes TEXT DEFAULT '', observations TEXT DEFAULT '', work_performed TEXT DEFAULT '', delivery_condition TEXT DEFAULT '', delivery_tested_by TEXT DEFAULT '', created_at TEXT DEFAULT CURRENT_TIMESTAMP, updated_at TEXT DEFAULT CURRENT_TIMESTAMP, completed_at TEXT);
                     INSERT INTO work_orders (order_number, title, description, type, priority, status, requested_by)
                     VALUES ('GM-2026-0001', 'Diagnóstico', 'Equipo sin video', 'inspeccion', 'media', 'solicitada', 'Cliente anterior');",
                )
                .unwrap();
        }
        initialize(&path).unwrap();
        {
            let migrated = open(&path).unwrap();
            let customers = list_customers(&migrated).unwrap();
            assert_eq!(customers.len(), 1);
            assert_eq!(customers[0].name, "Cliente anterior");
            let order = get_order(&migrated, 1, false).unwrap().unwrap();
            assert_eq!(order.customer_id, Some(customers[0].id));
            assert_eq!(order.requested_by, "Cliente anterior");
        }
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }
}
