import { mkdirSync } from 'node:fs';
import { dirname } from 'node:path';
import { DatabaseSync } from 'node:sqlite';

const ORDER_SELECT = `
  SELECT
    wo.id,
    wo.order_number AS orderNumber,
    wo.title,
    wo.description,
    wo.type,
    wo.priority,
    wo.status,
    wo.asset_id AS assetId,
    a.code AS assetCode,
    a.name AS assetName,
    COALESCE(NULLIF(wo.location, ''), a.location, '') AS location,
    wo.assigned_to AS assignedTo,
    wo.requested_by AS requestedBy,
    wo.scheduled_date AS scheduledDate,
    wo.estimated_hours AS estimatedHours,
    wo.safety_notes AS safetyNotes,
    wo.created_at AS createdAt,
    wo.updated_at AS updatedAt,
    wo.completed_at AS completedAt,
    (SELECT COUNT(*) FROM work_order_tasks t WHERE t.work_order_id = wo.id) AS taskCount,
    (SELECT COUNT(*) FROM work_order_tasks t WHERE t.work_order_id = wo.id AND t.completed = 1) AS completedTaskCount
  FROM work_orders wo
  LEFT JOIN assets a ON a.id = wo.asset_id
`;

function localDate(date = new Date()) {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, '0');
  const day = String(date.getDate()).padStart(2, '0');
  return `${year}-${month}-${day}`;
}

function rowToAsset(row) {
  return row ? { ...row, id: Number(row.id) } : null;
}

function rowToTask(row) {
  return row ? { ...row, id: Number(row.id), completed: Boolean(row.completed) } : null;
}

function rowToOrder(row) {
  if (!row) return null;
  return {
    ...row,
    id: Number(row.id),
    assetId: row.assetId === null ? null : Number(row.assetId),
    taskCount: Number(row.taskCount ?? 0),
    completedTaskCount: Number(row.completedTaskCount ?? 0)
  };
}

