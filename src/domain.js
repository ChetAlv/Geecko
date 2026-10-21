export const ORDER_STATUSES = Object.freeze([
  'solicitada',
  'planificada',
  'en_progreso',
  'en_espera',
  'completada',
  'cancelada'
]);

export const ORDER_PRIORITIES = Object.freeze(['critica', 'alta', 'media', 'baja']);
export const ORDER_TYPES = Object.freeze(['correctivo', 'preventivo', 'inspeccion', 'mejora']);
export const ASSET_STATUSES = Object.freeze(['operativo', 'detenido', 'mantenimiento', 'baja']);

const DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/;

export class ValidationError extends Error {
  constructor(message, fields = {}) {
    super(message);
    this.name = 'ValidationError';
    this.fields = fields;
  }
}

function text(value, maxLength, fallback = '') {
  if (value === undefined || value === null) return fallback;
  return String(value).trim().slice(0, maxLength);
}

function nullableInteger(value) {
  if (value === '' || value === null || value === undefined) return null;
  const parsed = Number(value);
  return Number.isInteger(parsed) && parsed > 0 ? parsed : null;
}

function nullableDate(value, field, fields) {
  if (value === '' || value === null || value === undefined) return null;
  const normalized = String(value);
  const match = normalized.match(DATE_PATTERN);
  let valid = Boolean(match);
  if (valid) {
    const [year, month, day] = normalized.split('-').map(Number);
    const date = new Date(Date.UTC(year, month - 1, day));
    valid = date.getUTCFullYear() === year && date.getUTCMonth() === month - 1 && date.getUTCDate() === day;
  }
  if (!valid) {
    fields[field] = 'Usa una fecha válida.';
    return null;
  }
  return normalized;
}

function enumValue(value, allowed, fallback, field, fields) {
  const normalized = text(value, 40, fallback);
  if (!allowed.includes(normalized)) {
    fields[field] = 'Selecciona una opción válida.';
    return fallback;
  }
  return normalized;
}

export function validateWorkOrder(input = {}, { partial = false } = {}) {
  const fields = {};
  const result = {};

  if (!partial || input.title !== undefined) {
    result.title = text(input.title, 120);
    if (result.title.length < 3) fields.title = 'Escribe un título de al menos 3 caracteres.';
  }

  if (!partial || input.description !== undefined) {
    result.description = text(input.description, 4000);
    if (result.description.length < 5) fields.description = 'Describe brevemente el trabajo requerido.';
  }

  if (!partial || input.type !== undefined) {
    result.type = enumValue(input.type, ORDER_TYPES, 'correctivo', 'type', fields);
  }

  if (!partial || input.priority !== undefined) {
    result.priority = enumValue(input.priority, ORDER_PRIORITIES, 'media', 'priority', fields);
  }

  if (!partial || input.status !== undefined) {
    result.status = enumValue(input.status, ORDER_STATUSES, 'solicitada', 'status', fields);
  }

  if (!partial || input.assetId !== undefined) result.assetId = nullableInteger(input.assetId);
  if (!partial || input.location !== undefined) result.location = text(input.location, 160);
  if (!partial || input.assignedTo !== undefined) result.assignedTo = text(input.assignedTo, 120);
  if (!partial || input.requestedBy !== undefined) result.requestedBy = text(input.requestedBy, 120);
  if (!partial || input.scheduledDate !== undefined) {
    result.scheduledDate = nullableDate(input.scheduledDate, 'scheduledDate', fields);
  }
  if (!partial || input.safetyNotes !== undefined) result.safetyNotes = text(input.safetyNotes, 2000);

  if (!partial || input.estimatedHours !== undefined) {
    if (input.estimatedHours === '' || input.estimatedHours === null || input.estimatedHours === undefined) {
      result.estimatedHours = null;
    } else {
      const hours = Number(input.estimatedHours);
      if (!Number.isFinite(hours) || hours < 0 || hours > 10000) {
        fields.estimatedHours = 'Ingresa una cantidad de horas válida.';
      } else {
        result.estimatedHours = Math.round(hours * 100) / 100;
      }
    }
  }

  if (!partial || input.tasks !== undefined) {
    result.tasks = Array.isArray(input.tasks)
      ? input.tasks.map((item) => text(item, 300)).filter(Boolean).slice(0, 100)
      : [];
  }

  if (Object.keys(fields).length) {
    throw new ValidationError('Revisa los datos de la orden.', fields);
  }

  return result;
}

export function validateAsset(input = {}) {
  const fields = {};
  const result = {
    code: text(input.code, 40).toUpperCase(),
    name: text(input.name, 120),
    category: text(input.category, 80),
    location: text(input.location, 160),
    status: enumValue(input.status, ASSET_STATUSES, 'operativo', 'status', fields),
    manufacturer: text(input.manufacturer, 100),
    model: text(input.model, 100),
    serialNumber: text(input.serialNumber, 100)
  };

  if (result.code.length < 2) fields.code = 'Ingresa un código de al menos 2 caracteres.';
  if (result.name.length < 2) fields.name = 'Ingresa el nombre del equipo.';
  if (!result.location) fields.location = 'Ingresa la ubicación del equipo.';

  if (Object.keys(fields).length) {
    throw new ValidationError('Revisa los datos del equipo.', fields);
  }

  return result;
}