export function createStore(databasePath) {
  if (databasePath !== ':memory:') mkdirSync(dirname(databasePath), { recursive: true });
  const db = new DatabaseSync(databasePath);
  db.exec('PRAGMA foreign_keys = ON;');
  db.exec('PRAGMA busy_timeout = 5000;');
  if (databasePath !== ':memory:') db.exec('PRAGMA journal_mode = WAL;');

  db.exec(`
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
      scheduled_date TEXT,
      estimated_hours REAL,
      safety_notes TEXT NOT NULL DEFAULT '',
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

    CREATE INDEX IF NOT EXISTS idx_work_orders_status ON work_orders(status);
    CREATE INDEX IF NOT EXISTS idx_work_orders_priority ON work_orders(priority);
    CREATE INDEX IF NOT EXISTS idx_work_orders_scheduled_date ON work_orders(scheduled_date);
    CREATE INDEX IF NOT EXISTS idx_work_orders_asset_id ON work_orders(asset_id);
    CREATE INDEX IF NOT EXISTS idx_tasks_work_order_id ON work_order_tasks(work_order_id);
  `);

  const statements = {
    insertOrder: db.prepare(`
      INSERT INTO work_orders (
        title, description, type, priority, status, asset_id, location,
        assigned_to, requested_by, scheduled_date, estimated_hours, safety_notes
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `),
    updateOrderNumber: db.prepare('UPDATE work_orders SET order_number = ? WHERE id = ?'),
    insertTask: db.prepare(`
      INSERT INTO work_order_tasks (work_order_id, description, position) VALUES (?, ?, ?)
    `),
    insertEvent: db.prepare(`
      INSERT INTO work_order_events (work_order_id, action, previous_status, new_status, note)
      VALUES (?, ?, ?, ?, ?)
    `),
    getOrder: db.prepare(`${ORDER_SELECT} WHERE wo.id = ?`),
    getTasks: db.prepare(`
      SELECT id, description, completed, completed_at AS completedAt, position
      FROM work_order_tasks WHERE work_order_id = ? ORDER BY position, id
    `),
    getEvents: db.prepare(`
      SELECT id, action, previous_status AS previousStatus, new_status AS newStatus,
             note, created_at AS createdAt
      FROM work_order_events WHERE work_order_id = ? ORDER BY id DESC
    `),
    getAsset: db.prepare(`
      SELECT id, code, name, category, location, status, manufacturer, model,
             serial_number AS serialNumber, created_at AS createdAt, updated_at AS updatedAt
      FROM assets WHERE id = ?
    `),
    getAssetByCode: db.prepare('SELECT id FROM assets WHERE code = ? COLLATE NOCASE'),
    insertAsset: db.prepare(`
      INSERT INTO assets (code, name, category, location, status, manufacturer, model, serial_number)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    `),
    setTask: db.prepare(`
      UPDATE work_order_tasks
      SET completed = ?, completed_at = CASE WHEN ? = 1 THEN CURRENT_TIMESTAMP ELSE NULL END
      WHERE id = ? AND work_order_id = ?
    `)
  };

  function getOrder(id, { includeDetails = true } = {}) {
    const order = rowToOrder(statements.getOrder.get(id));
    if (!order || !includeDetails) return order;
    order.tasks = statements.getTasks.all(id).map(rowToTask);
    order.events = statements.getEvents.all(id).map((row) => ({ ...row, id: Number(row.id) }));
    return order;
  }

  function createOrder(data) {
    db.exec('BEGIN IMMEDIATE');
    try {
      const inserted = statements.insertOrder.run(
        data.title,
        data.description,
        data.type,
        data.priority,
        data.status,
        data.assetId,
        data.location,
        data.assignedTo,
        data.requestedBy,
        data.scheduledDate,
        data.estimatedHours,
        data.safetyNotes
      );
      const id = Number(inserted.lastInsertRowid);
      const year = new Date().getFullYear();
      const orderNumber = `GM-${year}-${String(id).padStart(4, '0')}`;
      statements.updateOrderNumber.run(orderNumber, id);
      data.tasks.forEach((task, index) => statements.insertTask.run(id, task, index));
      statements.insertEvent.run(id, 'created', null, data.status, 'Orden creada');
      db.exec('COMMIT');
      return getOrder(id);
    } catch (error) {
      db.exec('ROLLBACK');
      throw error;
    }
  }

  function listOrders(filters = {}) {
    const conditions = [];
    const parameters = [];

    if (filters.status && filters.status !== 'todos') {
      conditions.push('wo.status = ?');
      parameters.push(filters.status);
    }
    if (filters.priority && filters.priority !== 'todas') {
      conditions.push('wo.priority = ?');
      parameters.push(filters.priority);
    }
    if (filters.search) {
      conditions.push(`(
        wo.order_number LIKE ? OR wo.title LIKE ? OR wo.description LIKE ? OR
        wo.assigned_to LIKE ? OR a.name LIKE ? OR a.code LIKE ? OR wo.location LIKE ?
      )`);
      const search = `%${filters.search.replaceAll('%', '\\%').replaceAll('_', '\\_')}%`;
      parameters.push(search, search, search, search, search, search, search);
    }

    const where = conditions.length ? ` WHERE ${conditions.join(' AND ')}` : '';
    const sql = `${ORDER_SELECT}${where}
      ORDER BY
        CASE wo.priority WHEN 'critica' THEN 1 WHEN 'alta' THEN 2 WHEN 'media' THEN 3 ELSE 4 END,
        CASE WHEN wo.scheduled_date IS NULL THEN 1 ELSE 0 END,
        wo.scheduled_date,
        wo.id DESC
      LIMIT 500`;
    return db.prepare(sql).all(...parameters).map(rowToOrder);
  }

  function updateOrder(id, changes) {
    const current = getOrder(id, { includeDetails: false });
    if (!current) return null;

    const columnMap = {
      title: 'title',
      description: 'description',
      type: 'type',
      priority: 'priority',
      status: 'status',
      assetId: 'asset_id',
      location: 'location',
      assignedTo: 'assigned_to',
      requestedBy: 'requested_by',
      scheduledDate: 'scheduled_date',
      estimatedHours: 'estimated_hours',
      safetyNotes: 'safety_notes'
    };
    const entries = Object.entries(changes).filter(([key]) => key in columnMap);
    if (!entries.length) return getOrder(id);

    const sets = entries.map(([key]) => `${columnMap[key]} = ?`);
    const values = entries.map(([, value]) => value);
    sets.push('updated_at = CURRENT_TIMESTAMP');
    if (changes.status === 'completada' && current.status !== 'completada') {
      sets.push('completed_at = CURRENT_TIMESTAMP');
    } else if (changes.status && changes.status !== 'completada') {
      sets.push('completed_at = NULL');
    }

    db.exec('BEGIN IMMEDIATE');
    try {
      db.prepare(`UPDATE work_orders SET ${sets.join(', ')} WHERE id = ?`).run(...values, id);
      if (changes.status && changes.status !== current.status) {
        statements.insertEvent.run(id, 'status_changed', current.status, changes.status, 'Estado actualizado');
      } else {
        statements.insertEvent.run(id, 'updated', current.status, changes.status ?? current.status, 'Orden actualizada');
      }
      db.exec('COMMIT');
      return getOrder(id);
    } catch (error) {
      db.exec('ROLLBACK');
      throw error;
    }
  }

  function setTaskCompleted(orderId, taskId, completed) {
    const result = statements.setTask.run(completed ? 1 : 0, completed ? 1 : 0, taskId, orderId);
    if (Number(result.changes) === 0) return null;
    statements.insertEvent.run(orderId, completed ? 'task_completed' : 'task_reopened', null, null, 'Lista de trabajo actualizada');
    return getOrder(orderId);
  }

  function listAssets() {
    return db.prepare(`
      SELECT a.id, a.code, a.name, a.category, a.location, a.status, a.manufacturer,
             a.model, a.serial_number AS serialNumber, a.created_at AS createdAt,
             a.updated_at AS updatedAt,
             COUNT(wo.id) AS orderCount,
             SUM(CASE WHEN wo.status NOT IN ('completada', 'cancelada') THEN 1 ELSE 0 END) AS openOrderCount
      FROM assets a
      LEFT JOIN work_orders wo ON wo.asset_id = a.id
      GROUP BY a.id
      ORDER BY a.name COLLATE NOCASE
    `).all().map((row) => ({
      ...row,
      id: Number(row.id),
      orderCount: Number(row.orderCount ?? 0),
      openOrderCount: Number(row.openOrderCount ?? 0)
    }));
  }

  function createAsset(data) {
    if (statements.getAssetByCode.get(data.code)) {
      const error = new Error('Ya existe un equipo con ese código.');
      error.code = 'DUPLICATE_ASSET_CODE';
      throw error;
    }
    const inserted = statements.insertAsset.run(
      data.code,
      data.name,
      data.category,
      data.location,
      data.status,
      data.manufacturer,
      data.model,
      data.serialNumber
    );
    return rowToAsset(statements.getAsset.get(Number(inserted.lastInsertRowid)));
  }

  function getDashboard() {
    const today = localDate();
    const monthStart = `${today.slice(0, 7)}-01`;
    const totals = db.prepare(`
      SELECT
        SUM(CASE WHEN status NOT IN ('completada', 'cancelada') THEN 1 ELSE 0 END) AS activeOrders,
        SUM(CASE WHEN priority = 'critica' AND status NOT IN ('completada', 'cancelada') THEN 1 ELSE 0 END) AS criticalOpen,
        SUM(CASE WHEN scheduled_date < ? AND status NOT IN ('completada', 'cancelada') THEN 1 ELSE 0 END) AS overdue,
        SUM(CASE WHEN scheduled_date = ? AND status NOT IN ('completada', 'cancelada') THEN 1 ELSE 0 END) AS dueToday,
        SUM(CASE WHEN completed_at >= ? THEN 1 ELSE 0 END) AS completedThisMonth,
        COUNT(*) AS totalOrders
      FROM work_orders
    `).get(today, today, monthStart);

    const byStatus = db.prepare(`
      SELECT status, COUNT(*) AS count FROM work_orders GROUP BY status
    `).all().map((row) => ({ status: row.status, count: Number(row.count) }));

    const byPriority = db.prepare(`
      SELECT priority, COUNT(*) AS count
      FROM work_orders
      WHERE status NOT IN ('completada', 'cancelada')
      GROUP BY priority
    `).all().map((row) => ({ priority: row.priority, count: Number(row.count) }));

    const upcoming = db.prepare(`${ORDER_SELECT}
      WHERE wo.status NOT IN ('completada', 'cancelada')
      ORDER BY
        CASE WHEN wo.scheduled_date IS NULL THEN 1 ELSE 0 END,
        wo.scheduled_date,
        CASE wo.priority WHEN 'critica' THEN 1 WHEN 'alta' THEN 2 WHEN 'media' THEN 3 ELSE 4 END
      LIMIT 6
    `).all().map(rowToOrder);

    return {
      totals: Object.fromEntries(Object.entries(totals).map(([key, value]) => [key, Number(value ?? 0)])),
      byStatus,
      byPriority,
      upcoming,
      generatedAt: new Date().toISOString()
    };
  }

  function exportAll() {
    return {
      format: 'geecko-maintenance-backup',
      version: 1,
      exportedAt: new Date().toISOString(),
      assets: listAssets(),
      workOrders: listOrders().map((order) => getOrder(order.id))
    };
  }

  return {
    close: () => db.close(),
    createOrder,
    listOrders,
    getOrder,
    updateOrder,
    setTaskCompleted,
    listAssets,
    createAsset,
    getDashboard,
    exportAll
  };
}
