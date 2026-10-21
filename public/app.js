import { buildOrderPayload } from './order-payload.js';

const detectedPlatform = /Android/i.test(navigator.userAgent) ? 'android' : 'desktop';
document.documentElement.dataset.platform = detectedPlatform;
document.documentElement.classList.toggle('is-mobile-app', detectedPlatform === 'android');

const STATUS_LABELS = {
  solicitada: 'Recibida',
  planificada: 'En diagnóstico',
  pendiente_autorizacion: 'Pendiente de autorización',
  en_progreso: 'En reparación',
  en_espera: 'En espera',
  completada: 'Lista para entregar',
  entregada: 'Entregada',
  cancelada: 'Cancelada'
};

const PRIORITY_LABELS = {
  critica: 'Urgente',
  alta: 'Alta',
  media: 'Media',
  baja: 'Baja'
};

const TYPE_LABELS = {
  correctivo: 'Reparación correctiva',
  preventivo: 'Mantenimiento preventivo',
  inspeccion: 'Diagnóstico',
  mejora: 'Actualización o mejora',
  garantia: 'Regreso por garantía'
};

const ASSET_STATUS_LABELS = {
  operativo: 'Funcional',
  detenido: 'Con falla',
  mantenimiento: 'En reparación',
  listo: 'Listo para entregar',
  entregado: 'Entregado',
  baja: 'No reparable'
};

const DELIVERY_CONDITION_LABELS = {
  reparado: 'Reparado y funcionando',
  funcionamiento_parcial: 'Funcionamiento parcial',
  sin_reparar: 'Se entrega sin reparar',
  falla_no_reproducida: 'No se reprodujo la falla',
  no_reparable: 'Equipo no reparable'
};

const DELIVERY_TESTER_LABELS = {
  cliente: 'Probado por el cliente',
  tecnico: 'Probado por un técnico (cliente no presente)'
};

const QUOTE_STATUS_LABELS = {
  draft: 'Borrador',
  pending: 'Pendiente',
  approved: 'Aprobada',
  rejected: 'Rechazada'
};

const QUOTE_ITEM_LABELS = {
  mano_obra: 'Mano de obra',
  repuesto: 'Repuesto',
  otro: 'Otro'
};

const AUTHORIZATION_METHOD_LABELS = {
  firma: 'Firma en cotización',
  presencial: 'Confirmación presencial',
  telefono: 'Llamada telefónica',
  whatsapp: 'WhatsApp',
  correo: 'Correo electrónico'
};

const PAYMENT_TYPE_LABELS = {
  advance: 'Anticipo',
  payment: 'Pago'
};

const PAYMENT_METHOD_LABELS = {
  cash: 'Efectivo',
  card: 'Tarjeta',
  transfer: 'Transferencia',
  deposit: 'Depósito bancario',
  other: 'Otro'
};

const WARRANTY_DECISION_LABELS = {
  pending: 'Pendiente de evaluación',
  covered: 'Cubierta por garantía',
  not_covered: 'Fuera de cobertura'
};

const ROLE_LABELS = {
  admin: 'Administrador',
  reception: 'Recepci\u00f3n',
  technician: 'T\u00e9cnico'
};

const PHOTO_CATEGORY_LABELS = {
  recepcion: 'Recepci\u00f3n',
  diagnostico: 'Diagn\u00f3stico',
  reparacion: 'Reparaci\u00f3n',
  entrega: 'Entrega'
};

const STATUS_TRANSITIONS = {
  solicitada: ['solicitada', 'planificada', 'en_progreso', 'en_espera', 'cancelada'],
  planificada: ['planificada', 'solicitada', 'en_progreso', 'en_espera', 'cancelada'],
  pendiente_autorizacion: ['pendiente_autorizacion', 'cancelada'],
  en_progreso: ['en_progreso', 'planificada', 'en_espera', 'completada', 'cancelada'],
  en_espera: ['en_espera', 'planificada', 'en_progreso', 'cancelada'],
  completada: ['completada', 'en_progreso', 'entregada'],
  entregada: ['entregada'],
  cancelada: ['cancelada', 'solicitada']
};

const state = {
  runtime: {
    platform: detectedPlatform,
    mobile: detectedPlatform === 'android',
    appVersion: '',
    storageMode: 'local'
  },
  dashboard: null,
  orders: [],
  assets: [],
  customers: [],
  inventory: [],
  purchaseReports: [],
  purchaseReceipts: [],
  inventoryMovements: [],
  sales: [],
  backupOverview: null,
  session: null,
  statistics: null,
  users: [],
  userActivity: [],
  syncStatus: null,
  syncing: false,
  photoUrls: [],
  qrCache: new Map(),
  activeView: 'dashboard',
  selectedOrder: null
};

const tauriInvoke = window.__TAURI__?.core?.invoke;
let syncIntervalId = null;
let syncTimeoutId = null;
let syncRequest = null;

const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];

function escapeHtml(value = '') {
  return String(value)
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
    .replaceAll("'", '&#039;');
}

function formatDate(value, options = {}) {
  if (!value) return 'Sin programar';
  const normalized = value.length === 10 ? `${value}T12:00:00` : `${value.replace(' ', 'T')}Z`;
  const date = new Date(normalized);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat('es-GT', {
    day: '2-digit',
    month: 'short',
    year: options.year ? 'numeric' : undefined
  }).format(date).replace('.', '');
}

function formatDateTime(value) {
  if (!value) return '';
  const date = new Date(`${value.replace(' ', 'T')}Z`);
  return new Intl.DateTimeFormat('es-GT', {
    day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit'
  }).format(date);
}

function formatCurrency(value) {
  return new Intl.NumberFormat('es-GT', {
    style: 'currency',
    currency: 'GTQ',
    minimumFractionDigits: 2
  }).format(Number(value) || 0);
}

async function api(path, options = {}) {
  if (!tauriInvoke) throw new Error('Abre Geecko desde la aplicación instalada.');
  const url = new URL(path, 'http://geecko.local');
  const body = options.body ? JSON.parse(options.body) : {};
  const method = options.method || 'GET';

  try {
    if (url.pathname === '/api/runtime') return await tauriInvoke('get_runtime_info');
    if (url.pathname === '/api/sync/status') return await tauriInvoke('get_sync_status');
    if (url.pathname === '/api/sync/config' && method === 'POST') {
      return await tauriInvoke('configure_sync', { input: body });
    }
    if (url.pathname === '/api/sync/run' && method === 'POST') return await tauriInvoke('sync_now');
    if (url.pathname === '/api/auth/status') return await tauriInvoke('get_auth_status');
    if (url.pathname === '/api/auth/setup' && method === 'POST') {
      return await tauriInvoke('create_initial_admin', { input: body });
    }
    if (url.pathname === '/api/auth/login' && method === 'POST') {
      return await tauriInvoke('login', { input: body });
    }
    if (url.pathname === '/api/auth/logout' && method === 'POST') return await tauriInvoke('logout');
    if (url.pathname === '/api/statistics' && method === 'GET') {
      return await tauriInvoke('get_statistics', { rangeDays: Number(url.searchParams.get('range') || 90) });
    }
    if (url.pathname === '/api/users' && method === 'GET') return await tauriInvoke('list_users');
    if (url.pathname === '/api/users' && method === 'POST') return await tauriInvoke('create_user', { input: body });
    if (url.pathname === '/api/user-activity' && method === 'GET') {
      return await tauriInvoke('list_user_activity', { limit: Number(url.searchParams.get('limit') || 100) });
    }
    const userActiveMatch = url.pathname.match(/^\/api\/users\/(\d+)\/active$/);
    if (userActiveMatch && method === 'PATCH') {
      return await tauriInvoke('set_user_active', { id: Number(userActiveMatch[1]), input: body });
    }
    const userPasswordMatch = url.pathname.match(/^\/api\/users\/(\d+)\/password$/);
    if (userPasswordMatch && method === 'PATCH') {
      return await tauriInvoke('reset_user_password', { id: Number(userPasswordMatch[1]), input: body });
    }
    if (url.pathname === '/api/dashboard') return await tauriInvoke('get_dashboard');
    if (url.pathname === '/api/backups' && method === 'GET') {
      return await tauriInvoke('list_backups');
    }
    if (url.pathname === '/api/backups/automatic' && method === 'POST') {
      return await tauriInvoke('ensure_automatic_backup');
    }
    if (url.pathname === '/api/backups/manual' && method === 'POST') {
      return await tauriInvoke('create_manual_backup');
    }
    if (url.pathname === '/api/backups/restore' && method === 'POST') {
      return await tauriInvoke('restore_backup', { input: body });
    }
    if (url.pathname === '/api/assets' && method === 'GET') {
      return { assets: await tauriInvoke('list_assets') };
    }
    if (url.pathname === '/api/assets' && method === 'POST') {
      return await tauriInvoke('create_asset', { input: body });
    }
    if (url.pathname === '/api/customers' && method === 'GET') {
      return { customers: await tauriInvoke('list_customers') };
    }
    if (url.pathname === '/api/customers' && method === 'POST') {
      return await tauriInvoke('create_customer', { input: body });
    }
    const customerMatch = url.pathname.match(/^\/api\/customers\/(\d+)$/);
    if (customerMatch && method === 'PATCH') {
      return await tauriInvoke('update_customer', {
        id: Number(customerMatch[1]),
        input: body
      });
    }
    if (url.pathname === '/api/inventory' && method === 'GET') {
      return { items: await tauriInvoke('list_inventory_items') };
    }
    if (url.pathname === '/api/inventory' && method === 'POST') {
      return await tauriInvoke('create_inventory_item', { input: body });
    }
    const inventoryMatch = url.pathname.match(/^\/api\/inventory\/(\d+)$/);
    if (inventoryMatch && method === 'PATCH') {
      return await tauriInvoke('update_inventory_item', {
        id: Number(inventoryMatch[1]),
        input: body
      });
    }
    if (url.pathname === '/api/purchase-reports' && method === 'POST') {
      return await tauriInvoke('create_purchase_report', { input: body });
    }
    if (url.pathname === '/api/purchase-reports' && method === 'GET') {
      return { reports: await tauriInvoke('list_purchase_reports') };
    }
    const receiptMatch = url.pathname.match(/^\/api\/purchase-reports\/(\d+)\/receipts$/);
    if (receiptMatch && method === 'POST') {
      return await tauriInvoke('receive_purchase', {
        reportId: Number(receiptMatch[1]),
        input: body
      });
    }
    if (url.pathname === '/api/purchase-receipts' && method === 'GET') {
      return { receipts: await tauriInvoke('list_purchase_receipts') };
    }
    if (url.pathname === '/api/inventory-movements' && method === 'GET') {
      return { movements: await tauriInvoke('list_inventory_movements') };
    }
    if (url.pathname === '/api/sales' && method === 'GET') {
      return { sales: await tauriInvoke('list_sales') };
    }
    if (url.pathname === '/api/sales' && method === 'POST') {
      return await tauriInvoke('create_sale', { input: body });
    }
    const saleCancelMatch = url.pathname.match(/^\/api\/sales\/(\d+)\/cancel$/);
    if (saleCancelMatch && method === 'POST') {
      return await tauriInvoke('cancel_sale', {
        id: Number(saleCancelMatch[1]),
        input: body
      });
    }
    if (url.pathname === '/api/orders' && method === 'GET') {
      const filters = {
        status: url.searchParams.get('status') || '',
        priority: url.searchParams.get('priority') || '',
        search: url.searchParams.get('search') || ''
      };
      return { orders: await tauriInvoke('list_orders', { filters }) };
    }
    if (url.pathname === '/api/orders' && method === 'POST') {
      return await tauriInvoke('create_order', { input: body });
    }

    const taskMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/tasks\/(\d+)$/);
    if (taskMatch && method === 'PATCH') {
      return await tauriInvoke('set_task_completed', {
        orderId: Number(taskMatch[1]),
        taskId: Number(taskMatch[2]),
        completed: Boolean(body.completed)
      });
    }

    const quoteMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/quote$/);
    if (quoteMatch && method === 'PUT') {
      return await tauriInvoke('save_diagnosis_quote', {
        id: Number(quoteMatch[1]),
        input: body
      });
    }
    if (quoteMatch && method === 'POST') {
      return await tauriInvoke('set_quote_decision', {
        id: Number(quoteMatch[1]),
        input: body
      });
    }

    const resourcesMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/resources$/);
    if (resourcesMatch && method === 'PUT') {
      return await tauriInvoke('save_order_resources', {
        id: Number(resourcesMatch[1]),
        input: body
      });
    }

    const paymentMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/payments$/);
    if (paymentMatch && method === 'POST') {
      return await tauriInvoke('create_order_payment', {
        orderId: Number(paymentMatch[1]),
        input: body
      });
    }
    const paymentCancelMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/payments\/(\d+)\/cancel$/);
    if (paymentCancelMatch && method === 'POST') {
      return await tauriInvoke('cancel_order_payment', {
        orderId: Number(paymentCancelMatch[1]),
        paymentId: Number(paymentCancelMatch[2]),
        input: body
      });
    }

    const warrantyMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/warranty$/);
    if (warrantyMatch && method === 'PUT') {
      return await tauriInvoke('save_order_warranty', {
        orderId: Number(warrantyMatch[1]),
        input: body
      });
    }
    const warrantyReturnMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/warranty-return$/);
    if (warrantyReturnMatch && method === 'POST') {
      return await tauriInvoke('create_warranty_return', {
        originalOrderId: Number(warrantyReturnMatch[1]),
        input: body
      });
    }
    const warrantyClaimMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/warranty-claim$/);
    if (warrantyClaimMatch && method === 'PUT') {
      return await tauriInvoke('resolve_warranty_claim', {
        returnOrderId: Number(warrantyClaimMatch[1]),
        input: body
      });
    }

    const orderPhotoMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/photos$/);
    if (orderPhotoMatch && method === 'POST') {
      return await tauriInvoke('add_order_photo', { orderId: Number(orderPhotoMatch[1]), input: body });
    }
    const photoDataMatch = url.pathname.match(/^\/api\/photos\/(\d+)\/data$/);
    if (photoDataMatch && method === 'GET') {
      return await tauriInvoke('get_order_photo_data', { photoId: Number(photoDataMatch[1]) });
    }
    const deletePhotoMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/photos\/(\d+)$/);
    if (deletePhotoMatch && method === 'DELETE') {
      return await tauriInvoke('delete_order_photo', { orderId: Number(deletePhotoMatch[1]), photoId: Number(deletePhotoMatch[2]) });
    }
    const orderQrMatch = url.pathname.match(/^\/api\/orders\/(\d+)\/qr$/);
    if (orderQrMatch && method === 'GET') {
      return await tauriInvoke('generate_order_qr', { orderId: Number(orderQrMatch[1]) });
    }

    const orderMatch = url.pathname.match(/^\/api\/orders\/(\d+)$/);
    if (orderMatch && method === 'GET') {
      return await tauriInvoke('get_order', { id: Number(orderMatch[1]) });
    }
    if (orderMatch && method === 'PATCH') {
      return await tauriInvoke('update_order', { id: Number(orderMatch[1]), changes: body });
    }
    throw new Error('Operación local no reconocida.');
  } catch (reason) {
    const message = typeof reason === 'string'
      ? reason
      : reason?.error || reason?.message || 'No fue posible completar la operación.';
    const error = new Error(message);
    error.fields = reason?.fields || {};
    throw error;
  } finally {
    if (method !== 'GET' && !url.pathname.startsWith('/api/sync/') && !url.pathname.startsWith('/api/auth/')) {
      scheduleSynchronization(350);
    }
  }
}

function statusBadge(status) {
  return `<span class="status-badge status-${escapeHtml(status)}"><span></span>${escapeHtml(STATUS_LABELS[status] || status)}</span>`;
}

function priorityBadge(priority) {
  return `<span class="priority-badge priority-${escapeHtml(priority)}"><span></span>${escapeHtml(PRIORITY_LABELS[priority] || priority)}</span>`;
}

function showToast(message, tone = 'success') {
  const toast = document.createElement('div');
  toast.className = `toast toast-${tone}`;
  toast.innerHTML = `<span>${tone === 'success' ? '✓' : '!'}</span><p>${escapeHtml(message)}</p>`;
  $('#toast-stack').append(toast);
  setTimeout(() => toast.classList.add('is-visible'), 20);
  setTimeout(() => {
    toast.classList.remove('is-visible');
    setTimeout(() => toast.remove(), 250);
  }, 3800);
}

function renderSyncStatus() {
  const status = state.syncStatus;
  const configured = Boolean(status?.configured);
  const hasError = Boolean(status?.lastError);
  const title = state.syncing ? 'Sincronizando' : configured ? (hasError ? 'Sin conexión' : 'Datos compartidos') : 'Modo local';
  const copy = state.syncing
    ? 'Enviando y recibiendo cambios…'
    : configured
      ? hasError
        ? status.lastError
        : status.pendingChanges
          ? `${status.pendingChanges} cambio${status.pendingChanges === 1 ? '' : 's'} pendiente${status.pendingChanges === 1 ? '' : 's'}`
          : 'PostgreSQL actualizado'
      : 'Sin configurar';
  const sidebar = $('#sidebar-sync-status');
  if (sidebar) {
    sidebar.classList.toggle('is-syncing', state.syncing);
    sidebar.classList.toggle('is-error', configured && hasError && !state.syncing);
    sidebar.classList.toggle('is-offline', !configured);
    $('#sidebar-sync-title').textContent = title;
    $('#sidebar-sync-copy').textContent = copy;
  }
  const mobileBadge = $('#mobile-sync-badge');
  if (mobileBadge) {
    mobileBadge.classList.toggle('is-syncing', state.syncing);
    mobileBadge.classList.toggle('is-error', configured && hasError && !state.syncing);
    mobileBadge.innerHTML = `<i></i> ${state.syncing ? 'Enviando' : configured ? (hasError ? 'Sin red' : 'En línea') : 'Local'}`;
  }
  const mobileCopy = $('#mobile-runtime-copy');
  if (mobileCopy) {
    mobileCopy.textContent = configured
      ? copy
      : `Datos locales · versión ${state.runtime.appVersion || '0.1.0'}`;
  }
  const pill = $('#sync-state-pill');
  if (pill) {
    pill.textContent = state.syncing ? 'Sincronizando' : configured ? (hasError ? 'Con error' : 'Conectado') : 'Sin configurar';
    pill.className = `sync-state-pill ${state.syncing ? 'is-syncing' : configured ? (hasError ? 'is-error' : 'is-online') : ''}`;
  }
  const pending = $('#sync-pending-count');
  if (pending) {
    const count = status?.pendingChanges || 0;
    pending.textContent = `${count} cambio${count === 1 ? '' : 's'} pendiente${count === 1 ? '' : 's'}`;
  }
  const lastCopy = $('#sync-last-copy');
  if (lastCopy) {
    lastCopy.textContent = hasError
      ? status.lastError
      : status?.lastSyncedAt
        ? `Última sincronización: ${formatDateTime(status.lastSyncedAt)}`
        : 'Aún no se ha sincronizado';
  }
  const form = $('#sync-settings-form');
  if (form && status && form.dataset.dirty !== 'true') {
    form.elements.serverUrl.value = status.serverUrl || '';
    form.elements.deviceId.value = status.deviceId || '';
    form.elements.devicePrefix.value = status.devicePrefix || '';
    form.elements.token.value = '';
    form.elements.token.required = !status.tokenConfigured;
    form.elements.token.placeholder = status.tokenConfigured
      ? 'Token guardado · déjalo vacío para conservarlo'
      : 'Pega el token generado en el servidor';
  }
}

async function refreshSyncStatus() {
  state.syncStatus = await api('/api/sync/status');
  renderSyncStatus();
  return state.syncStatus;
}

async function runSynchronization({ quiet = true } = {}) {
  if (!state.session) return null;
  if (!state.syncStatus) await refreshSyncStatus();
  if (!state.syncStatus?.configured) {
    if (!quiet) showToast('Configura primero el servidor en Usuarios.', 'error');
    return null;
  }
  if (syncRequest) return syncRequest;
  state.syncing = true;
  renderSyncStatus();
  syncRequest = (async () => {
    try {
      const result = await api('/api/sync/run', { method: 'POST', body: '{}' });
      state.syncStatus = result.status;
      if (result.remoteChanges > 0) {
        const selectedOrderId = state.selectedOrder?.id;
        await refreshAll();
        if (selectedOrderId && $('#detail-drawer-layer')?.classList.contains('is-open')) {
          await openOrderDetail(selectedOrderId);
        }
      }
      if (!quiet) {
        showToast(`Sincronización completa: ${result.pushed} enviados y ${result.remoteChanges} recibidos.`);
      }
      return result;
    } catch (error) {
      try { state.syncStatus = await api('/api/sync/status'); } catch (_) { /* conserva el estado previo */ }
      if (!quiet) showToast(error.message, 'error');
      return null;
    } finally {
      state.syncing = false;
      syncRequest = null;
      renderSyncStatus();
    }
  })();
  return syncRequest;
}

function scheduleSynchronization(delay = 350) {
  if (!state.session || !state.syncStatus?.configured) return;
  clearTimeout(syncTimeoutId);
  syncTimeoutId = setTimeout(() => runSynchronization({ quiet: true }), delay);
}

function startSynchronization() {
  clearInterval(syncIntervalId);
  syncIntervalId = setInterval(() => {
    if (document.visibilityState === 'visible') runSynchronization({ quiet: true });
  }, 5000);
  scheduleSynchronization(100);
}

function stopSynchronization() {
  clearInterval(syncIntervalId);
  clearTimeout(syncTimeoutId);
  syncIntervalId = null;
  syncTimeoutId = null;
}

function setLayer(layer, open) {
  layer.hidden = !open;
  requestAnimationFrame(() => {
    layer.classList.toggle('is-open', open);
    document.body.classList.toggle('has-drawer', $$('.drawer-layer.is-open').length > 0);
  });
}

function openOrderDrawer(order = null) {
  const form = $('#order-form');
  form.reset();
  clearFormErrors(form);
  const isWarrantyReturn = order?.type === 'garantia';
  form.elements.type.querySelector('[value="garantia"]').hidden = !isWarrantyReturn;
  [...form.elements.type.options].forEach((option) => {
    option.disabled = isWarrantyReturn && option.value !== 'garantia';
  });
  form.elements.orderId.value = order?.id || '';
  $$('[name="assetMode"], [name="customerMode"]', form).forEach((radio) => { radio.disabled = false; });
  if (order) {
    form.elements.title.value = order.title || '';
    form.elements.description.value = order.description || '';
    form.elements.type.value = order.type || 'correctivo';
    form.elements.priority.value = order.priority || 'media';
    $('[name="assetMode"][value="existing"]', form).checked = true;
    $('[name="assetMode"][value="new"]', form).disabled = true;
    form.elements.assetId.value = order.assetId || '';
    $('[name="customerMode"][value="existing"]', form).checked = true;
    $('[name="customerMode"][value="new"]', form).disabled = true;
    form.elements.customerId.value = order.customerId || '';
    form.elements.location.value = order.location || '';
    form.elements.assignedTo.value = order.assignedTo || '';
    form.elements.scheduledDate.value = order.scheduledDate || '';
    form.elements.estimatedHours.value = order.estimatedHours ?? '';
    form.elements.tasks.value = (order.tasks || []).map((task) => task.description).join('\n');
    form.elements.safetyNotes.value = order.safetyNotes || '';
    form.elements.observations.value = order.observations || '';
    const knownAccessories = $$('[name="accessories"]', form).map((input) => input.value);
    $$('[name="accessories"]', form).forEach((input) => {
      input.checked = (order.accessories || []).includes(input.value);
    });
    form.elements.otherAccessories.value = (order.accessories || [])
      .filter((item) => !knownAccessories.includes(item)).join('\n');
    $('#order-form-eyebrow').textContent = 'EDICIÓN DE ORDEN';
    $('#new-order-title').textContent = `Editar ${order.orderNumber}`;
    $('#order-form-subtitle').textContent = 'Corrige el servicio, cliente, equipo, seguimiento, accesorios y tareas.';
    $('#order-submit-button').textContent = 'Guardar cambios';
  } else {
    $('#order-form-eyebrow').textContent = 'RECEPCIÓN DE EQUIPO';
    $('#new-order-title').textContent = 'Nueva orden de servicio';
    $('#order-form-subtitle').textContent = 'Registra el equipo, sus accesorios y el trabajo solicitado en un solo paso.';
    $('#order-submit-button').textContent = 'Crear orden';
  }
  syncAssetMode();
  syncCustomerMode();
  setLayer($('#order-drawer-layer'), true);
  setTimeout(() => form.elements.title.focus(), 220);
}

function closeOrderDrawer(restoreDetail = true) {
  const editing = Boolean(Number($('#order-form').elements.orderId.value));
  setLayer($('#order-drawer-layer'), false);
  if (restoreDetail && editing && state.selectedOrder) {
    setLayer($('#detail-drawer-layer'), true);
  }
}

function openCustomerDrawer(customer = null) {
  const form = $('#customer-form');
  form.reset();
  clearFormErrors(form);
  form.elements.id.value = customer?.id || '';
  ['name', 'phone', 'email', 'address', 'company', 'taxId', 'notes'].forEach((field) => {
    form.elements[field].value = customer?.[field] || '';
  });
  $('#customer-form-title').textContent = customer ? 'Editar cliente' : 'Agregar cliente';
  $('.submit-button', form).textContent = customer ? 'Guardar cambios' : 'Guardar cliente';
  setLayer($('#customer-drawer-layer'), true);
  setTimeout(() => form.elements.name.focus(), 220);
}

function closeCustomerDrawer() {
  setLayer($('#customer-drawer-layer'), false);
}

function openInventoryDrawer(item = null) {
  const form = $('#inventory-form');
  form.reset();
  clearFormErrors(form);
  form.elements.id.value = item?.id || '';
  form.elements.name.value = item?.name || '';
  form.elements.code.value = item?.code || '';
  form.elements.category.value = item?.category || '';
  form.elements.unit.value = item?.unit || 'unidad';
  form.elements.quantity.value = item?.quantity ?? 0;
  form.elements.minimumStock.value = item?.minimumStock ?? 0;
  $('#inventory-form-title').textContent = item ? 'Editar producto' : 'Agregar producto';
  $('#inventory-form-subtitle').textContent = item
    ? 'Actualiza manualmente sus datos y existencias.'
    : 'Registra las existencias actuales y el nivel que activa una compra.';
  $('.submit-button', form).textContent = item ? 'Guardar cambios' : 'Guardar producto';
  setLayer($('#inventory-drawer-layer'), true);
  setTimeout(() => form.elements.name.focus(), 220);
}

function closeInventoryDrawer() {
  setLayer($('#inventory-drawer-layer'), false);
}

function closePurchaseReport() {
  setLayer($('#purchase-report-drawer-layer'), false);
}

function closeDetail() {
  setLayer($('#detail-drawer-layer'), false);
  releasePhotoUrls();
  state.selectedOrder = null;
}

function quoteItemTemplate(item = {}) {
  const quantity = item.quantity ?? 1;
  const unitPrice = item.unitPrice ?? 0;
  return `
    <div class="quote-line">
      <div class="quote-line-main">
        <select class="quote-kind" aria-label="Tipo de concepto">
          ${Object.entries(QUOTE_ITEM_LABELS).map(([value, label]) => `<option value="${value}" ${item.kind === value ? 'selected' : ''}>${escapeHtml(label)}</option>`).join('')}
        </select>
        <input class="quote-description" maxlength="300" value="${escapeHtml(item.description || '')}" placeholder="Descripción del trabajo o repuesto" aria-label="Descripción del concepto">
      </div>
      <input class="quote-quantity" type="number" min="0.01" max="1000000" step="0.01" value="${escapeHtml(quantity)}" aria-label="Cantidad">
      <input class="quote-unit-price" type="number" min="0" step="0.01" value="${escapeHtml(unitPrice)}" aria-label="Precio unitario">
      <strong class="quote-line-total">${escapeHtml(formatCurrency(Number(quantity) * Number(unitPrice)))}</strong>
      <button type="button" class="icon-button quote-remove-button" data-action="remove-quote-item" aria-label="Quitar concepto">×</button>
    </div>
  `;
}

function collectQuoteInput() {
  const form = $('#quote-form');
  return {
    diagnosis: form.elements.diagnosis.value.trim(),
    recommendation: form.elements.recommendation.value.trim(),
    discount: form.elements.discount.value || '0',
    items: $$('.quote-line', form).map((row) => ({
      kind: $('.quote-kind', row).value,
      description: $('.quote-description', row).value.trim(),
      quantity: $('.quote-quantity', row).value,
      unitPrice: $('.quote-unit-price', row).value
    }))
  };
}

function updateQuoteTotals() {
  const form = $('#quote-form');
  if (!form) return;
  let subtotal = 0;
  $$('.quote-line', form).forEach((row) => {
    const quantity = Math.max(0, Number($('.quote-quantity', row).value) || 0);
    const price = Math.max(0, Number($('.quote-unit-price', row).value) || 0);
    const lineTotal = Math.round(quantity * price * 100) / 100;
    subtotal += lineTotal;
    $('.quote-line-total', row).textContent = formatCurrency(lineTotal);
  });
  subtotal = Math.round(subtotal * 100) / 100;
  const discount = Math.max(0, Number(form.elements.discount.value) || 0);
  const total = Math.max(0, subtotal - discount);
  $('#quote-subtotal').textContent = formatCurrency(subtotal);
  $('#quote-discount-total').textContent = `− ${formatCurrency(discount)}`;
  $('#quote-total').textContent = formatCurrency(total);
}

function renderQuoteDrawer(order) {
  const form = $('#quote-form');
  form.reset();
  clearFormErrors(form);
  form.elements.orderId.value = order.id;
  form.elements.diagnosis.value = order.diagnosis || '';
  form.elements.recommendation.value = order.recommendation || '';
  form.elements.discount.value = Number(order.quoteDiscount || 0).toFixed(2);
  form.elements.authorizedBy.value = order.authorizedBy || order.requestedBy || '';
  form.elements.method.value = order.authorizationMethod || '';
  form.elements.authorizationNote.value = order.authorizationNote || '';
  $('#quote-title').textContent = `${order.orderNumber} · Cotización`;
  $('#quote-order-summary').innerHTML = `
    <div><span>Cliente</span><strong>${escapeHtml(order.requestedBy || 'No indicado')}</strong></div>
    <div><span>Equipo</span><strong>${escapeHtml([order.assetName, order.assetCode].filter(Boolean).join(' · ') || 'No identificado')}</strong></div>
    <div><span>Servicio solicitado</span><strong>${escapeHtml(order.title)}</strong></div>
  `;
  const items = order.quoteItems?.length ? order.quoteItems : [{}];
  $('#quote-items').innerHTML = items.map(quoteItemTemplate).join('');

  const status = order.quoteStatus || 'draft';
  const pending = status === 'pending';
  const immutable = ['entregada', 'cancelada', 'completada'].includes(order.status);
  $('#quote-status-pill').textContent = QUOTE_STATUS_LABELS[status] || status;
  $('#quote-status-pill').className = `quote-status-pill quote-status-${status}`;
  $('#quote-authorization-help').textContent = status === 'approved'
    ? `Autorizada ${order.authorizationAt ? formatDateTime(order.authorizationAt) : ''}`
    : status === 'rejected'
      ? `Rechazada ${order.authorizationAt ? formatDateTime(order.authorizationAt) : ''}. Edita y vuelve a solicitar si es necesario.`
      : pending
        ? 'Registra la respuesta recibida del cliente.'
        : 'Solicita autorización antes de registrar una decisión.';
  $$('.quote-decision-actions button', form).forEach((button) => { button.disabled = !pending || immutable; });
  $$('.quote-authorization-section input, .quote-authorization-section select, .quote-authorization-section textarea', form)
    .forEach((control) => { control.disabled = !pending || immutable; });
  $$('.quote-diagnosis-section textarea, .quote-items-section input, .quote-items-section select, .quote-items-section button, .quote-discount-field input', form)
    .forEach((control) => { control.disabled = pending || immutable; });
  $('.quote-save-button', form).hidden = pending || immutable;
  $('[data-action="request-quote-authorization"]', form).hidden = pending || immutable;
  $('[data-action="request-quote-authorization"]', form).textContent = status === 'rejected'
    ? 'Solicitar nuevamente'
    : 'Solicitar autorización';
  updateQuoteTotals();
}

function openQuoteDrawer(order) {
  renderQuoteDrawer(order);
  applyPermissions($('#quote-drawer-layer'));
  setLayer($('#detail-drawer-layer'), false);
  setLayer($('#quote-drawer-layer'), true);
}

function closeQuoteDrawer(restoreDetail = true) {
  setLayer($('#quote-drawer-layer'), false);
  if (restoreDetail && state.selectedOrder) {
    renderOrderDetail(state.selectedOrder);
    setLayer($('#detail-drawer-layer'), true);
  }
}

async function saveDiagnosisQuote(showConfirmation = true) {
  if (!state.selectedOrder) return null;
  const form = $('#quote-form');
  clearFormErrors(form);
  try {
    const order = await api(`/api/orders/${state.selectedOrder.id}/quote`, {
      method: 'PUT',
      body: JSON.stringify(collectQuoteInput())
    });
    state.selectedOrder = order;
    renderQuoteDrawer(order);
    await refreshAll();
    if (showConfirmation) showToast('Diagnóstico y cotización guardados.');
    return order;
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
    return null;
  }
}

async function decideQuote(decision) {
  if (!state.selectedOrder) return null;
  const form = $('#quote-form');
  let current = state.selectedOrder;
  if (decision === 'pending') {
    current = await saveDiagnosisQuote(false);
    if (!current) return null;
  }
  const input = {
    decision,
    method: decision === 'pending' ? '' : form.elements.method.value,
    authorizedBy: decision === 'pending' ? '' : form.elements.authorizedBy.value,
    note: decision === 'pending' ? '' : form.elements.authorizationNote.value
  };
  try {
    const order = await api(`/api/orders/${current.id}/quote`, {
      method: 'POST',
      body: JSON.stringify(input)
    });
    state.selectedOrder = order;
    renderQuoteDrawer(order);
    await refreshAll();
    const message = decision === 'pending'
      ? 'Cotización marcada como pendiente de autorización.'
      : decision === 'approved'
        ? 'Aprobación registrada. La orden pasó a reparación.'
        : 'Rechazo registrado en el historial.';
    showToast(message);
    return order;
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
    return null;
  }
}

function orderPartTemplate(part = {}) {
  const selectedId = Number(part.inventoryItemId) || 0;
  const options = state.inventory.map((item) => {
    const effectiveStock = item.quantity + (item.id === selectedId ? Number(part.quantity || 0) : 0);
    return `<option value="${item.id}" ${item.id === selectedId ? 'selected' : ''}>${escapeHtml(item.code)} · ${escapeHtml(item.name)} · disponibles: ${effectiveStock} ${escapeHtml(item.unit)}</option>`;
  }).join('');
  const missingOption = selectedId && !state.inventory.some((item) => item.id === selectedId)
    ? `<option value="${selectedId}" selected>${escapeHtml(part.itemCode || '')} · ${escapeHtml(part.itemName || 'Producto no disponible')}</option>`
    : '';
  const quantity = part.quantity ?? 1;
  const unitPrice = part.unitPrice ?? 0;
  return `
    <div class="resource-line resource-part-line">
      <div class="resource-product-field">
        <select class="resource-part-select" aria-label="Producto del inventario">
          <option value="">Seleccionar repuesto…</option>
          ${missingOption}${options}
        </select>
        <small class="resource-stock-note"></small>
      </div>
      <input class="resource-part-quantity" type="number" min="1" max="1000000" step="1" value="${escapeHtml(quantity)}" aria-label="Cantidad utilizada">
      <input class="resource-part-price" type="number" min="0" step="0.01" value="${escapeHtml(unitPrice)}" aria-label="Precio unitario">
      <strong class="resource-line-total">${escapeHtml(formatCurrency(Number(quantity) * Number(unitPrice)))}</strong>
      <button type="button" class="icon-button resource-remove-button" data-action="remove-order-part" aria-label="Quitar repuesto">×</button>
    </div>
  `;
}

function orderLaborTemplate(line = {}, order = state.selectedOrder) {
  const hours = line.hours ?? 1;
  const hourlyRate = line.hourlyRate ?? 0;
  return `
    <div class="resource-line resource-labor-line">
      <div class="resource-labor-main">
        <input class="resource-technician" maxlength="160" value="${escapeHtml(line.technician || order?.assignedTo || '')}" placeholder="Técnico responsable" aria-label="Técnico">
        <input class="resource-labor-description" maxlength="500" value="${escapeHtml(line.description || '')}" placeholder="Actividad realizada" aria-label="Actividad realizada">
      </div>
      <input class="resource-labor-hours" type="number" min="0.01" max="10000" step="0.25" value="${escapeHtml(hours)}" aria-label="Horas trabajadas">
      <input class="resource-labor-rate" type="number" min="0" step="0.01" value="${escapeHtml(hourlyRate)}" aria-label="Tarifa por hora">
      <strong class="resource-line-total">${escapeHtml(formatCurrency(Number(hours) * Number(hourlyRate)))}</strong>
      <button type="button" class="icon-button resource-remove-button" data-action="remove-order-labor" aria-label="Quitar mano de obra">×</button>
    </div>
  `;
}

function updateResourceTotals() {
  const form = $('#resources-form');
  let partsTotal = 0;
  let laborTotal = 0;
  $$('.resource-part-line', form).forEach((row) => {
    const quantity = Math.max(0, Number($('.resource-part-quantity', row).value) || 0);
    const price = Math.max(0, Number($('.resource-part-price', row).value) || 0);
    const subtotal = Math.round(quantity * price * 100) / 100;
    partsTotal += subtotal;
    $('.resource-line-total', row).textContent = formatCurrency(subtotal);
  });
  $$('.resource-labor-line', form).forEach((row) => {
    const hours = Math.max(0, Number($('.resource-labor-hours', row).value) || 0);
    const rate = Math.max(0, Number($('.resource-labor-rate', row).value) || 0);
    const subtotal = Math.round(hours * rate * 100) / 100;
    laborTotal += subtotal;
    $('.resource-line-total', row).textContent = formatCurrency(subtotal);
  });
  partsTotal = Math.round(partsTotal * 100) / 100;
  laborTotal = Math.round(laborTotal * 100) / 100;
  $('#resource-parts-total').textContent = formatCurrency(partsTotal);
  $('#resource-labor-total').textContent = formatCurrency(laborTotal);
  $('#resources-total').textContent = formatCurrency(partsTotal + laborTotal);
}

function collectOrderResources() {
  const form = $('#resources-form');
  return {
    parts: $$('.resource-part-line', form).map((row) => ({
      inventoryItemId: $('.resource-part-select', row).value,
      quantity: $('.resource-part-quantity', row).value,
      unitPrice: $('.resource-part-price', row).value
    })),
    labor: $$('.resource-labor-line', form).map((row) => ({
      technician: $('.resource-technician', row).value.trim(),
      description: $('.resource-labor-description', row).value.trim(),
      hours: $('.resource-labor-hours', row).value,
      hourlyRate: $('.resource-labor-rate', row).value
    }))
  };
}

function renderResourcesDrawer(order) {
  const form = $('#resources-form');
  form.reset();
  clearFormErrors(form);
  form.elements.orderId.value = order.id;
  $('#resources-title').textContent = `${order.orderNumber} · Consumos`;
  $('#resources-order-summary').innerHTML = `
    <div><span>Equipo</span><strong>${escapeHtml([order.assetName, order.assetCode].filter(Boolean).join(' · ') || 'No identificado')}</strong></div>
    <div><span>Estado</span><strong>${escapeHtml(STATUS_LABELS[order.status] || order.status)}</strong></div>
    <div><span>Trabajo</span><strong>${escapeHtml(order.title)}</strong></div>
  `;
  $('#order-parts').innerHTML = (order.parts || []).map(orderPartTemplate).join('');
  $('#order-labor').innerHTML = (order.labor || []).map((line) => orderLaborTemplate(line, order)).join('');
  $('#parts-inventory-empty').hidden = state.inventory.length > 0;
  const locked = ['pendiente_autorizacion', 'completada', 'entregada', 'cancelada'].includes(order.status);
  $$('.resource-editor-section input, .resource-editor-section select, .resource-editor-section button', form)
    .forEach((control) => { control.disabled = locked; });
  $('[data-action="add-order-part"]', form).disabled = locked || state.inventory.length === 0;
  $('.resources-save-button', form).hidden = locked;
  updateResourceTotals();
}

function openResourcesDrawer(order) {
  renderResourcesDrawer(order);
  setLayer($('#detail-drawer-layer'), false);
  setLayer($('#resources-drawer-layer'), true);
}

function closeResourcesDrawer(restoreDetail = true) {
  setLayer($('#resources-drawer-layer'), false);
  if (restoreDetail && state.selectedOrder) {
    renderOrderDetail(state.selectedOrder);
    setLayer($('#detail-drawer-layer'), true);
  }
}

function openPaymentDrawer(order) {
  const form = $('#payment-form');
  form.reset();
  clearFormErrors(form);
  form.elements.orderId.value = order.id;
  form.elements.paymentType.value = order.payments?.some((payment) => payment.status === 'active') ? 'payment' : 'advance';
  if (order.financialTotal > 0 && order.balanceDue > 0) {
    form.elements.amount.value = order.balanceDue.toFixed(2);
    form.elements.amount.max = order.balanceDue.toFixed(2);
  } else {
    form.elements.amount.value = '';
    form.elements.amount.removeAttribute('max');
  }
  $('#payment-order-summary').innerHTML = `
    <div><span>Orden</span><strong>${escapeHtml(order.orderNumber)}</strong></div>
    <div><span>Total de referencia</span><strong>${order.financialTotal > 0 ? escapeHtml(formatCurrency(order.financialTotal)) : 'Por definir'}</strong></div>
    <div><span>Pagado</span><strong>${escapeHtml(formatCurrency(order.paymentsTotal))}</strong></div>
    <div><span>Saldo</span><strong>${escapeHtml(formatCurrency(order.balanceDue))}</strong></div>
  `;
  $('#payment-balance-help').textContent = order.financialTotal > 0
    ? `Puedes registrar hasta ${formatCurrency(order.balanceDue)}.`
    : 'El total de la orden aún no está definido. El monto se conservará como saldo a favor.';
  setLayer($('#detail-drawer-layer'), false);
  setLayer($('#payment-drawer-layer'), true);
  setTimeout(() => form.elements.amount.focus(), 220);
}

function closePaymentDrawer(restoreDetail = true) {
  setLayer($('#payment-drawer-layer'), false);
  if (restoreDetail && state.selectedOrder) {
    renderOrderDetail(state.selectedOrder);
    setLayer($('#detail-drawer-layer'), true);
  }
}

function openWarrantyDrawer(order) {
  const form = $('#warranty-form');
  form.reset();
  clearFormErrors(form);
  form.elements.orderId.value = order.id;
  const today = new Date().toISOString().slice(0, 10);
  form.elements.startsAt.value = order.warranty?.startsAt || order.completedAt?.slice(0, 10) || today;
  form.elements.durationDays.value = order.warranty?.durationDays || 90;
  form.elements.coverage.value = order.warranty?.coverage || '';
  form.elements.exclusions.value = order.warranty?.exclusions || '';
  $('#warranty-title').textContent = order.warranty ? 'Editar garantía' : 'Definir garantía';
  $('#warranty-order-summary').innerHTML = `<div><span>Orden</span><strong>${escapeHtml(order.orderNumber)}</strong></div><div><span>Equipo</span><strong>${escapeHtml(order.assetName || 'Sin identificar')}</strong></div><div><span>Cliente</span><strong>${escapeHtml(order.requestedBy || 'No indicado')}</strong></div>`;
  setLayer($('#detail-drawer-layer'), false);
  setLayer($('#warranty-drawer-layer'), true);
  setTimeout(() => form.elements.coverage.focus(), 220);
}

function closeWarrantyDrawer(restoreDetail = true) {
  setLayer($('#warranty-drawer-layer'), false);
  if (restoreDetail && state.selectedOrder) {
    renderOrderDetail(state.selectedOrder);
    setLayer($('#detail-drawer-layer'), true);
  }
}

function openWarrantyReturnDrawer(order) {
  const form = $('#warranty-return-form');
  form.reset();
  clearFormErrors(form);
  form.elements.originalOrderId.value = order.id;
  const warranty = order.warranty;
  $('#warranty-return-summary').innerHTML = `<div><span>Garantía</span><strong>${escapeHtml(warranty.warrantyNumber)}</strong></div><div><span>Orden original</span><strong>${escapeHtml(order.orderNumber)}</strong></div><div><span>Vencimiento</span><strong>${escapeHtml(formatDate(warranty.expiresAt, { year: true }))}</strong></div>`;
  setLayer($('#detail-drawer-layer'), false);
  setLayer($('#warranty-return-drawer-layer'), true);
  setTimeout(() => form.elements.reportedIssue.focus(), 220);
}

function closeWarrantyReturnDrawer(restoreDetail = true) {
  setLayer($('#warranty-return-drawer-layer'), false);
  if (restoreDetail && state.selectedOrder) {
    renderOrderDetail(state.selectedOrder);
    setLayer($('#detail-drawer-layer'), true);
  }
}

async function saveOrderResources() {
  if (!state.selectedOrder) return null;
  const form = $('#resources-form');
  clearFormErrors(form);
  try {
    const order = await api(`/api/orders/${state.selectedOrder.id}/resources`, {
      method: 'PUT',
      body: JSON.stringify(collectOrderResources())
    });
    state.selectedOrder = order;
    await refreshAll();
    renderResourcesDrawer(order);
    showToast('Repuestos, existencias y mano de obra actualizados.');
    return order;
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
    return null;
  }
}

function switchView(view) {
  if (!['dashboard', 'statistics', 'orders', 'customers', 'assets', 'inventory', 'sales', 'backups', 'users'].includes(view)) return;
  if (view === 'users' && state.session?.role !== 'admin') return;
  state.activeView = view;
  $$('.view').forEach((panel) => panel.classList.toggle('is-active', panel.dataset.viewPanel === view));
  $$('.nav-item').forEach((item) => item.classList.toggle('is-active', item.dataset.view === view));
  $$('.mobile-nav-item').forEach((item) => item.classList.toggle('is-active', item.dataset.view === view));
  const headings = {
    statistics: ['Estad\u00edsticas', 'Resultados operativos, ingresos y productividad del taller'],
    users: ['Usuarios y actividad', 'Cuentas locales, perfiles de acceso y auditor\u00eda'],
    dashboard: ['Resumen del taller', 'Control de diagnósticos y reparaciones'],
    orders: ['Órdenes de servicio', 'Recibe, diagnostica y da seguimiento a cada equipo'],
    customers: ['Clientes', 'Directorio e historial de atención'],
    assets: ['Equipos registrados', 'Historial de mantenimiento y reparaciones'],
    inventory: ['Inventario', 'Control de repuestos, consumibles, compras y movimientos'],
    sales: ['Ventas directas', 'Accesorios y productos vendidos sin orden de servicio'],
    backups: ['Respaldos y restauración', 'Copias locales verificadas para proteger la información del taller']
  };
  $('#page-title').textContent = headings[view][0];
  $('#page-subtitle').textContent = headings[view][1];
  $('#sidebar').classList.remove('is-open');
  $('#mobile-overlay').classList.remove('is-open');
  if (view === 'backups') refreshBackupCenter().catch((error) => showToast(error.message, 'error'));
  if (view === 'statistics') refreshStatistics().catch((error) => showToast(error.message, 'error'));
  if (view === 'users') {
    refreshUsers().catch((error) => showToast(error.message, 'error'));
    refreshSyncStatus().catch((error) => showToast(error.message, 'error'));
  }
}

function metricCard(icon, label, value, detail, tone = '') {
  return `
    <article class="metric-card ${tone}">
      <div class="metric-icon">${icon}</div>
      <div class="metric-copy">
        <span>${escapeHtml(label)}</span>
        <strong>${Number(value)}</strong>
        <small>${escapeHtml(detail)}</small>
      </div>
    </article>
  `;
}

function mobileOrderCard(order) {
  const secondary = [order.assetName, order.requestedBy].filter(Boolean).join(' · ') || order.location || 'Sin equipo asignado';
  return `
    <button type="button" class="mobile-record-card mobile-order-card" data-action="open-order" data-order-id="${order.id}">
      <span class="mobile-record-heading"><strong class="order-number">${escapeHtml(order.orderNumber)}</strong>${statusBadge(order.status)}</span>
      <span class="mobile-record-title">${escapeHtml(order.title)}</span>
      <span class="mobile-record-copy">${escapeHtml(secondary)}</span>
      <span class="mobile-record-meta">
        ${priorityBadge(order.priority)}
        <small class="${isOverdue(order) ? 'is-overdue' : ''}">${escapeHtml(formatDate(order.scheduledDate))}</small>
      </span>
    </button>
  `;
}

function renderDashboard() {
  const dashboard = state.dashboard;
  if (!dashboard) return;
  const totals = dashboard.totals;
  $('#metric-grid').innerHTML = [
    metricCard('↗', 'Órdenes activas', totals.activeOrders, `${totals.dueToday} programadas para hoy`, 'metric-green'),
    metricCard('!', 'Servicios urgentes', totals.criticalOpen, totals.criticalOpen ? 'Requieren atención inmediata' : 'Sin urgencias abiertas', 'metric-red'),
    metricCard('◷', 'Órdenes atrasadas', totals.overdue, totals.overdue ? 'Fuera de la fecha programada' : 'Planificación al día', 'metric-amber'),
    metricCard('✓', 'Listas este mes', totals.completedThisMonth, `${totals.totalOrders} órdenes históricas`, 'metric-blue')
  ].join('');

  $('#nav-order-count').textContent = totals.activeOrders;
  $('#workload-total').textContent = totals.activeOrders;
  $('#workload-caption').textContent = totals.activeOrders ? 'en el flujo del taller' : 'Todo bajo control';

  const activeStatuses = ['solicitada', 'planificada', 'pendiente_autorizacion', 'en_progreso', 'en_espera', 'completada'];
  const countByStatus = Object.fromEntries(dashboard.byStatus.map((item) => [item.status, item.count]));
  const max = Math.max(1, ...activeStatuses.map((status) => countByStatus[status] || 0));
  $('#status-breakdown').innerHTML = activeStatuses.map((status) => {
    const count = countByStatus[status] || 0;
    return `
      <div class="breakdown-row">
        <div><span class="breakdown-dot dot-${status}"></span>${escapeHtml(STATUS_LABELS[status])}<b>${count}</b></div>
        <progress value="${count}" max="${max}" aria-label="${escapeHtml(STATUS_LABELS[status])}: ${count}"></progress>
      </div>
    `;
  }).join('');

  $('#attention-title').textContent = totals.overdue ? `${totals.overdue} ${totals.overdue === 1 ? 'orden atrasada' : 'órdenes atrasadas'}` : 'Sin atrasos';
  $('#attention-copy').textContent = totals.overdue ? 'Revisa las fechas o reasigna los trabajos pendientes.' : 'La planificación está al día.';
  $('.attention-box').classList.toggle('has-alert', totals.overdue > 0);

  const body = $('#dashboard-orders');
  body.innerHTML = dashboard.upcoming.map((order) => `
    <tr data-order-id="${order.id}" tabindex="0">
      <td><span class="order-number">${escapeHtml(order.orderNumber)}</span></td>
      <td>
        <div class="primary-cell"><strong>${escapeHtml(order.title)}</strong><span>${escapeHtml(order.assetName || order.location || 'Sin equipo asignado')}</span></div>
      </td>
      <td>${priorityBadge(order.priority)}</td>
      <td><span class="date-value ${isOverdue(order) ? 'is-overdue' : ''}">${escapeHtml(formatDate(order.scheduledDate))}</span></td>
      <td>${statusBadge(order.status)}</td>
    </tr>
  `).join('');
  $('#mobile-dashboard-orders').innerHTML = dashboard.upcoming.map(mobileOrderCard).join('');
  $('#dashboard-empty').hidden = dashboard.upcoming.length > 0;
  $('.compact-table').hidden = dashboard.upcoming.length === 0;
  $('#mobile-dashboard-orders').hidden = dashboard.upcoming.length === 0;
}

function isOverdue(order) {
  const today = new Date();
  const date = `${today.getFullYear()}-${String(today.getMonth() + 1).padStart(2, '0')}-${String(today.getDate()).padStart(2, '0')}`;
  return order.scheduledDate && order.scheduledDate < date && !['completada', 'entregada', 'cancelada'].includes(order.status);
}

function renderOrders() {
  const body = $('#orders-table-body');
  body.innerHTML = state.orders.map((order) => `
    <tr data-order-id="${order.id}" tabindex="0">
      <td><span class="order-number">${escapeHtml(order.orderNumber)}</span></td>
      <td>
        <div class="primary-cell"><strong>${escapeHtml(order.title)}</strong><span>${escapeHtml(order.assetName || order.location || 'Sin equipo asignado')}</span></div>
      </td>
      <td><span class="type-label">${escapeHtml(TYPE_LABELS[order.type] || order.type)}</span></td>
      <td>${priorityBadge(order.priority)}</td>
      <td><span class="date-value ${isOverdue(order) ? 'is-overdue' : ''}">${escapeHtml(formatDate(order.scheduledDate))}</span></td>
      <td><span class="assignee">${escapeHtml(order.assignedTo || 'Sin asignar')}</span></td>
      <td>${statusBadge(order.status)}</td>
      <td><button class="row-action" data-order-id="${order.id}" aria-label="Abrir ${escapeHtml(order.orderNumber)}">›</button></td>
    </tr>
  `).join('');
  $('#mobile-orders-list').innerHTML = state.orders.map(mobileOrderCard).join('');
  $('#order-result-count').textContent = `${state.orders.length} ${state.orders.length === 1 ? 'orden' : 'órdenes'}`;
  $('#orders-empty').hidden = state.orders.length > 0;
  $('#orders-view .data-table').hidden = state.orders.length === 0;
  $('#mobile-orders-list').hidden = state.orders.length === 0;
}

function renderAssets() {
  $('#assets-table-body').innerHTML = state.assets.map((asset) => `
    <tr>
      <td><span class="asset-code">${escapeHtml(asset.code)}</span></td>
      <td>
        <div class="asset-name-cell"><span>${escapeHtml(asset.name.slice(0, 1).toUpperCase())}</span><div><strong>${escapeHtml(asset.name)}</strong><small>${escapeHtml([asset.manufacturer, asset.model].filter(Boolean).join(' · ') || 'Sin datos de fabricante')}</small></div></div>
      </td>
      <td>${escapeHtml(asset.customerName || 'Sin cliente vinculado')}</td>
      <td>${escapeHtml(asset.category || 'Sin categoría')}</td>
      <td>${escapeHtml(asset.location)}</td>
      <td><span class="asset-status asset-${escapeHtml(asset.status)}"><span></span>${escapeHtml(ASSET_STATUS_LABELS[asset.status] || asset.status)}</span></td>
      <td><strong class="open-order-count">${asset.openOrderCount}</strong></td>
    </tr>
  `).join('');
  $('#mobile-assets-list').innerHTML = state.assets.map((asset) => `
    <article class="mobile-record-card">
      <span class="mobile-record-heading"><strong class="asset-code">${escapeHtml(asset.code)}</strong><span class="asset-status asset-${escapeHtml(asset.status)}"><span></span>${escapeHtml(ASSET_STATUS_LABELS[asset.status] || asset.status)}</span></span>
      <span class="mobile-record-title">${escapeHtml(asset.name)}</span>
      <span class="mobile-record-copy">${escapeHtml([asset.manufacturer, asset.model].filter(Boolean).join(' · ') || asset.category || 'Sin datos de fabricante')}</span>
      <span class="mobile-record-meta"><small>${escapeHtml(asset.customerName || 'Sin cliente vinculado')}</small><small>${asset.openOrderCount} abiertas</small></span>
    </article>
  `).join('');
  $('#asset-result-count').textContent = `${state.assets.length} ${state.assets.length === 1 ? 'equipo' : 'equipos'}`;
  $('#assets-empty').hidden = state.assets.length > 0;
  $('#assets-view .data-table').hidden = state.assets.length === 0;
  $('#mobile-assets-list').hidden = state.assets.length === 0;

  $('#order-asset-select').innerHTML = '<option value="">Seleccionar equipo…</option>' + state.assets.map((asset) =>
    `<option value="${asset.id}">${escapeHtml(asset.code)} · ${escapeHtml(asset.name)}</option>`
  ).join('');
}

function renderCustomers() {
  const query = $('#customer-search').value.trim().toLocaleLowerCase('es');
  const customers = state.customers.filter((customer) => [
    customer.name, customer.phone, customer.email, customer.company, customer.taxId
  ].some((value) => String(value || '').toLocaleLowerCase('es').includes(query)));
  $('#customer-total-count').textContent = state.customers.length;
  $('#customer-active-count').textContent = state.customers.filter((customer) => customer.activeOrderCount > 0).length;
  $('#customer-equipment-count').textContent = state.customers.reduce((total, customer) => total + customer.equipmentCount, 0);
  $('#customers-table-body').innerHTML = customers.map((customer) => `
    <tr>
      <td><div class="customer-name-cell"><span>${escapeHtml(customer.name.slice(0, 1).toUpperCase())}</span><div><strong>${escapeHtml(customer.name)}</strong><small>${escapeHtml(customer.address || 'Sin dirección registrada')}</small></div></div></td>
      <td><div class="primary-cell"><strong>${escapeHtml(customer.phone || 'Sin teléfono')}</strong><span>${escapeHtml(customer.email || 'Sin correo')}</span></div></td>
      <td><div class="primary-cell"><strong>${escapeHtml(customer.company || 'Cliente particular')}</strong><span>${escapeHtml(customer.taxId ? `NIT ${customer.taxId}` : 'Sin NIT')}</span></div></td>
      <td>${customer.equipmentCount}</td>
      <td><strong>${customer.orderCount}</strong>${customer.activeOrderCount ? `<span class="customer-active-orders">${customer.activeOrderCount} activas</span>` : ''}</td>
      <td>${escapeHtml(customer.lastServiceAt ? formatDate(customer.lastServiceAt, { year: true }) : 'Sin servicios')}</td>
      <td><div class="customer-row-actions"><button class="row-action customer-orders-button" type="button" data-action="customer-orders" data-customer-id="${customer.id}">Órdenes</button><button class="row-action customer-edit-button" type="button" data-action="edit-customer" data-customer-id="${customer.id}">Editar</button></div></td>
    </tr>
  `).join('') || (state.customers.length ? '<tr><td colspan="7"><div class="table-empty-message">No hay clientes que coincidan con la búsqueda.</div></td></tr>' : '');
  $('#mobile-customers-list').innerHTML = customers.map((customer) => `
    <article class="mobile-record-card mobile-customer-card">
      <span class="mobile-record-heading"><strong>${escapeHtml(customer.name)}</strong>${customer.activeOrderCount ? `<span class="customer-active-orders">${customer.activeOrderCount} activas</span>` : ''}</span>
      <span class="mobile-record-copy">${escapeHtml(customer.phone || 'Sin teléfono')}${customer.company ? ` · ${escapeHtml(customer.company)}` : ''}</span>
      <span class="mobile-record-meta"><small>${customer.equipmentCount} equipos</small><small>${customer.orderCount} órdenes</small></span>
      <span class="mobile-record-actions">
        <button class="secondary-button compact-button" type="button" data-action="customer-orders" data-customer-id="${customer.id}">Órdenes</button>
        <button class="ghost-button compact-button" type="button" data-action="edit-customer" data-customer-id="${customer.id}">Editar</button>
      </span>
    </article>
  `).join('');
  $('#customer-result-count').textContent = `${customers.length} ${customers.length === 1 ? 'cliente' : 'clientes'}`;
  $('#customers-empty').hidden = state.customers.length > 0;
  $('#customers-view .data-table').hidden = state.customers.length === 0;
  $('#mobile-customers-list').hidden = state.customers.length === 0;
  $('#order-customer-select').innerHTML = '<option value="">Seleccionar cliente…</option>' + state.customers.map((customer) =>
    `<option value="${customer.id}">${escapeHtml(customer.name)}${customer.phone ? ` · ${escapeHtml(customer.phone)}` : ''}</option>`
  ).join('');
}

function inventoryLevel(item) {
  if (item.quantity === 0) return { key: 'out', label: 'Agotado' };
  if (item.minimumStock > 0 && item.quantity <= item.minimumStock) {
    return { key: 'low', label: 'Compra requerida' };
  }
  return { key: 'available', label: 'Disponible' };
}

function renderInventory() {
  const query = $('#inventory-search').value.trim().toLocaleLowerCase('es');
  const items = state.inventory.filter((item) => [item.code, item.name, item.category, item.unit]
    .some((value) => String(value || '').toLocaleLowerCase('es').includes(query)));
  const totalUnits = state.inventory.reduce((total, item) => total + item.quantity, 0);
  const lowCount = state.inventory.filter((item) => item.quantity === 0
    || (item.minimumStock > 0 && item.quantity <= item.minimumStock)).length;

  $('#inventory-reference-count').textContent = state.inventory.length;
  $('#inventory-unit-count').textContent = totalUnits;
  $('#inventory-low-count').textContent = lowCount;
  $('#inventory-table-body').innerHTML = items.map((item) => {
    const level = inventoryLevel(item);
    return `
      <tr>
        <td><span class="asset-code">${escapeHtml(item.code)}</span></td>
        <td><div class="primary-cell"><strong>${escapeHtml(item.name)}</strong><span>${item.orderUsageCount ? `Usado en ${item.orderUsageCount} ${item.orderUsageCount === 1 ? 'orden' : 'órdenes'} · ${item.consumedQuantity} ${escapeHtml(item.unit)}` : `Actualizado ${escapeHtml(formatDate(item.updatedAt, { year: true }))}`}</span></div></td>
        <td>${escapeHtml(item.category || 'Sin categoría')}</td>
        <td>${escapeHtml(item.unit)}</td>
        <td><strong class="inventory-quantity">${item.quantity}</strong></td>
        <td>${item.minimumStock}</td>
        <td><span class="inventory-status inventory-${level.key}"><span></span>${escapeHtml(level.label)}</span></td>
        <td><button class="row-action inventory-edit-button" type="button" data-action="edit-inventory-item" data-inventory-id="${item.id}" aria-label="Editar ${escapeHtml(item.name)}">Editar</button></td>
      </tr>
    `;
  }).join('') || (state.inventory.length ? '<tr><td colspan="8"><div class="table-empty-message">No hay productos que coincidan con la búsqueda.</div></td></tr>' : '');
  $('#inventory-empty').hidden = state.inventory.length > 0;
  $('#inventory-view .data-table').hidden = state.inventory.length === 0;
}

function purchaseLineTemplate(item) {
  const level = inventoryLevel(item);
  const selected = level.key !== 'available';
  const suggestedQuantity = Math.max(item.minimumStock - item.quantity, 1);
  return `
    <div class="purchase-line ${selected ? 'is-selected' : ''}" data-purchase-item-id="${item.id}">
      <label class="purchase-line-product">
        <input class="purchase-select" type="checkbox" ${selected ? 'checked' : ''}>
        <span>
          <strong>${escapeHtml(item.name)}</strong>
          <small>${escapeHtml(item.code)} · Hay ${item.quantity} ${escapeHtml(item.unit)}${item.minimumStock > 0 ? ` · Mínimo ${item.minimumStock}` : ''}</small>
        </span>
      </label>
      <label class="purchase-line-field">
        <span class="sr-only">Cantidad para ${escapeHtml(item.name)}</span>
        <input class="purchase-quantity" type="number" min="1" step="1" value="${suggestedQuantity}" ${selected ? '' : 'disabled'}>
      </label>
      <label class="purchase-line-field cost-field">
        <span aria-hidden="true">Q</span>
        <input class="purchase-cost" type="number" min="0.01" step="0.01" inputmode="decimal" placeholder="0.00" ${selected ? '' : 'disabled'}>
      </label>
      <strong class="purchase-line-total">Q0.00</strong>
    </div>
  `;
}

function updatePurchaseReportTotal() {
  let total = 0;
  $$('.purchase-line', $('#purchase-report-form')).forEach((row) => {
    const selected = $('.purchase-select', row).checked;
    row.classList.toggle('is-selected', selected);
    const quantity = Number($('.purchase-quantity', row).value) || 0;
    const cost = Number($('.purchase-cost', row).value) || 0;
    const subtotal = selected ? quantity * cost : 0;
    $('.purchase-line-total', row).textContent = formatCurrency(subtotal);
    total += subtotal;
  });
  $('#purchase-report-total').textContent = formatCurrency(total);
}

function openPurchaseReport() {
  if (state.inventory.length === 0) {
    showToast('Agrega al menos un producto antes de generar el reporte.', 'error');
    openInventoryDrawer();
    return;
  }
  const form = $('#purchase-report-form');
  form.reset();
  clearFormErrors(form);
  $('#purchase-report-lines').innerHTML = state.inventory.map(purchaseLineTemplate).join('');
  updatePurchaseReportTotal();
  setLayer($('#purchase-report-drawer-layer'), true);
}

function renderPurchaseReports() {
  $('#purchase-reports-table-body').innerHTML = state.purchaseReports.map((report) => `
    <tr>
      <td><span class="order-number">${escapeHtml(report.reportNumber)}</span></td>
      <td>${escapeHtml(formatDate(report.createdAt, { year: true }))}</td>
      <td>${escapeHtml(report.supplier || 'Por definir')}</td>
      <td><div class="primary-cell"><strong>${report.receivedQuantity} / ${report.orderedQuantity}</strong><span>${report.status === 'received' ? 'Recibida' : report.status === 'partial' ? 'Recepción parcial' : 'Pendiente'}</span></div></td>
      <td><strong>${escapeHtml(formatCurrency(report.totalCost))}</strong></td>
      <td><div class="table-row-actions">
        ${report.status !== 'received' ? `<button class="row-action" type="button" data-action="receive-purchase" data-report-id="${report.id}">Recibir</button>` : ''}
        <button class="row-action report-print-button" type="button" data-action="print-purchase-report" data-report-id="${report.id}">Imprimir</button>
      </div></td>
    </tr>
  `).join('');
  $('#purchase-report-count').textContent = `${state.purchaseReports.length} ${state.purchaseReports.length === 1 ? 'reporte' : 'reportes'}`;
  $('#purchase-reports-empty').hidden = state.purchaseReports.length > 0;
  $('.purchase-history-panel .data-table').hidden = state.purchaseReports.length === 0;
}

const MOVEMENT_LABELS = {
  opening_balance: 'Saldo inicial',
  initial_stock: 'Alta inicial',
  manual_adjustment: 'Ajuste manual',
  purchase_receipt: 'Recepción de compra',
  work_order_adjustment: 'Consumo de orden',
  sale: 'Venta directa',
  sale_cancelled: 'Anulación de venta'
};

function renderInventoryMovements() {
  $('#inventory-movements-table-body').innerHTML = state.inventoryMovements.map((movement) => {
    const direction = movement.quantityDelta > 0 ? 'positive' : movement.quantityDelta < 0 ? 'negative' : 'neutral';
    const quantity = `${movement.quantityDelta > 0 ? '+' : ''}${movement.quantityDelta}`;
    return `
      <tr>
        <td>${escapeHtml(formatDateTime(movement.createdAt))}</td>
        <td><div class="primary-cell"><strong>${escapeHtml(movement.itemName)}</strong><span>${escapeHtml(movement.itemCode)} · ${escapeHtml(movement.unit)}</span></div></td>
        <td>${escapeHtml(MOVEMENT_LABELS[movement.movementType] || movement.movementType)}</td>
        <td><strong class="movement-quantity movement-${direction}">${escapeHtml(quantity)}</strong></td>
        <td>${movement.balanceAfter}</td>
        <td><div class="primary-cell"><strong>${escapeHtml(movement.referenceNumber || 'Manual')}</strong><span>${escapeHtml(movement.note || '')}</span></div></td>
      </tr>
    `;
  }).join('');
  $('#inventory-movement-count').textContent = `${state.inventoryMovements.length} ${state.inventoryMovements.length === 1 ? 'movimiento' : 'movimientos'}`;
  $('#inventory-movements-empty').hidden = state.inventoryMovements.length > 0;
  $('.movement-history-panel .data-table').hidden = state.inventoryMovements.length === 0;
}

function purchaseReceiptLineTemplate(line) {
  return `
    <div class="purchase-line is-selected" data-report-line-id="${line.id}" data-unit-cost="${line.unitCost}">
      <label class="purchase-line-product">
        <input class="receipt-select" type="checkbox" checked>
        <span><strong>${escapeHtml(line.itemName)}</strong><small>${escapeHtml(line.itemCode)} · Pendiente ${line.remainingQuantity} ${escapeHtml(line.unit)}</small></span>
      </label>
      <label class="purchase-line-field">
        <span class="sr-only">Cantidad recibida de ${escapeHtml(line.itemName)}</span>
        <input class="receipt-quantity" type="number" min="1" max="${line.remainingQuantity}" step="1" value="${line.remainingQuantity}">
      </label>
      <span class="receipt-unit-cost">${escapeHtml(formatCurrency(line.unitCost))}</span>
      <strong class="purchase-line-total">${escapeHtml(formatCurrency(line.remainingQuantity * line.unitCost))}</strong>
    </div>
  `;
}

function updatePurchaseReceiptTotal() {
  let total = 0;
  $$('.purchase-line', $('#purchase-receipt-form')).forEach((row) => {
    const selected = $('.receipt-select', row).checked;
    const quantityInput = $('.receipt-quantity', row);
    quantityInput.disabled = !selected;
    row.classList.toggle('is-selected', selected);
    const subtotal = selected ? (Number(quantityInput.value) || 0) * Number(row.dataset.unitCost) : 0;
    $('.purchase-line-total', row).textContent = formatCurrency(subtotal);
    total += subtotal;
  });
  $('#purchase-receipt-total').textContent = formatCurrency(total);
}

function openPurchaseReceipt(report) {
  const pendingLines = report.lines.filter((line) => line.remainingQuantity > 0 && line.inventoryItemId);
  if (!pendingLines.length) {
    showToast('Este reporte no tiene productos pendientes de recepción.', 'error');
    return;
  }
  const form = $('#purchase-receipt-form');
  form.reset();
  clearFormErrors(form);
  form.elements.reportId.value = report.id;
  $('#purchase-receipt-summary').innerHTML = `<div><span>Reporte</span><strong>${escapeHtml(report.reportNumber)}</strong></div><div><span>Proveedor</span><strong>${escapeHtml(report.supplier || 'Por definir')}</strong></div><div><span>Avance</span><strong>${report.receivedQuantity} de ${report.orderedQuantity}</strong></div>`;
  $('#purchase-receipt-lines').innerHTML = pendingLines.map(purchaseReceiptLineTemplate).join('');
  updatePurchaseReceiptTotal();
  setLayer($('#purchase-receipt-drawer-layer'), true);
}

function closePurchaseReceipt() {
  setLayer($('#purchase-receipt-drawer-layer'), false);
}

function saleLineTemplate(line = {}) {
  const available = state.inventory.filter((item) => item.quantity > 0);
  return `
    <div class="resource-line resource-part-line sale-line">
      <select class="sale-item-select" aria-label="Producto">
        <option value="">Seleccionar producto…</option>
        ${available.map((item) => `<option value="${item.id}" data-stock="${item.quantity}" ${Number(line.inventoryItemId) === item.id ? 'selected' : ''}>${escapeHtml(item.code)} · ${escapeHtml(item.name)} (${item.quantity})</option>`).join('')}
      </select>
      <input class="sale-quantity" type="number" min="1" step="1" value="${line.quantity || 1}" aria-label="Cantidad">
      <input class="sale-unit-price" type="number" min="0" step="0.01" value="${line.unitPrice || 0}" aria-label="Precio unitario">
      <strong class="sale-line-total">Q0.00</strong>
      <button type="button" class="icon-button resource-remove-button" data-action="remove-sale-line" aria-label="Quitar producto">×</button>
      <small class="sale-stock-help"></small>
    </div>
  `;
}

function updateSaleTotals() {
  let subtotal = 0;
  $$('.sale-line', $('#sale-form')).forEach((row) => {
    const quantity = Number($('.sale-quantity', row).value) || 0;
    const unitPrice = Number($('.sale-unit-price', row).value) || 0;
    const selected = $('.sale-item-select', row).selectedOptions[0];
    const stock = Number(selected?.dataset.stock) || 0;
    $('.sale-stock-help', row).textContent = selected?.value ? `Disponible: ${stock}` : '';
    $('.sale-quantity', row).max = stock || '';
    const lineTotal = quantity * unitPrice;
    $('.sale-line-total', row).textContent = formatCurrency(lineTotal);
    subtotal += lineTotal;
  });
  const discount = Number($('#sale-form').elements.discount.value) || 0;
  $('#sale-subtotal').textContent = formatCurrency(subtotal);
  $('#sale-discount').textContent = `− ${formatCurrency(discount)}`;
  $('#sale-total').textContent = formatCurrency(Math.max(subtotal - discount, 0));
}

function openSaleDrawer() {
  if (!state.inventory.some((item) => item.quantity > 0)) {
    showToast('No hay productos con existencias disponibles para vender.', 'error');
    switchView('inventory');
    return;
  }
  const form = $('#sale-form');
  form.reset();
  clearFormErrors(form);
  $('#sale-lines').innerHTML = saleLineTemplate();
  updateSaleTotals();
  setLayer($('#sale-drawer-layer'), true);
}

function closeSaleDrawer() {
  setLayer($('#sale-drawer-layer'), false);
}

function renderSales() {
  const activeSales = state.sales.filter((sale) => sale.status === 'completed');
  const activeTotal = activeSales.reduce((total, sale) => total + sale.total, 0);
  $('#sales-count').textContent = state.sales.length;
  $('#active-sales-count').textContent = activeSales.length;
  $('#sales-total').textContent = formatCurrency(activeTotal);
  $('#sales-table-body').innerHTML = state.sales.map((sale) => `
    <tr>
      <td><span class="order-number">${escapeHtml(sale.saleNumber)}</span></td>
      <td>${escapeHtml(formatDateTime(sale.createdAt))}</td>
      <td>${escapeHtml(sale.customerName || 'Consumidor final')}</td>
      <td>${sale.lines.length}</td>
      <td><strong>${escapeHtml(formatCurrency(sale.total))}</strong></td>
      <td><span class="sale-status sale-${sale.status}">${sale.status === 'cancelled' ? 'Anulada' : 'Completada'}</span></td>
      <td><div class="table-row-actions"><button class="row-action" data-action="print-sale" data-sale-id="${sale.id}">Imprimir</button>${sale.status === 'completed' ? `<button class="row-action danger-action" data-action="cancel-sale" data-sale-id="${sale.id}">Anular</button>` : ''}</div></td>
    </tr>
  `).join('');
  $('#sales-result-count').textContent = `${state.sales.length} ${state.sales.length === 1 ? 'venta' : 'ventas'}`;
  $('#sales-empty').hidden = state.sales.length > 0;
  $('#sales-view .data-table').hidden = state.sales.length === 0;
}

function formatBackupDate(epochMs) {
  if (!epochMs) return 'Fecha no disponible';
  return new Intl.DateTimeFormat('es-GT', {
    day: '2-digit', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit'
  }).format(new Date(epochMs)).replace('.', '');
}

function formatFileSize(bytes) {
  const value = Number(bytes) || 0;
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / (1024 * 1024)).toFixed(1)} MB`;
}

function backupKindLabel(kind) {
  if (kind === 'automatic') return 'Automático';
  if (kind === 'manual') return 'Manual';
  return 'Seguridad previa';
}

function renderBackupCenter() {
  const overview = state.backupOverview;
  if (!overview) return;
  const backups = overview.backups || [];
  const validBackups = backups.filter((backup) => backup.valid);
  const latest = validBackups[0];
  $('#backup-automatic-status').textContent = overview.automaticEnabled ? 'Activo' : 'Inactivo';
  $('#backup-policy').textContent = overview.automaticEnabled
    ? `Una copia diaria · se conservan ${overview.automaticRetention}`
    : 'Sin programación';
  $('#backup-count').textContent = validBackups.length;
  $('#backup-last-date').textContent = latest ? formatBackupDate(latest.modifiedAtEpochMs) : 'Sin copias';
  $('#backup-last-kind').textContent = latest ? backupKindLabel(latest.kind) : '—';
  $('#backup-directory').textContent = overview.directory;
  $('#backup-result-count').textContent = `${backups.length} ${backups.length === 1 ? 'respaldo' : 'respaldos'}`;
  $('#backup-table-body').innerHTML = backups.map((backup) => `
    <tr class="${backup.valid ? '' : 'backup-invalid-row'}">
      <td><div class="backup-date-cell"><strong>${escapeHtml(formatBackupDate(backup.modifiedAtEpochMs))}</strong><small>${escapeHtml(backup.fileName)}</small></div></td>
      <td><span class="backup-kind backup-${escapeHtml(backup.kind)}">${escapeHtml(backupKindLabel(backup.kind))}</span></td>
      <td><div class="backup-content-cell"><span>${backup.workOrderCount} órdenes</span><span>${backup.customerCount} clientes</span><span>${backup.inventoryItemCount} productos</span></div></td>
      <td>${escapeHtml(formatFileSize(backup.sizeBytes))}</td>
      <td><span class="backup-validity ${backup.valid ? 'is-valid' : 'is-invalid'}"><i></i>${backup.valid ? 'Verificado' : 'No válido'}</span></td>
      <td><button type="button" class="secondary-button compact-button" data-action="restore-backup" data-backup-file="${escapeHtml(backup.fileName)}" ${backup.valid ? '' : 'disabled'}>Restaurar</button></td>
    </tr>
  `).join('');
  $('#backups-empty').hidden = backups.length > 0;
  $('#backups-view .backup-table').hidden = backups.length === 0;
  applyPermissions($('#backups-view'));
}

async function refreshBackupCenter(ensureAutomatic = false) {
  state.backupOverview = await api(ensureAutomatic ? '/api/backups/automatic' : '/api/backups', {
    method: ensureAutomatic ? 'POST' : 'GET'
  });
  renderBackupCenter();
  return state.backupOverview;
}

function applyPermissions(root = document) {
  const role = state.session?.role;
  const technicalActions = ['open-resources', 'edit-warranty', 'save-warranty-decision', 'upload-photo', 'delete-photo'];
  const receptionActions = ['new-order', 'new-customer', 'edit-customer', 'new-inventory-item', 'edit-inventory-item', 'new-purchase-report', 'receive-purchase', 'new-sale', 'cancel-sale', 'open-payment', 'cancel-payment', 'new-warranty-return'];
  [...technicalActions, ...receptionActions, 'approve-quote', 'reject-quote', 'revise-quote'].forEach((action) => {
    $$(`[data-action="${action}"]`, root).forEach((element) => { element.hidden = false; });
  });
  $$('[data-admin-only]', root).forEach((element) => { element.hidden = role !== 'admin'; });
  if (role === 'reception') {
    technicalActions.forEach((action) => $$(`[data-action="${action}"]`, root).forEach((element) => { element.hidden = true; }));
    $$('[data-action="request-quote-authorization"], [data-action="revise-quote"], [data-action="add-quote-item"], [data-action="remove-quote-item"], .quote-save-button', root).forEach((element) => { element.hidden = true; });
    $$('.quote-diagnosis-section textarea, .quote-items-section input, .quote-items-section select, .quote-discount-field input', root).forEach((control) => { control.disabled = true; });
    $$('input[data-task-id]', root).forEach((control) => { control.disabled = true; });
  }
  if (role === 'technician') {
    receptionActions.forEach((action) => $$(`[data-action="${action}"]`, root).forEach((element) => { element.hidden = true; }));
    $$('[data-action="approve-quote"], [data-action="reject-quote"], .quote-authorization-section .field', root).forEach((element) => { element.hidden = true; });
  }
  if (role !== 'admin') {
    $$('[data-action="restore-backup"]', root).forEach((element) => { element.hidden = true; });
  }
}

function showAuthentication(requiresSetup = false) {
  stopSynchronization();
  state.session = null;
  $('.app-shell').hidden = true;
  $('#auth-screen').hidden = false;
  $('#setup-panel').hidden = !requiresSetup;
  $('#login-panel').hidden = requiresSetup;
  requestAnimationFrame(() => $(requiresSetup ? '#setup-form input' : '#login-form input')?.focus());
}

async function beginSession(user) {
  state.session = user;
  $('#auth-screen').hidden = true;
  $('.app-shell').hidden = false;
  $('#session-user-name').textContent = user.displayName;
  $('#session-user-role').textContent = ROLE_LABELS[user.role] || user.role;
  $('#session-avatar').textContent = (user.displayName || user.username || 'G').charAt(0).toUpperCase();
  applyPermissions();
  await refreshAll();
  await refreshSyncStatus();
  startSynchronization();
  refreshBackupCenter(true).catch((error) => console.error('Respaldo automatico:', error));
}

async function bootstrapApplication() {
  try {
    try {
      state.runtime = await api('/api/runtime');
    } catch (error) {
      console.warn('No fue posible consultar el entorno de ejecución:', error);
    }
    document.documentElement.dataset.platform = state.runtime.platform;
    document.documentElement.classList.toggle('is-mobile-app', Boolean(state.runtime.mobile));
    const runtimeCopy = $('#mobile-runtime-copy');
    if (runtimeCopy) runtimeCopy.textContent = `Datos locales · versión ${state.runtime.appVersion || '0.1.0'}`;
    const authStatus = await api('/api/auth/status');
    if (authStatus.currentUser) await beginSession(authStatus.currentUser);
    else showAuthentication(authStatus.requiresSetup);
  } catch (error) {
    showAuthentication(false);
    $('#login-error').textContent = error.message;
  }
}

async function refreshStatistics() {
  const range = Number($('#statistics-range').value || 90);
  state.statistics = await api(`/api/statistics?range=${range}`);
  renderStatistics();
}

function renderStatistics() {
  const data = state.statistics;
  if (!data) return;
  const totalRevenue = data.serviceRevenue + data.salesRevenue;
  const warrantyRate = data.warrantyReturns ? Math.round((data.warrantyCovered / data.warrantyReturns) * 100) : 0;
  $('#statistics-metrics').innerHTML = `
    <article><span>Órdenes recibidas</span><strong>${data.ordersReceived}</strong><small>${data.ordersDelivered} entregadas</small></article>
    <article><span>Ingresos registrados</span><strong>${escapeHtml(formatCurrency(totalRevenue))}</strong><small>Servicios y ventas directas</small></article>
    <article><span>Tiempo promedio</span><strong>${Number(data.averageTurnaroundDays).toFixed(1)} días</strong><small>Ingreso a finalización</small></article>
    <article><span>Regresos por garantía</span><strong>${data.warrantyReturns}</strong><small>${warrantyRate}% cubiertos</small></article>
  `;
  const maxRevenue = Math.max(1, ...data.monthly.map((month) => month.serviceRevenue + month.salesRevenue));
  $('#monthly-chart').innerHTML = data.monthly.map((month) => {
    const revenue = month.serviceRevenue + month.salesRevenue;
    const label = new Intl.DateTimeFormat('es-GT', { month: 'short' }).format(new Date(`${month.month}-02T12:00:00`)).replace('.', '');
    return `<div class="month-column" title="${escapeHtml(formatCurrency(revenue))}"><div class="month-bar-track"><span style="height:${Math.max(4, (revenue / maxRevenue) * 100)}%"></span></div><strong>${escapeHtml(label)}</strong><small>${month.orderCount} ord.</small></div>`;
  }).join('');
  const maximumStatus = Math.max(1, ...data.byStatus.map((item) => item.count));
  $('#statistics-statuses').innerHTML = data.byStatus.length ? data.byStatus.map((item) => `
    <div><span>${escapeHtml(STATUS_LABELS[item.status] || item.status)} <strong>${item.count}</strong></span><progress value="${item.count}" max="${maximumStatus}"></progress></div>
  `).join('') : '<p class="empty-copy">No hay órdenes en el periodo.</p>';
  $('#technician-statistics').innerHTML = data.technicians.map((item) => `<tr><td><strong>${escapeHtml(item.technician)}</strong></td><td>${item.jobs}</td><td>${Number(item.hours).toFixed(1)} h</td><td>${escapeHtml(formatCurrency(item.laborTotal))}</td></tr>`).join('');
  $('#technician-statistics-empty').hidden = data.technicians.length > 0;
}

async function refreshUsers() {
  if (state.session?.role !== 'admin') return;
  const [users, activity] = await Promise.all([
    api('/api/users'),
    api('/api/user-activity?limit=100')
  ]);
  state.users = users;
  state.userActivity = activity;
  renderUsers();
}

function renderUsers() {
  $('#users-count').textContent = `${state.users.length} usuario${state.users.length === 1 ? '' : 's'}`;
  $('#users-table-body').innerHTML = state.users.map((user) => `
    <tr class="${user.active ? '' : 'is-muted'}" data-user-id="${user.id}">
      <td><strong>${escapeHtml(user.displayName)}</strong><small>@${escapeHtml(user.username)}</small></td>
      <td>${escapeHtml(ROLE_LABELS[user.role] || user.role)}</td>
      <td>${user.lastLoginAt ? escapeHtml(formatDateTime(user.lastLoginAt)) : 'Nunca'}</td>
      <td><span class="account-status ${user.active ? 'is-active' : ''}">${user.active ? 'Activa' : 'Inactiva'}</span></td>
      <td><div class="user-row-actions"><button class="row-action" data-action="reset-user-password" data-user-id="${user.id}">Cambiar clave</button><button class="row-action ${user.active ? 'danger-action' : ''}" data-action="toggle-user" data-user-id="${user.id}" ${user.id === state.session.id ? 'disabled' : ''}>${user.active ? 'Desactivar' : 'Activar'}</button></div></td>
    </tr>
  `).join('');
  $('#user-activity-list').innerHTML = state.userActivity.length ? state.userActivity.map((item) => `
    <li><span class="activity-avatar">${escapeHtml(item.userDisplayName.charAt(0).toUpperCase())}</span><div><strong>${escapeHtml(item.userDisplayName)}</strong><p>${escapeHtml(item.summary)}</p><small>${escapeHtml(formatDateTime(item.createdAt))} · ${escapeHtml(ROLE_LABELS[item.userRole] || item.userRole)}</small></div></li>
  `).join('') : '<li class="empty-copy">No hay actividad registrada.</li>';
}

function openUserDrawer() {
  const form = $('#user-form');
  form.reset();
  clearFormErrors(form);
  setLayer($('#user-drawer-layer'), true);
  requestAnimationFrame(() => form.elements.displayName.focus());
}

function closeUserDrawer() {
  setLayer($('#user-drawer-layer'), false);
}

function releasePhotoUrls() {
  state.photoUrls.forEach((url) => URL.revokeObjectURL(url));
  state.photoUrls = [];
}

async function loadPhotoBytes(file) {
  if (!file.type.startsWith('image/')) throw new Error('Selecciona una imagen JPEG, PNG o WebP.');
  if (file.size > 20 * 1024 * 1024) throw new Error('La imagen original no puede superar 20 MB.');
  const bitmap = await createImageBitmap(file);
  const scale = Math.min(1, 1800 / Math.max(bitmap.width, bitmap.height));
  const canvas = document.createElement('canvas');
  canvas.width = Math.max(1, Math.round(bitmap.width * scale));
  canvas.height = Math.max(1, Math.round(bitmap.height * scale));
  canvas.getContext('2d').drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  bitmap.close();
  const mimeType = file.type === 'image/png' && file.size < 3 * 1024 * 1024 ? 'image/png' : 'image/jpeg';
  const blob = await new Promise((resolve) => canvas.toBlob(resolve, mimeType, 0.82));
  if (!blob) throw new Error('No fue posible procesar la fotografía.');
  return { mimeType, data: [...new Uint8Array(await blob.arrayBuffer())] };
}

async function hydrateOrderMedia(order) {
  if (state.selectedOrder?.id !== order.id) return;
  try {
    let qr = state.qrCache.get(order.id);
    if (!qr) {
      qr = await api(`/api/orders/${order.id}/qr`);
      state.qrCache.set(order.id, qr);
    }
    if (state.selectedOrder?.id === order.id && $('#order-qr-preview')) $('#order-qr-preview').innerHTML = qr.svg;
  } catch (error) {
    console.error('QR:', error);
  }
  await Promise.all((order.photos || []).map(async (photo) => {
    try {
      const data = await api(`/api/photos/${photo.id}/data`);
      if (state.selectedOrder?.id !== order.id) return;
      const url = URL.createObjectURL(new Blob([new Uint8Array(data)], { type: photo.mimeType }));
      state.photoUrls.push(url);
      const image = $(`[data-photo-image="${photo.id}"]`);
      if (image) { image.src = url; image.closest('a').href = url; }
    } catch (error) {
      console.error('Fotografía:', error);
    }
  }));
}

function renderOrderDetail(order) {
  releasePhotoUrls();
  const progress = order.taskCount ? Math.round((order.completedTaskCount / order.taskCount) * 100) : 0;
  const accessories = order.accessories || [];
  const equipmentModel = [order.assetManufacturer, order.assetModel].filter(Boolean).join(' · ');
  const isCoveredWarranty = order.warrantyClaim?.decision === 'covered';
  const eventRows = order.events.slice(0, 8).map((event) => `
    <li><span></span><div><strong>${escapeHtml(eventLabel(event))}</strong><small>${escapeHtml(formatDateTime(event.createdAt))}</small></div></li>
  `).join('');
  const paymentRows = (order.payments || []).map((payment) => `
    <li class="payment-history-item ${payment.status === 'cancelled' ? 'is-cancelled' : ''}">
      <div>
        <strong>${escapeHtml(payment.receiptNumber)} · ${escapeHtml(PAYMENT_TYPE_LABELS[payment.paymentType] || payment.paymentType)}</strong>
        <small>${escapeHtml(formatDateTime(payment.createdAt))} · ${escapeHtml(PAYMENT_METHOD_LABELS[payment.method] || payment.method)}${payment.reference ? ` · Ref. ${escapeHtml(payment.reference)}` : ''}</small>
        ${payment.status === 'cancelled' ? `<em>Anulado: ${escapeHtml(payment.cancellationNote || 'Sin detalle')}</em>` : ''}
      </div>
      <strong>${escapeHtml(formatCurrency(payment.amount))}</strong>
      <div class="payment-row-actions">
        <button class="row-action" type="button" data-action="print-payment" data-payment-id="${payment.id}">Recibo</button>
        ${payment.status === 'active' ? `<button class="row-action danger-action" type="button" data-action="cancel-payment" data-payment-id="${payment.id}">Anular</button>` : ''}
      </div>
    </li>
  `).join('');
  $('#detail-content').innerHTML = `
    <div class="drawer-header detail-header">
      <div>
        <span class="order-number">${escapeHtml(order.orderNumber)}</span>
        <h2 id="detail-title">${escapeHtml(order.title)}</h2>
        <div class="detail-badges">${priorityBadge(order.priority)}${statusBadge(order.status)}</div>
      </div>
      <div class="detail-header-actions">
        <button class="secondary-button compact-button" data-action="edit-order">Editar orden</button>
        <button class="icon-button" data-action="close-detail" aria-label="Cerrar">×</button>
      </div>
    </div>
    <div class="detail-body">
      <section class="detail-section description-section">
        <h3>Descripción del trabajo</h3>
        <p>${escapeHtml(order.description)}</p>
      </section>

      <section class="detail-grid">
        <div class="detail-fact"><span>Equipo recibido</span><strong>${escapeHtml(order.assetName || 'Sin identificar')}</strong><small>${escapeHtml(order.assetCode || '')}</small></div>
        <div class="detail-fact"><span>Tipo de equipo</span><strong>${escapeHtml(order.assetCategory || 'No indicado')}</strong></div>
        <div class="detail-fact"><span>Marca y modelo</span><strong>${escapeHtml(equipmentModel || 'No indicados')}</strong></div>
        <div class="detail-fact"><span>Número de serie</span><strong>${escapeHtml(order.assetSerialNumber || 'No indicado')}</strong></div>
        <div class="detail-fact"><span>Cliente / contacto</span><strong>${escapeHtml(order.requestedBy || 'No indicado')}</strong></div>
        <div class="detail-fact"><span>Teléfono</span><strong>${escapeHtml(order.customerPhone || 'No indicado')}</strong></div>
        <div class="detail-fact"><span>Correo electrónico</span><strong>${escapeHtml(order.customerEmail || 'No indicado')}</strong></div>
        <div class="detail-fact"><span>Empresa / NIT</span><strong>${escapeHtml([order.customerCompany, order.customerTaxId].filter(Boolean).join(' · ') || 'No indicados')}</strong></div>
        <div class="detail-fact"><span>Sede / referencia</span><strong>${escapeHtml(order.location || 'No indicada')}</strong></div>
        <div class="detail-fact"><span>Técnico responsable</span><strong>${escapeHtml(order.assignedTo || 'Sin asignar')}</strong></div>
        <div class="detail-fact"><span>Fecha prevista</span><strong>${escapeHtml(formatDate(order.scheduledDate, { year: true }))}</strong></div>
        <div class="detail-fact"><span>Servicio</span><strong>${escapeHtml(TYPE_LABELS[order.type] || order.type)}</strong></div>
        <div class="detail-fact"><span>Tiempo estimado</span><strong>${order.estimatedHours === null ? 'No estimado' : `${order.estimatedHours} h`}</strong></div>
      </section>

      <section class="detail-section accessories-section">
        <h3>Accesorios recibidos</h3>
        ${accessories.length
          ? `<div class="accessory-tags">${accessories.map((item) => `<span>${escapeHtml(item)}</span>`).join('')}</div>`
          : '<p>Sin accesorios registrados.</p>'}
      </section>

      ${order.observations ? `
        <section class="detail-section observations-section"><h3>Observaciones de recepción</h3><p>${escapeHtml(order.observations)}</p></section>
      ` : ''}

      <section class="detail-section order-media-section">
        <div class="section-title-row">
          <div><h3>Evidencia fotográfica y QR</h3><p>Las imágenes se guardan dentro del respaldo local de la orden.</p></div>
          <button type="button" class="secondary-button compact-button" data-action="print-qr-label">Imprimir etiqueta QR</button>
        </div>
        <div class="order-media-layout">
          <div class="order-qr-card"><div id="order-qr-preview" class="qr-loading">Generando QR...</div><strong>${escapeHtml(order.orderNumber)}</strong><small>Identificación interna</small></div>
          <div class="photo-workspace">
            ${['admin', 'technician'].includes(state.session?.role) ? `
              <div class="photo-upload-row">
                <label class="field photo-file-field"><span>Fotografía</span><input id="order-photo-file" type="file" accept="image/jpeg,image/png,image/webp"></label>
                <label class="field"><span>Etapa</span><select id="order-photo-category">${Object.entries(PHOTO_CATEGORY_LABELS).map(([value, label]) => `<option value="${value}">${escapeHtml(label)}</option>`).join('')}</select></label>
                <label class="field photo-caption-field"><span>Descripción</span><input id="order-photo-caption" maxlength="300" placeholder="Ej. Golpe en esquina izquierda"></label>
                <button type="button" class="primary-button compact-button" data-action="upload-photo">Agregar</button>
              </div>` : ''}
            <div class="photo-gallery" id="order-photo-gallery">
              ${(order.photos || []).map((photo) => `<article class="photo-card"><a target="_blank" rel="noreferrer"><div class="photo-placeholder"><span>Cargando...</span><img data-photo-image="${photo.id}" alt="${escapeHtml(photo.caption || photo.fileName)}"></div></a><div><span>${escapeHtml(PHOTO_CATEGORY_LABELS[photo.category] || photo.category)}</span><strong>${escapeHtml(photo.caption || photo.fileName)}</strong><small>${escapeHtml(photo.createdBy)} · ${escapeHtml(formatDateTime(photo.createdAt))}</small></div>${['admin', 'technician'].includes(state.session?.role) ? `<button class="icon-button danger-action" type="button" data-action="delete-photo" data-photo-id="${photo.id}" aria-label="Eliminar fotografía">&times;</button>` : ''}</article>`).join('') || '<p class="empty-copy">Aún no hay fotografías de esta orden.</p>'}
            </div>
          </div>
        </div>
      </section>

      <section class="detail-section quote-overview">
        <div class="section-title-row">
          <div>
            <h3>Diagnóstico y cotización</h3>
            <p>${order.diagnosis ? escapeHtml(order.diagnosis) : 'Aún no se ha registrado el diagnóstico técnico.'}</p>
          </div>
          <span class="quote-status-pill quote-status-${escapeHtml(order.quoteStatus || 'draft')}">${escapeHtml(QUOTE_STATUS_LABELS[order.quoteStatus] || 'Borrador')}</span>
        </div>
        <div class="quote-overview-footer">
          <div><span>Total cotizado</span><strong>${escapeHtml(formatCurrency(order.quoteTotal || 0))}</strong></div>
          <button type="button" class="secondary-button" data-action="open-quote">Abrir diagnóstico y cotización</button>
        </div>
        ${order.quoteStatus === 'approved' || order.quoteStatus === 'rejected' ? `
          <small class="quote-decision-summary">${order.quoteStatus === 'approved' ? 'Autorizada' : 'Rechazada'} por ${escapeHtml(order.authorizedBy || 'cliente')} · ${escapeHtml(AUTHORIZATION_METHOD_LABELS[order.authorizationMethod] || order.authorizationMethod || 'medio no indicado')} · ${escapeHtml(formatDateTime(order.authorizationAt))}</small>
        ` : ''}
      </section>

      <section class="detail-section resources-overview">
        <div class="section-title-row">
          <div>
            <h3>Repuestos y mano de obra</h3>
            <p>${order.parts?.length || order.labor?.length
              ? `${order.parts?.length || 0} repuestos y ${order.labor?.length || 0} trabajos registrados.`
              : 'Aún no se han registrado consumos reales para esta orden.'}</p>
          </div>
          <strong>${escapeHtml(formatCurrency(order.resourcesTotal || 0))}</strong>
        </div>
        ${order.parts?.length ? `<ul class="resource-summary-list">${order.parts.slice(0, 4).map((part) => `<li><span>${escapeHtml(part.itemName)} · ${part.quantity} ${escapeHtml(part.unit)}</span><strong>${escapeHtml(formatCurrency(part.lineTotal))}</strong></li>`).join('')}</ul>` : ''}
        ${order.labor?.length ? `<ul class="resource-summary-list labor-summary-list">${order.labor.slice(0, 4).map((line) => `<li><span>${escapeHtml(line.description)} · ${line.hours} h</span><strong>${escapeHtml(formatCurrency(line.lineTotal))}</strong></li>`).join('')}</ul>` : ''}
        <div class="resources-overview-footer">
          <small>Repuestos ${escapeHtml(formatCurrency(order.partsTotal || 0))} · Mano de obra ${escapeHtml(formatCurrency(order.laborTotal || 0))}</small>
          <button type="button" class="secondary-button" data-action="open-resources">Administrar consumos</button>
        </div>
      </section>

      <section class="detail-section financial-overview">
        <div class="section-title-row">
          <div>
            <h3>Anticipos y pagos</h3>
            <p>${isCoveredWarranty
              ? 'La cobertura fue aprobada; los consumos quedan registrados sólo para control interno.'
              : order.financialTotal > 0
              ? `El total usa ${order.resourcesTotal > 0 ? 'los cargos reales registrados' : 'la cotización actual'} como referencia.`
              : 'Aún no hay un total definido; cualquier anticipo quedará como saldo a favor.'}</p>
          </div>
          <button type="button" class="secondary-button compact-button" data-action="open-payment" ${order.status === 'cancelada' || isCoveredWarranty || (order.financialTotal > 0 && order.balanceDue <= 0) ? 'disabled' : ''}>${isCoveredWarranty ? 'Cubierta por garantía' : order.financialTotal > 0 && order.balanceDue <= 0 ? 'Saldo cubierto' : '+ Registrar pago'}</button>
        </div>
        <div class="financial-summary">
          <div><span>Total de la orden</span><strong>${isCoveredWarranty ? 'Q0.00 · Garantía' : order.financialTotal > 0 ? escapeHtml(formatCurrency(order.financialTotal)) : 'Por definir'}</strong></div>
          <div><span>Total pagado</span><strong>${escapeHtml(formatCurrency(order.paymentsTotal))}</strong></div>
          <div class="${order.balanceDue > 0 ? 'has-balance' : 'is-paid'}"><span>Saldo pendiente</span><strong>${escapeHtml(formatCurrency(order.balanceDue))}</strong></div>
          ${order.creditBalance > 0 ? `<div class="has-credit"><span>Saldo a favor</span><strong>${escapeHtml(formatCurrency(order.creditBalance))}</strong></div>` : ''}
        </div>
        ${(order.payments || []).length ? `<ul class="payment-history-list">${paymentRows}</ul>` : '<p class="payment-empty-copy">No hay anticipos ni pagos registrados.</p>'}
      </section>

      ${order.warrantyClaim ? `
        <section class="detail-section warranty-claim-overview">
          <div class="section-title-row">
            <div>
              <h3>Regreso por garantía · ${escapeHtml(order.warrantyClaim.claimNumber)}</h3>
              <p>Relacionado con la orden <button type="button" class="inline-link-button" data-action="open-linked-order" data-order-id="${order.warrantyClaim.originalOrderId}">${escapeHtml(order.warrantyClaim.originalOrderNumber)}</button>.</p>
            </div>
            <span class="warranty-status warranty-${order.warrantyClaim.withinWarranty ? 'active' : 'expired'}">${order.warrantyClaim.withinWarranty ? 'Recibido dentro de vigencia' : 'Recibido fuera de vigencia'}</span>
          </div>
          <div class="warranty-claim-facts">
            <div><span>Falla reportada</span><p>${escapeHtml(order.warrantyClaim.reportedIssue)}</p></div>
            <div><span>Condición al reingresar</span><p>${escapeHtml(order.warrantyClaim.receivedCondition || 'Sin observaciones adicionales')}</p></div>
            <div><span>Garantía vencía</span><strong>${escapeHtml(formatDate(order.warrantyClaim.warrantyExpiresAt, { year: true }))}</strong></div>
          </div>
          <div class="warranty-decision-editor">
            <label class="field">
              <span>Decisión de cobertura</span>
              <select id="warranty-decision">
                ${Object.entries(WARRANTY_DECISION_LABELS).map(([value, label]) => `<option value="${value}" ${value === order.warrantyClaim.decision ? 'selected' : ''}>${escapeHtml(label)}</option>`).join('')}
              </select>
            </label>
            <label class="field field-wide">
              <span>Evaluación y resolución</span>
              <textarea id="warranty-resolution" rows="4" maxlength="4000" placeholder="Describe las pruebas y por qué aplica o no la cobertura...">${escapeHtml(order.warrantyClaim.resolution || '')}</textarea>
              <small class="field-error" data-error-for="resolution"></small>
            </label>
            <button type="button" class="secondary-button" data-action="save-warranty-decision">Guardar decisión</button>
          </div>
          ${order.warrantyClaim.decision === 'covered' ? '<p class="warranty-covered-note">Esta orden está cubierta: los consumos se conservan para control interno, pero el total a cobrar al cliente es Q0.00.</p>' : ''}
        </section>
      ` : ''}

      <section class="detail-section warranty-overview">
        <div class="section-title-row">
          <div>
            <h3>Garantía del servicio</h3>
            <p>${order.warranty ? `Cobertura ${escapeHtml(order.warranty.warrantyNumber)} registrada para este trabajo.` : 'Aún no se han definido condiciones de garantía para esta orden.'}</p>
          </div>
          ${order.warranty ? `<span class="warranty-status warranty-${escapeHtml(order.warranty.status)}">${order.warranty.status === 'active' ? 'Vigente' : order.warranty.status === 'expired' ? 'Vencida' : 'Anulada'}</span>` : ''}
        </div>
        ${order.warranty ? `
          <div class="warranty-period"><div><span>Inicio</span><strong>${escapeHtml(formatDate(order.warranty.startsAt, { year: true }))}</strong></div><div><span>Vencimiento</span><strong>${escapeHtml(formatDate(order.warranty.expiresAt, { year: true }))}</strong></div><div><span>Vigencia</span><strong>${order.warranty.durationDays} días</strong></div></div>
          <div class="warranty-copy"><span>Cobertura</span><p>${escapeHtml(order.warranty.coverage)}</p></div>
          ${order.warranty.exclusions ? `<div class="warranty-copy"><span>Exclusiones</span><p>${escapeHtml(order.warranty.exclusions)}</p></div>` : ''}
          ${(order.warrantyReturns || []).length ? `<ul class="warranty-return-list">${order.warrantyReturns.map((claim) => `<li><button type="button" data-action="open-linked-order" data-order-id="${claim.returnOrderId}"><strong>${escapeHtml(claim.claimNumber)}</strong><span>${escapeHtml(claim.returnOrderNumber)} · ${escapeHtml(WARRANTY_DECISION_LABELS[claim.decision] || claim.decision)}</span></button></li>`).join('')}</ul>` : ''}
          <div class="warranty-actions"><button type="button" class="secondary-button compact-button" data-action="edit-warranty">Editar condiciones</button><button type="button" class="primary-button compact-button" data-action="new-warranty-return">Recibir regreso</button></div>
        ` : `
          <button type="button" class="secondary-button" data-action="edit-warranty" ${['completada', 'entregada'].includes(order.status) ? '' : 'disabled'}>Definir garantía</button>
          ${!['completada', 'entregada'].includes(order.status) ? '<small class="warranty-help">Disponible cuando la reparación esté lista o entregada.</small>' : ''}
        `}
      </section>

      <section class="detail-section status-editor">
        <div><h3>Actualizar estado</h3><p>Sólo se muestran habilitados los siguientes pasos permitidos.</p></div>
        <select id="detail-status-select" aria-label="Estado de la orden">
          ${Object.entries(STATUS_LABELS).map(([value, label]) => {
            const allowed = (STATUS_TRANSITIONS[order.status] || [order.status]).includes(value);
            return `<option value="${value}" ${value === order.status ? 'selected' : ''} ${allowed ? '' : 'disabled'}>${escapeHtml(label)}</option>`;
          }).join('')}
        </select>
      </section>

      ${order.tasks.length ? `
        <section class="detail-section task-section">
          <div class="section-title-row"><div><h3>Lista de trabajo</h3><p>${order.completedTaskCount} de ${order.taskCount} tareas completadas</p></div><strong>${progress}%</strong></div>
          <progress value="${progress}" max="100" aria-label="Progreso de tareas: ${progress}%"></progress>
          <div class="task-list">
            ${order.tasks.map((task) => `
              <label class="task-item ${task.completed ? 'is-complete' : ''}">
                <input type="checkbox" data-task-id="${task.id}" ${task.completed ? 'checked' : ''} ${['pendiente_autorizacion', 'completada', 'entregada', 'cancelada'].includes(order.status) ? 'disabled' : ''}>
                <span class="custom-check">✓</span>
                <span>${escapeHtml(task.description)}</span>
              </label>
            `).join('')}
          </div>
        </section>
      ` : ''}

      ${order.safetyNotes ? `
        <section class="detail-section safety-section"><span>i</span><div><h3>Notas técnicas</h3><p>${escapeHtml(order.safetyNotes)}</p></div></section>
      ` : ''}

      <section class="detail-section delivery-editor">
        <div class="section-title-row">
          <div><h3>Datos de entrega</h3><p>Completa estos campos para generar la constancia de entrega.</p></div>
          <span class="saved-indicator">${order.workPerformed && order.deliveryCondition && order.deliveryTestedBy ? 'Datos guardados' : 'Pendiente'}</span>
        </div>
        <label class="field field-wide">
          <span>Trabajo realizado</span>
          <textarea id="delivery-work-performed" rows="5" maxlength="6000" placeholder="Describe el diagnóstico, las piezas reemplazadas, la limpieza y las pruebas realizadas...">${escapeHtml(order.workPerformed || '')}</textarea>
        </label>
        <label class="field field-wide">
          <span>Estado en que se entrega</span>
          <select id="delivery-condition">
            <option value="">Seleccionar…</option>
            ${Object.entries(DELIVERY_CONDITION_LABELS).map(([value, label]) => `<option value="${value}" ${value === order.deliveryCondition ? 'selected' : ''}>${escapeHtml(label)}</option>`).join('')}
          </select>
        </label>
        <label class="field field-wide">
          <span>Prueba de funcionamiento</span>
          <select id="delivery-tested-by">
            <option value="">Seleccionar quién realizó la prueba…</option>
            ${Object.entries(DELIVERY_TESTER_LABELS).map(([value, label]) => `<option value="${value}" ${value === order.deliveryTestedBy ? 'selected' : ''}>${escapeHtml(label)}</option>`).join('')}
          </select>
        </label>
        <button type="button" class="secondary-button save-delivery-button" data-action="save-delivery">Guardar datos de entrega</button>
      </section>

      <section class="detail-section history-section">
        <h3>Historial</h3>
        <ul>${eventRows}</ul>
      </section>
    </div>
    <div class="drawer-footer detail-footer">
      <button class="ghost-button" data-action="close-detail">Cerrar</button>
      <button class="secondary-button" data-action="print-reception">Imprimir recepción</button>
      <button class="primary-button" data-action="print-delivery">Imprimir entrega</button>
    </div>
  `;
  applyPermissions($('#detail-content'));
  hydrateOrderMedia(order);
}

function printFact(label, value) {
  return `<div class="print-fact"><span>${escapeHtml(label)}</span><strong>${escapeHtml(value || 'No indicado')}</strong></div>`;
}

function printHeader(order, documentTitle, documentSubtitle) {
  return `
    <header class="print-header">
      <div class="print-brand">
        <img src="/logo-geecko.png" alt="Geecko">
        <small>Servicio técnico de computadoras</small>
      </div>
      <div class="print-document-title">
        <span>${escapeHtml(documentSubtitle)}</span>
        <h1>${escapeHtml(documentTitle)}</h1>
        <strong>${escapeHtml(order.orderNumber)}</strong>
      </div>
      ${order.qrSvg ? `<div class="print-header-qr">${order.qrSvg}<small>Escanear para identificar</small></div>` : ''}
    </header>
  `;
}

function printEquipmentGrid(order) {
  return `
    <div class="print-fact-grid">
      ${printFact('Equipo', order.assetName || 'Sin identificar')}
      ${printFact('Código interno', order.assetCode || 'Sin código')}
      ${printFact('Tipo', order.assetCategory)}
      ${printFact('Marca', order.assetManufacturer)}
      ${printFact('Modelo', order.assetModel)}
      ${printFact('Número de serie', order.assetSerialNumber)}
    </div>
  `;
}

function printAccessoryList(order, emptyMessage = 'Sin accesorios registrados.') {
  const accessories = order.accessories || [];
  return accessories.length
    ? `<ul class="print-inline-list">${accessories.map((item) => `<li>${escapeHtml(item)}</li>`).join('')}</ul>`
    : `<p class="print-muted">${escapeHtml(emptyMessage)}</p>`;
}

function buildReceptionPrint(order) {
  const tasks = order.tasks || [];
  return `
    <article class="print-sheet print-reception-sheet">
      ${printHeader(order, 'Constancia de recepción', 'Documento para el cliente')}
      <div class="print-meta-row">
        ${printFact('Fecha de ingreso', formatDateTime(order.createdAt))}
        ${printFact('Cliente / contacto', [order.requestedBy, order.customerPhone].filter(Boolean).join(' · '))}
        ${printFact('Empresa / referencia', order.location)}
      </div>

      ${order.warrantyClaim ? `
        <section class="print-section print-warranty-return">
          <h2>Reingreso por garantía</h2>
          <div class="print-meta-row">
            ${printFact('Registro de regreso', order.warrantyClaim.claimNumber)}
            ${printFact('Orden original', order.warrantyClaim.originalOrderNumber)}
            ${printFact('Vigencia al recibir', order.warrantyClaim.withinWarranty ? 'Dentro del plazo' : 'Fuera del plazo')}
          </div>
          <div class="print-copy-block"><span>Falla reportada</span><p>${escapeHtml(order.warrantyClaim.reportedIssue)}</p></div>
          ${order.warrantyClaim.receivedCondition ? `<div class="print-copy-block"><span>Condición al reingresar</span><p>${escapeHtml(order.warrantyClaim.receivedCondition)}</p></div>` : ''}
          <p class="print-muted">La recepción dentro del plazo no implica aprobación automática; la cobertura se determina después de la evaluación técnica y conforme a sus condiciones.</p>
        </section>
      ` : ''}

      <section class="print-section">
        <h2>Información del equipo recibido</h2>
        ${printEquipmentGrid(order)}
      </section>

      <section class="print-section">
        <h2>Accesorios entregados con el equipo</h2>
        ${printAccessoryList(order)}
      </section>

      <section class="print-section print-work-section">
        <h2>Servicio solicitado</h2>
        <div class="print-copy-block"><span>Motivo de ingreso</span><strong>${escapeHtml(order.title)}</strong></div>
        <div class="print-copy-block"><span>Falla reportada / descripción</span><p>${escapeHtml(order.description)}</p></div>
        <div class="print-copy-block"><span>Tipo de servicio</span><p>${escapeHtml(TYPE_LABELS[order.type] || order.type)}</p></div>
        ${tasks.length ? `<div class="print-copy-block"><span>Trabajo previsto</span><ul class="print-task-list">${tasks.map((task) => `<li>${escapeHtml(task.description)}</li>`).join('')}</ul></div>` : ''}
      </section>

      ${order.observations ? `
        <section class="print-section">
          <h2>Observaciones de recepción</h2>
          <p class="print-long-copy">${escapeHtml(order.observations)}</p>
        </section>
      ` : ''}

      <section class="print-acknowledgement">
        <p>Confirmo que la información del equipo y los accesorios entregados corresponde a lo recibido para revisión o reparación.</p>
        <div class="print-signatures single-signature">
          <div><span></span><strong>Firma del cliente de enterado</strong><small>Nombre y fecha</small></div>
        </div>
      </section>

      <div class="print-internal-ticket">
        <div class="print-cut-line"><span>✂</span><strong>Corte para identificación interna</strong></div>
        <section class="print-internal-stub">
          <div class="stub-code"><span>Código del equipo</span><strong>${escapeHtml(order.assetCode || order.orderNumber)}</strong></div>
          <div class="stub-data">
            ${printFact('Orden', order.orderNumber)}
            ${printFact('Equipo', order.assetName || order.assetCategory)}
            ${printFact('Cliente', order.requestedBy)}
            ${printFact('Ingreso', formatDate(order.createdAt, { year: true }))}
          </div>
        </section>
      </div>
    </article>
  `;
}

function buildDeliveryPrint(order) {
  const deliveryDate = new Intl.DateTimeFormat('es-GT', {
    day: '2-digit', month: 'long', year: 'numeric'
  }).format(new Date());
  const testerLabel = DELIVERY_TESTER_LABELS[order.deliveryTestedBy] || order.deliveryTestedBy;
  const deliveryAcknowledgement = order.deliveryTestedBy === 'tecnico'
    ? 'El equipo fue probado por un técnico antes de la entrega debido a que el cliente no estuvo presente. La persona que recibe confirma la entrega del equipo y los accesorios descritos.'
    : 'El cliente realizó la prueba de funcionamiento y recibe el equipo y los accesorios descritos, después de ser informado del trabajo realizado y de su estado de entrega.';
  const partRows = (order.parts || []).map((part) => `
    <tr><td>${escapeHtml(part.itemCode)}</td><td>${escapeHtml(part.itemName)}</td><td class="print-number">${part.quantity} ${escapeHtml(part.unit)}</td><td class="print-number">${escapeHtml(formatCurrency(part.unitPrice))}</td><td class="print-number"><strong>${escapeHtml(formatCurrency(part.lineTotal))}</strong></td></tr>
  `).join('');
  const laborRows = (order.labor || []).map((line) => `
    <tr><td>${escapeHtml(line.technician)}</td><td>${escapeHtml(line.description)}</td><td class="print-number">${line.hours} h</td><td class="print-number">${escapeHtml(formatCurrency(line.hourlyRate))}</td><td class="print-number"><strong>${escapeHtml(formatCurrency(line.lineTotal))}</strong></td></tr>
  `).join('');
  const warrantyPrint = order.warranty ? `
    <section class="print-section print-warranty-section">
      <h2>Garantía del servicio · ${escapeHtml(order.warranty.warrantyNumber)}</h2>
      <div class="print-meta-row">
        ${printFact('Inicio', formatDate(order.warranty.startsAt, { year: true }))}
        ${printFact('Vencimiento', formatDate(order.warranty.expiresAt, { year: true }))}
        ${printFact('Vigencia', `${order.warranty.durationDays} días`)}
      </div>
      <div class="print-copy-block"><span>Cobertura</span><p>${escapeHtml(order.warranty.coverage)}</p></div>
      ${order.warranty.exclusions ? `<div class="print-copy-block"><span>Exclusiones</span><p>${escapeHtml(order.warranty.exclusions)}</p></div>` : ''}
    </section>
  ` : order.warrantyClaim ? `
    <section class="print-section print-warranty-section">
      <h2>Resolución de regreso · ${escapeHtml(order.warrantyClaim.claimNumber)}</h2>
      <div class="print-meta-row">
        ${printFact('Orden original', order.warrantyClaim.originalOrderNumber)}
        ${printFact('Recepción', order.warrantyClaim.withinWarranty ? 'Dentro de vigencia' : 'Fuera de vigencia')}
        ${printFact('Cobertura', WARRANTY_DECISION_LABELS[order.warrantyClaim.decision] || order.warrantyClaim.decision)}
      </div>
      <div class="print-copy-block"><span>Falla reportada</span><p>${escapeHtml(order.warrantyClaim.reportedIssue)}</p></div>
      ${order.warrantyClaim.resolution ? `<div class="print-copy-block"><span>Evaluación y resolución</span><p>${escapeHtml(order.warrantyClaim.resolution)}</p></div>` : ''}
      ${order.warrantyClaim.decision === 'covered' ? '<p class="print-warranty-covered">Servicio cubierto por garantía: total a cobrar al cliente Q0.00.</p>' : ''}
    </section>
  ` : '';
  return `
    <article class="print-sheet print-delivery-sheet">
      ${printHeader(order, 'Constancia de entrega', 'Documento para el cliente')}
      <div class="print-meta-row">
        ${printFact('Fecha de entrega', deliveryDate)}
        ${printFact('Cliente / contacto', [order.requestedBy, order.customerPhone].filter(Boolean).join(' · '))}
        ${printFact('Técnico responsable', order.assignedTo)}
      </div>

      <section class="print-section">
        <h2>Equipo entregado</h2>
        ${printEquipmentGrid(order)}
      </section>

      <section class="print-section">
        <h2>Trabajo realizado</h2>
        <p class="print-long-copy">${escapeHtml(order.workPerformed)}</p>
      </section>

      ${partRows || laborRows ? `
        <section class="print-section print-resources-section">
          <h2>Repuestos y mano de obra registrados</h2>
          ${partRows ? `<h3>Repuestos utilizados</h3><table class="print-resource-table"><thead><tr><th>Código</th><th>Repuesto</th><th>Cantidad</th><th>Precio</th><th>Subtotal</th></tr></thead><tbody>${partRows}</tbody></table>` : ''}
          ${laborRows ? `<h3>Mano de obra</h3><table class="print-resource-table"><thead><tr><th>Técnico</th><th>Actividad</th><th>Horas</th><th>Tarifa</th><th>Subtotal</th></tr></thead><tbody>${laborRows}</tbody></table>` : ''}
          <div class="print-resource-total"><span>Total registrado</span><strong>${escapeHtml(formatCurrency(order.resourcesTotal || 0))}</strong></div>
        </section>
      ` : ''}

      ${order.financialTotal > 0 || order.paymentsTotal > 0 ? `
        <section class="print-section">
          <h2>Estado de cuenta</h2>
          <div class="payment-balance-print">
            <div><span>Total de la orden</span><strong>${order.financialTotal > 0 ? escapeHtml(formatCurrency(order.financialTotal)) : 'Por definir'}</strong></div>
            <div><span>Total pagado</span><strong>${escapeHtml(formatCurrency(order.paymentsTotal))}</strong></div>
            <div><span>Saldo pendiente</span><strong>${escapeHtml(formatCurrency(order.balanceDue))}</strong></div>
            <div><span>Saldo a favor</span><strong>${escapeHtml(formatCurrency(order.creditBalance))}</strong></div>
          </div>
        </section>
      ` : ''}

      ${warrantyPrint}

      <section class="print-section print-delivery-summary">
        <div class="print-condition-section">
          <h2>Estado de entrega</h2>
          <strong>${escapeHtml(DELIVERY_CONDITION_LABELS[order.deliveryCondition] || order.deliveryCondition)}</strong>
        </div>
        <div class="print-test-section">
          <h2>Prueba de funcionamiento</h2>
          <strong>${escapeHtml(testerLabel)}</strong>
        </div>
      </section>

      <section class="print-section">
        <h2>Accesorios devueltos con el equipo</h2>
        ${printAccessoryList(order)}
      </section>

      ${order.observations ? `
        <section class="print-section">
          <h2>Observaciones registradas al recibir</h2>
          <p class="print-long-copy">${escapeHtml(order.observations)}</p>
        </section>
      ` : ''}

      <section class="print-acknowledgement delivery-acknowledgement">
        <p>${escapeHtml(deliveryAcknowledgement)}</p>
        <div class="print-signatures">
          <div><span></span><strong>Firma de recibido del cliente</strong><small>Nombre y fecha</small></div>
          <div><span></span><strong>Entrega del técnico</strong><small>Nombre y fecha</small></div>
        </div>
      </section>
    </article>
  `;
}

function buildQuotePrint(order) {
  const items = order.quoteItems || [];
  const rows = items.map((item) => `
    <tr>
      <td><small>${escapeHtml(QUOTE_ITEM_LABELS[item.kind] || item.kind)}</small><strong>${escapeHtml(item.description)}</strong></td>
      <td class="print-number">${escapeHtml(String(item.quantity))}</td>
      <td class="print-number">${escapeHtml(formatCurrency(item.unitPrice))}</td>
      <td class="print-number"><strong>${escapeHtml(formatCurrency(item.lineTotal))}</strong></td>
    </tr>
  `).join('');
  const decision = order.quoteStatus === 'approved' || order.quoteStatus === 'rejected'
    ? `<div class="print-quote-decision"><strong>${order.quoteStatus === 'approved' ? 'APROBADA' : 'RECHAZADA'}</strong><span>Por ${escapeHtml(order.authorizedBy || 'cliente')} vía ${escapeHtml(AUTHORIZATION_METHOD_LABELS[order.authorizationMethod] || order.authorizationMethod || 'medio no indicado')} · ${escapeHtml(formatDateTime(order.authorizationAt))}</span>${order.authorizationNote ? `<p>${escapeHtml(order.authorizationNote)}</p>` : ''}</div>`
    : '';
  return `
    <article class="print-sheet print-quote-sheet">
      ${printHeader(order, 'Diagnóstico y cotización', 'Documento para autorización')}
      <div class="print-meta-row">
        ${printFact('Fecha', formatDate(new Date().toISOString().slice(0, 10), { year: true }))}
        ${printFact('Cliente / contacto', [order.requestedBy, order.customerPhone].filter(Boolean).join(' · '))}
        ${printFact('Estado', QUOTE_STATUS_LABELS[order.quoteStatus] || 'Borrador')}
      </div>
      <section class="print-section">
        <h2>Equipo revisado</h2>
        ${printEquipmentGrid(order)}
      </section>
      <section class="print-section">
        <h2>Diagnóstico técnico</h2>
        <p class="print-long-copy">${escapeHtml(order.diagnosis)}</p>
        ${order.recommendation ? `<div class="print-copy-block"><span>Recomendación</span><p>${escapeHtml(order.recommendation)}</p></div>` : ''}
      </section>
      <section class="print-section">
        <h2>Detalle de la cotización</h2>
        <table class="print-quote-table">
          <thead><tr><th>Concepto</th><th>Cantidad</th><th>Precio unitario</th><th>Subtotal</th></tr></thead>
          <tbody>${rows}</tbody>
        </table>
        <div class="print-quote-totals">
          <div><span>Subtotal</span><strong>${escapeHtml(formatCurrency(order.quoteSubtotal))}</strong></div>
          <div><span>Descuento</span><strong>− ${escapeHtml(formatCurrency(order.quoteDiscount))}</strong></div>
          <div><span>Total a autorizar</span><strong>${escapeHtml(formatCurrency(order.quoteTotal))}</strong></div>
        </div>
      </section>
      ${decision}
      <section class="print-acknowledgement quote-acknowledgement">
        <p>Autorizo los trabajos y cargos descritos en esta cotización. Cualquier trabajo adicional deberá ser informado antes de realizarse.</p>
        <div class="print-signatures">
          <div><span></span><strong>Autorización del cliente</strong><small>Nombre, firma y fecha</small></div>
          <div><span></span><strong>Responsable Geecko</strong><small>Nombre y fecha</small></div>
        </div>
      </section>
    </article>
  `;
}

function buildPurchaseReportPrint(report) {
  const rows = report.lines.map((line) => `
    <tr>
      <td><strong>${escapeHtml(line.itemCode)}</strong></td>
      <td>${escapeHtml(line.itemName)}</td>
      <td class="print-number">${line.stockQuantity}</td>
      <td class="print-number">${line.purchaseQuantity} ${escapeHtml(line.unit)}</td>
      <td class="print-number">${escapeHtml(formatCurrency(line.unitCost))}</td>
      <td class="print-number"><strong>${escapeHtml(formatCurrency(line.lineTotal))}</strong></td>
    </tr>
  `).join('');
  return `
    <article class="print-sheet print-purchase-sheet">
      <header class="print-header">
        <div class="print-brand">
          <img src="/logo-geecko.png" alt="Geecko">
          <small>Servicio técnico de computadoras</small>
        </div>
        <div class="print-document-title">
          <span>Control interno</span>
          <h1>Reporte de compras</h1>
          <strong>${escapeHtml(report.reportNumber)}</strong>
        </div>
      </header>
      <div class="print-meta-row">
        ${printFact('Fecha de generación', formatDateTime(report.createdAt))}
        ${printFact('Proveedor / referencia', report.supplier || 'Por definir')}
        ${printFact('Productos incluidos', String(report.lines.length))}
      </div>
      <section class="print-section">
        <h2>Productos solicitados</h2>
        <table class="print-purchase-table">
          <thead><tr><th>Código</th><th>Producto</th><th>Existencia</th><th>Comprar</th><th>Costo unitario</th><th>Subtotal</th></tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </section>
      ${report.notes ? `<section class="print-section"><h2>Notas de compra</h2><p class="print-long-copy">${escapeHtml(report.notes)}</p></section>` : ''}
      <div class="print-purchase-total"><span>Total estimado</span><strong>${escapeHtml(formatCurrency(report.totalCost))}</strong></div>
      <div class="print-signatures purchase-signatures">
        <div><span></span><strong>Elaborado por</strong><small>Nombre y fecha</small></div>
        <div><span></span><strong>Autorizado por</strong><small>Nombre y fecha</small></div>
      </div>
    </article>
  `;
}

function buildSalePrint(sale) {
  const rows = sale.lines.map((line) => `
    <tr>
      <td><strong>${escapeHtml(line.itemCode)}</strong><br>${escapeHtml(line.itemName)}</td>
      <td class="print-number">${line.quantity} ${escapeHtml(line.unit)}</td>
      <td class="print-number">${escapeHtml(formatCurrency(line.unitPrice))}</td>
      <td class="print-number"><strong>${escapeHtml(formatCurrency(line.lineTotal))}</strong></td>
    </tr>
  `).join('');
  return `
    <article class="print-sheet print-sale-sheet">
      <header class="print-header">
        <div class="print-brand"><img src="/logo-geecko.png" alt="Geecko"><small>Servicio técnico y accesorios</small></div>
        <div class="print-document-title">
          <span>Comprobante interno · No es factura fiscal ni FEL</span>
          <h1>Venta directa</h1>
          <strong>${escapeHtml(sale.saleNumber)}</strong>
        </div>
      </header>
      <div class="print-meta-row">
        ${printFact('Fecha', formatDateTime(sale.createdAt))}
        ${printFact('Cliente', sale.customerName || 'Consumidor final')}
        ${printFact('Estado', sale.status === 'cancelled' ? 'ANULADA' : 'Completada')}
      </div>
      <section class="print-section">
        <h2>Productos</h2>
        <table class="print-quote-table">
          <thead><tr><th>Producto</th><th>Cantidad</th><th>Precio unitario</th><th>Subtotal</th></tr></thead>
          <tbody>${rows}</tbody>
        </table>
        <div class="print-quote-totals">
          <div><span>Subtotal</span><strong>${escapeHtml(formatCurrency(sale.subtotal))}</strong></div>
          <div><span>Descuento</span><strong>− ${escapeHtml(formatCurrency(sale.discount))}</strong></div>
          <div><span>Total</span><strong>${escapeHtml(formatCurrency(sale.total))}</strong></div>
        </div>
      </section>
      ${sale.notes ? `<section class="print-section"><h2>Observaciones</h2><p class="print-long-copy">${escapeHtml(sale.notes)}</p></section>` : ''}
      ${sale.status === 'cancelled' ? `<section class="print-section"><h2>Venta anulada</h2><p class="print-long-copy">${escapeHtml(sale.cancellationNote || 'Sin motivo registrado.')}</p></section>` : ''}
      <div class="print-signatures purchase-signatures"><div><span></span><strong>Entregado por</strong><small>Nombre y fecha</small></div><div><span></span><strong>Recibido por</strong><small>Nombre y firma</small></div></div>
    </article>
  `;
}

function buildPaymentReceiptPrint(data) {
  const { order, payment } = data;
  const equipment = [order.assetName, order.assetManufacturer, order.assetModel].filter(Boolean).join(' · ');
  const hasDefinedTotal = payment.orderTotal > 0;
  return `
    <article class="print-sheet print-payment-sheet">
      <header class="print-header">
        <div class="print-brand"><img src="/logo-geecko.png" alt="Geecko"><small>Servicio técnico de computadoras</small></div>
        <div class="print-document-title">
          <span>Comprobante interno de cobro</span>
          <h1>Recibo de ${escapeHtml((PAYMENT_TYPE_LABELS[payment.paymentType] || 'pago').toLowerCase())}</h1>
          <strong>${escapeHtml(payment.receiptNumber)}</strong>
        </div>
      </header>
      ${payment.status === 'cancelled' ? '<div class="print-cancelled-banner">RECIBO ANULADO</div>' : ''}
      <div class="print-meta-row">
        ${printFact('Fecha', formatDateTime(payment.createdAt))}
        ${printFact('Orden de servicio', order.orderNumber)}
        ${printFact('Cliente', order.requestedBy || 'No indicado')}
      </div>
      <section class="payment-receipt-amount">
        <span>Recibimos la cantidad de</span>
        <strong>${escapeHtml(formatCurrency(payment.amount))}</strong>
        <small>${escapeHtml(PAYMENT_TYPE_LABELS[payment.paymentType] || payment.paymentType)} · ${escapeHtml(PAYMENT_METHOD_LABELS[payment.method] || payment.method)}</small>
      </section>
      <section class="print-section">
        <h2>Información del servicio</h2>
        <div class="print-meta-row payment-service-grid">
          ${printFact('Trabajo', order.title)}
          ${printFact('Equipo', equipment || 'No indicado')}
          ${printFact('Código de equipo', order.assetCode || 'No indicado')}
        </div>
      </section>
      <section class="print-section">
        <h2>Aplicación del pago</h2>
        <div class="payment-balance-print">
          <div><span>Total de referencia</span><strong>${hasDefinedTotal ? escapeHtml(formatCurrency(payment.orderTotal)) : 'Por definir'}</strong></div>
          <div><span>Saldo anterior</span><strong>${hasDefinedTotal ? escapeHtml(formatCurrency(payment.balanceBefore)) : 'Por definir'}</strong></div>
          <div><span>Este pago</span><strong>${escapeHtml(formatCurrency(payment.amount))}</strong></div>
          <div><span>Saldo posterior</span><strong>${hasDefinedTotal ? escapeHtml(formatCurrency(payment.balanceAfter)) : 'Saldo a favor'}</strong></div>
        </div>
        ${!hasDefinedTotal ? '<p class="print-long-copy">El total del servicio aún no estaba definido al emitir este recibo. El monto queda registrado como anticipo a favor del cliente.</p>' : ''}
      </section>
      <section class="print-section payment-detail-print">
        <h2>Detalle del cobro</h2>
        <div class="print-meta-row">
          ${printFact('Forma de pago', PAYMENT_METHOD_LABELS[payment.method] || payment.method)}
          ${printFact('Referencia', payment.reference || 'Sin referencia')}
          ${printFact('Recibido por', payment.receivedBy)}
        </div>
        ${payment.note ? `<p class="print-long-copy">${escapeHtml(payment.note)}</p>` : ''}
        ${payment.status === 'cancelled' ? `<p class="print-long-copy"><strong>Motivo de anulación:</strong> ${escapeHtml(payment.cancellationNote || 'No indicado')}</p>` : ''}
      </section>
      <p class="payment-receipt-disclaimer">Este documento es un recibo interno asociado a la orden de servicio indicada. No constituye factura fiscal ni FEL.</p>
      <div class="print-signatures purchase-signatures">
        <div><span></span><strong>Recibido por Geecko</strong><small>${escapeHtml(payment.receivedBy)} · Firma</small></div>
        <div><span></span><strong>Cliente</strong><small>Nombre y firma</small></div>
      </div>
    </article>
  `;
}

function buildQrLabelPrint(order) {
  return `<article class="print-sheet qr-label-sheet"><section class="qr-label-card"><img src="/logo-geecko.png" alt="Geecko"><div class="qr-label-code">${order.qrSvg || ''}</div><h1>${escapeHtml(order.orderNumber)}</h1><strong>${escapeHtml(order.assetCode || 'Sin código interno')}</strong><p>${escapeHtml(order.assetName || order.assetCategory || 'Equipo')}</p><small>${escapeHtml(order.requestedBy || 'Cliente no indicado')}</small></section></article>`;
}

async function printDocument(kind, documentData) {
  const order = kind === 'payment' ? documentData.order : documentData;
  if (order?.id && !order.qrSvg) {
    try {
      const qr = state.qrCache.get(order.id) || await api(`/api/orders/${order.id}/qr`);
      state.qrCache.set(order.id, qr);
      order.qrSvg = qr.svg;
    } catch (error) {
      console.error('QR de impresion:', error);
    }
  }
  const root = $('#print-root');
  if (kind === 'purchase') root.innerHTML = buildPurchaseReportPrint(documentData);
  else if (kind === 'sale') root.innerHTML = buildSalePrint(documentData);
  else if (kind === 'payment') root.innerHTML = buildPaymentReceiptPrint(documentData);
  else if (kind === 'quote') root.innerHTML = buildQuotePrint(documentData);
  else if (kind === 'qr') root.innerHTML = buildQrLabelPrint(documentData);
  else root.innerHTML = kind === 'delivery' ? buildDeliveryPrint(documentData) : buildReceptionPrint(documentData);
  root.setAttribute('aria-hidden', 'false');
  document.body.classList.add('is-printing');
  requestAnimationFrame(() => requestAnimationFrame(() => window.print()));
}

async function saveDeliveryDetails(requireComplete = false) {
  if (!state.selectedOrder) return null;
  const workPerformed = $('#delivery-work-performed')?.value.trim() || '';
  const deliveryCondition = $('#delivery-condition')?.value || '';
  const deliveryTestedBy = $('#delivery-tested-by')?.value || '';
  if (requireComplete && !workPerformed) {
    showToast('Describe el trabajo realizado antes de imprimir la entrega.', 'error');
    $('#delivery-work-performed')?.focus();
    return null;
  }
  if (requireComplete && !deliveryCondition) {
    showToast('Selecciona el estado en que se entrega el equipo.', 'error');
    $('#delivery-condition')?.focus();
    return null;
  }
  if (requireComplete && !deliveryTestedBy) {
    showToast('Indica quién realizó la prueba de funcionamiento.', 'error');
    $('#delivery-tested-by')?.focus();
    return null;
  }
  if (workPerformed === (state.selectedOrder.workPerformed || '')
      && deliveryCondition === (state.selectedOrder.deliveryCondition || '')
      && deliveryTestedBy === (state.selectedOrder.deliveryTestedBy || '')) {
    return state.selectedOrder;
  }
  try {
    const order = await api(`/api/orders/${state.selectedOrder.id}`, {
      method: 'PATCH',
      body: JSON.stringify({ workPerformed, deliveryCondition, deliveryTestedBy })
    });
    state.selectedOrder = order;
    renderOrderDetail(order);
    await refreshAll();
    return order;
  } catch (error) {
    showToast(error.message, 'error');
    return null;
  }
}

function eventLabel(event) {
  if (event.action === 'created') return 'Orden creada';
  if (event.action === 'status_changed') return `Estado: ${STATUS_LABELS[event.newStatus] || event.newStatus}`;
  if (event.action === 'task_completed') return 'Tarea completada';
  if (event.action === 'task_reopened') return 'Tarea reabierta';
  if (event.action === 'quote_saved') return event.note || 'Diagnóstico y cotización guardados';
  if (event.action === 'quote_requested') return 'Cotización pendiente de autorización';
  if (event.action === 'quote_approved') return event.note || 'Cotización aprobada';
  if (event.action === 'quote_rejected') return event.note || 'Cotización rechazada';
  if (event.action === 'resources_saved') return event.note || 'Repuestos y mano de obra actualizados';
  if (event.action === 'photo_added') return event.note || 'Fotografía agregada';
  if (event.action === 'photo_deleted') return event.note || 'Fotografía eliminada';
  if (event.action === 'payment_received') return event.note || 'Pago recibido';
  if (event.action === 'payment_cancelled') return event.note || 'Pago anulado';
  if (event.action === 'warranty_saved') return event.note || 'Garantía registrada';
  if (event.action === 'warranty_return_created') return event.note || 'Orden de regreso creada';
  if (event.action === 'warranty_return_received') return event.note || 'Equipo recibido nuevamente';
  if (event.action === 'warranty_covered') return 'Regreso cubierto por garantía';
  if (event.action === 'warranty_not_covered') return 'Regreso fuera de cobertura';
  if (event.action === 'warranty_pending') return 'Cobertura pendiente de evaluación';
  return 'Orden actualizada';
}

async function openOrderDetail(id) {
  try {
    const order = await api(`/api/orders/${id}`);
    state.selectedOrder = order;
    renderOrderDetail(order);
    setLayer($('#detail-drawer-layer'), true);
  } catch (error) {
    showToast(error.message, 'error');
  }
}

function clearFormErrors(form) {
  $$('.field-error', form).forEach((node) => { node.textContent = ''; });
  $$('.field.has-error', form).forEach((node) => node.classList.remove('has-error'));
}

function setFormErrors(form, fields = {}) {
  clearFormErrors(form);
  Object.entries(fields).forEach(([name, message]) => {
    const error = $(`[data-error-for="${name}"]`, form);
    if (error) {
      error.textContent = message;
      error.closest('.field')?.classList.add('has-error');
    }
  });
}

function syncAssetMode() {
  const form = $('#order-form');
  const mode = $('[name="assetMode"]:checked', form)?.value || 'new';
  const incomingFields = $('#incoming-asset-fields');
  const existingFields = $('#existing-asset-fields');
  incomingFields.hidden = mode !== 'new';
  existingFields.hidden = mode !== 'existing';
  $$('input, select, textarea', incomingFields).forEach((control) => { control.disabled = mode !== 'new'; });
  $$('input, select, textarea', existingFields).forEach((control) => { control.disabled = mode !== 'existing'; });
  $$('.mode-choice', form).forEach((choice) => {
    choice.classList.toggle('is-selected', $('input', choice).checked);
  });
}

function syncCustomerMode() {
  const form = $('#order-form');
  const mode = $('[name="customerMode"]:checked', form)?.value || 'new';
  const incomingFields = $('#incoming-customer-fields');
  const existingFields = $('#existing-customer-fields');
  incomingFields.hidden = mode !== 'new';
  existingFields.hidden = mode !== 'existing';
  $$('input, select, textarea', incomingFields).forEach((control) => { control.disabled = mode !== 'new'; });
  $$('input, select, textarea', existingFields).forEach((control) => { control.disabled = mode !== 'existing'; });
  $$('.customer-mode .mode-choice', form).forEach((choice) => {
    choice.classList.toggle('is-selected', $('input', choice).checked);
  });
}

async function refreshAll() {
  const params = currentOrderParams();
  const [dashboard, ordersResponse, assetsResponse, customersResponse, inventoryResponse, reportsResponse, receiptsResponse, movementsResponse, salesResponse] = await Promise.all([
    api('/api/dashboard'),
    api(`/api/orders?${params}`),
    api('/api/assets'),
    api('/api/customers'),
    api('/api/inventory'),
    api('/api/purchase-reports'),
    api('/api/purchase-receipts'),
    api('/api/inventory-movements'),
    api('/api/sales')
  ]);
  state.dashboard = dashboard;
  state.orders = ordersResponse.orders;
  state.assets = assetsResponse.assets;
  state.customers = customersResponse.customers;
  state.inventory = inventoryResponse.items;
  state.purchaseReports = reportsResponse.reports;
  state.purchaseReceipts = receiptsResponse.receipts;
  state.inventoryMovements = movementsResponse.movements;
  state.sales = salesResponse.sales;
  renderDashboard();
  renderOrders();
  renderAssets();
  renderCustomers();
  renderInventory();
  renderPurchaseReports();
  renderInventoryMovements();
  renderSales();
  applyPermissions();
}

function currentOrderParams() {
  return new URLSearchParams({
    search: $('#order-search').value.trim(),
    status: $('#status-filter').value,
    priority: $('#priority-filter').value
  });
}

async function refreshOrdersWithFilters() {
  const params = currentOrderParams();
  try {
    const response = await api(`/api/orders?${params}`);
    state.orders = response.orders;
    renderOrders();
  } catch (error) {
    showToast(error.message, 'error');
  }
}

$('#login-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const button = $('button[type="submit"]', form);
  $('#login-error').textContent = '';
  button.disabled = true;
  try {
    const user = await api('/api/auth/login', {
      method: 'POST',
      body: JSON.stringify({ username: form.elements.username.value, password: form.elements.password.value })
    });
    form.reset();
    await beginSession(user);
  } catch (error) {
    $('#login-error').textContent = error.message;
  } finally {
    button.disabled = false;
  }
});

$('#setup-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const button = $('button[type="submit"]', form);
  $('#setup-error').textContent = '';
  button.disabled = true;
  try {
    const user = await api('/api/auth/setup', {
      method: 'POST',
      body: JSON.stringify({ displayName: form.elements.displayName.value, username: form.elements.username.value, password: form.elements.password.value, role: 'admin' })
    });
    form.reset();
    await beginSession(user);
  } catch (error) {
    $('#setup-error').textContent = error.message;
  } finally {
    button.disabled = false;
  }
});

$('#user-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  clearFormErrors(form);
  const button = $('.submit-button', form);
  button.disabled = true;
  try {
    await api('/api/users', {
      method: 'POST',
      body: JSON.stringify(Object.fromEntries(new FormData(form)))
    });
    closeUserDrawer();
    await refreshUsers();
    showToast('Usuario local creado.');
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    button.disabled = false;
  }
});

$('#sync-settings-form').addEventListener('input', (event) => {
  event.currentTarget.dataset.dirty = 'true';
});

$('#sync-settings-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const button = $('button[type="submit"]', form);
  button.disabled = true;
  button.textContent = 'Verificando…';
  try {
    state.syncStatus = await api('/api/sync/config', {
      method: 'POST',
      body: JSON.stringify(Object.fromEntries(new FormData(form)))
    });
    form.dataset.dirty = 'false';
    renderSyncStatus();
    startSynchronization();
    await runSynchronization({ quiet: false });
  } catch (error) {
    showToast(error.message, 'error');
  } finally {
    button.disabled = false;
    button.textContent = 'Verificar y guardar';
  }
});

$('#order-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const formData = new FormData(form);
  const { orderId, values } = buildOrderPayload(formData);
  submit.disabled = true;
  submit.textContent = orderId ? 'Guardando…' : 'Creando…';
  clearFormErrors(form);
  try {
    const order = await api(orderId ? `/api/orders/${orderId}` : '/api/orders', {
      method: orderId ? 'PATCH' : 'POST',
      body: JSON.stringify(values)
    });
    form.reset();
    syncAssetMode();
    syncCustomerMode();
    closeOrderDrawer(false);
    await refreshAll();
    showToast(orderId ? `${order.orderNumber} actualizada correctamente.` : `${order.orderNumber} creada correctamente.`);
    await openOrderDetail(order.id);
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = orderId ? 'Guardar cambios' : 'Crear orden';
  }
});

$('#quote-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const button = $('.quote-save-button', event.currentTarget);
  button.disabled = true;
  button.textContent = 'Guardando…';
  await saveDiagnosisQuote();
  if (button.isConnected) {
    button.disabled = false;
    button.textContent = 'Guardar borrador';
  }
});

$('#resources-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const button = $('.resources-save-button', event.currentTarget);
  button.disabled = true;
  button.textContent = 'Guardando…';
  await saveOrderResources();
  if (button.isConnected) {
    button.disabled = false;
    button.textContent = 'Guardar consumos';
  }
});

$('#payment-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const orderId = Number(form.elements.orderId.value);
  const input = {
    paymentType: form.elements.paymentType.value,
    method: form.elements.method.value,
    amount: form.elements.amount.value,
    reference: form.elements.reference.value,
    receivedBy: form.elements.receivedBy.value,
    note: form.elements.note.value
  };
  submit.disabled = true;
  submit.textContent = 'Guardando…';
  clearFormErrors(form);
  try {
    const payment = await api(`/api/orders/${orderId}/payments`, {
      method: 'POST',
      body: JSON.stringify(input)
    });
    const order = await api(`/api/orders/${orderId}`);
    state.selectedOrder = order;
    closePaymentDrawer(false);
    await refreshAll();
    showToast(`${payment.receiptNumber} registrado correctamente.`);
    printDocument('payment', { order, payment });
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = 'Guardar e imprimir recibo';
  }
});

$('#warranty-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const orderId = Number(form.elements.orderId.value);
  submit.disabled = true;
  submit.textContent = 'Guardando…';
  clearFormErrors(form);
  try {
    const warranty = await api(`/api/orders/${orderId}/warranty`, {
      method: 'PUT',
      body: JSON.stringify({
        startsAt: form.elements.startsAt.value,
        durationDays: form.elements.durationDays.value,
        coverage: form.elements.coverage.value,
        exclusions: form.elements.exclusions.value
      })
    });
    const order = await api(`/api/orders/${orderId}`);
    state.selectedOrder = order;
    closeWarrantyDrawer(false);
    renderOrderDetail(order);
    setLayer($('#detail-drawer-layer'), true);
    await refreshAll();
    showToast(`${warranty.warrantyNumber} guardada correctamente.`);
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = 'Guardar garantía';
  }
});

$('#warranty-return-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const originalOrderId = Number(form.elements.originalOrderId.value);
  const formData = new FormData(form);
  const accessories = [
    ...formData.getAll('returnAccessories'),
    ...(form.elements.otherReturnAccessories.value || '').split('\n').map((item) => item.trim()).filter(Boolean)
  ];
  submit.disabled = true;
  submit.textContent = 'Creando…';
  clearFormErrors(form);
  try {
    const order = await api(`/api/orders/${originalOrderId}/warranty-return`, {
      method: 'POST',
      body: JSON.stringify({
        reportedIssue: form.elements.reportedIssue.value,
        receivedCondition: form.elements.receivedCondition.value,
        priority: form.elements.priority.value,
        accessories
      })
    });
    state.selectedOrder = order;
    closeWarrantyReturnDrawer(false);
    await refreshAll();
    showToast(`${order.warrantyClaim.claimNumber} creada como ${order.orderNumber}.`);
    printDocument('reception', order);
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = 'Crear orden e imprimir recepción';
  }
});

$('#customer-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const values = Object.fromEntries(new FormData(form).entries());
  const id = Number(values.id) || null;
  delete values.id;
  submit.disabled = true;
  submit.textContent = id ? 'Guardando…' : 'Agregando…';
  clearFormErrors(form);
  try {
    const customer = await api(id ? `/api/customers/${id}` : '/api/customers', {
      method: id ? 'PATCH' : 'POST',
      body: JSON.stringify(values)
    });
    closeCustomerDrawer();
    await refreshAll();
    showToast(`${customer.name} ${id ? 'actualizado' : 'agregado'} correctamente.`);
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = id ? 'Guardar cambios' : 'Guardar cliente';
  }
});

$('#inventory-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const values = Object.fromEntries(new FormData(form).entries());
  const id = Number(values.id) || null;
  delete values.id;
  submit.disabled = true;
  submit.textContent = id ? 'Guardando…' : 'Agregando…';
  clearFormErrors(form);
  try {
    const item = await api(id ? `/api/inventory/${id}` : '/api/inventory', {
      method: id ? 'PATCH' : 'POST',
      body: JSON.stringify(values)
    });
    closeInventoryDrawer();
    await refreshAll();
    showToast(`${item.name} ${id ? 'actualizado' : 'agregado'} correctamente.`);
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = id ? 'Guardar cambios' : 'Guardar producto';
  }
});

$('#purchase-report-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const lines = $$('.purchase-line', form)
    .filter((row) => $('.purchase-select', row).checked)
    .map((row) => ({
      itemId: row.dataset.purchaseItemId,
      purchaseQuantity: $('.purchase-quantity', row).value,
      unitCost: $('.purchase-cost', row).value
    }));
  const formData = new FormData(form);
  const input = {
    supplier: formData.get('supplier') || '',
    notes: formData.get('notes') || '',
    lines
  };
  submit.disabled = true;
  submit.textContent = 'Generando…';
  clearFormErrors(form);
  try {
    const report = await api('/api/purchase-reports', {
      method: 'POST',
      body: JSON.stringify(input)
    });
    closePurchaseReport();
    await refreshAll();
    showToast(`${report.reportNumber} guardado correctamente.`);
    printDocument('purchase', report);
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = 'Guardar e imprimir';
  }
});

$('#purchase-receipt-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const lines = $$('.purchase-line', form)
    .filter((row) => $('.receipt-select', row).checked)
    .map((row) => ({
      purchaseReportLineId: row.dataset.reportLineId,
      receivedQuantity: $('.receipt-quantity', row).value
    }));
  submit.disabled = true;
  submit.textContent = 'Registrando…';
  clearFormErrors(form);
  try {
    const receipt = await api(`/api/purchase-reports/${form.elements.reportId.value}/receipts`, {
      method: 'POST',
      body: JSON.stringify({ notes: form.elements.notes.value, lines })
    });
    closePurchaseReceipt();
    await refreshAll();
    showToast(`${receipt.receiptNumber} registrada. Las existencias ya fueron actualizadas.`);
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = 'Registrar entrada';
  }
});

$('#sale-form').addEventListener('submit', async (event) => {
  event.preventDefault();
  const form = event.currentTarget;
  const submit = $('.submit-button', form);
  const lines = $$('.sale-line', form).map((row) => ({
    inventoryItemId: $('.sale-item-select', row).value,
    quantity: $('.sale-quantity', row).value,
    unitPrice: $('.sale-unit-price', row).value
  }));
  submit.disabled = true;
  submit.textContent = 'Guardando…';
  clearFormErrors(form);
  try {
    const sale = await api('/api/sales', {
      method: 'POST',
      body: JSON.stringify({
        customerName: form.elements.customerName.value,
        notes: form.elements.notes.value,
        discount: form.elements.discount.value,
        lines
      })
    });
    closeSaleDrawer();
    await refreshAll();
    showToast(`${sale.saleNumber} registrada. El inventario fue actualizado.`);
    printDocument('sale', sale);
  } catch (error) {
    setFormErrors(form, error.fields);
    showToast(error.message, 'error');
  } finally {
    submit.disabled = false;
    submit.textContent = 'Guardar e imprimir';
  }
});

document.addEventListener('change', async (event) => {
  if (event.target.matches('.purchase-select')) {
    const row = event.target.closest('.purchase-line');
    const selected = event.target.checked;
    $$('.purchase-quantity, .purchase-cost', row).forEach((input) => { input.disabled = !selected; });
    updatePurchaseReportTotal();
    if (selected) $('.purchase-cost', row).focus();
  }

  if (event.target.matches('.receipt-select')) updatePurchaseReceiptTotal();
  if (event.target.matches('.sale-item-select')) updateSaleTotals();

  if (event.target.matches('[name="assetMode"]')) {
    syncAssetMode();
  }
  if (event.target.matches('[name="customerMode"]')) {
    syncCustomerMode();
  }

  if (event.target.id === 'detail-status-select' && state.selectedOrder) {
    const nextStatus = event.target.value;
    event.target.disabled = true;
    try {
      if (nextStatus === 'completada' || nextStatus === 'entregada') {
        const saved = await saveDeliveryDetails(nextStatus === 'entregada');
        if (!saved) {
          renderOrderDetail(state.selectedOrder);
          return;
        }
      }
      const order = await api(`/api/orders/${state.selectedOrder.id}`, {
        method: 'PATCH', body: JSON.stringify({ status: nextStatus })
      });
      state.selectedOrder = order;
      renderOrderDetail(order);
      await refreshAll();
      showToast('Estado actualizado.');
    } catch (error) {
      showToast(error.message, 'error');
      renderOrderDetail(state.selectedOrder);
    }
  }

  if (event.target.matches('[data-task-id]') && state.selectedOrder) {
    const taskId = Number(event.target.dataset.taskId);
    event.target.disabled = true;
    try {
      const order = await api(`/api/orders/${state.selectedOrder.id}/tasks/${taskId}`, {
        method: 'PATCH', body: JSON.stringify({ completed: event.target.checked })
      });
      state.selectedOrder = order;
      renderOrderDetail(order);
      await refreshAll();
    } catch (error) {
      showToast(error.message, 'error');
      event.target.checked = !event.target.checked;
      event.target.disabled = false;
    }
  }
});

document.addEventListener('click', async (event) => {
  const viewTrigger = event.target.closest('[data-view]');
  if (viewTrigger) switchView(viewTrigger.dataset.view);

  const action = event.target.closest('[data-action]')?.dataset.action;
  if (action === 'logout') {
    if (!window.confirm('¿Cerrar la sesión actual?')) return;
    try { await api('/api/auth/logout', { method: 'POST' }); } catch (error) { console.error(error); }
    releasePhotoUrls();
    showAuthentication(false);
    $('#login-form').reset();
    return;
  }
  if (action === 'sync-now') {
    await runSynchronization({ quiet: false });
    return;
  }
  if (action === 'new-user') openUserDrawer();
  if (action === 'close-user') closeUserDrawer();
  if (action === 'toggle-user') {
    const id = Number(event.target.closest('[data-user-id]').dataset.userId);
    const user = state.users.find((item) => item.id === id);
    if (user && window.confirm(`${user.active ? 'Desactivar' : 'Activar'} la cuenta de ${user.displayName}?`)) {
      try {
        await api(`/api/users/${id}/active`, { method: 'PATCH', body: JSON.stringify({ active: !user.active }) });
        await refreshUsers();
        showToast(`Cuenta ${user.active ? 'desactivada' : 'activada'}.`);
      } catch (error) { showToast(error.message, 'error'); }
    }
  }
  if (action === 'reset-user-password') {
    const id = Number(event.target.closest('[data-user-id]').dataset.userId);
    const user = state.users.find((item) => item.id === id);
    const password = window.prompt(`Nueva contraseña temporal para ${user?.displayName || 'el usuario'} (mínimo 8 caracteres):`);
    if (password === null) return;
    try {
      await api(`/api/users/${id}/password`, { method: 'PATCH', body: JSON.stringify({ password }) });
      await refreshUsers();
      showToast('Contraseña actualizada.');
    } catch (error) { showToast(error.message, 'error'); }
  }
  if (action === 'print-qr-label' && state.selectedOrder) printDocument('qr', state.selectedOrder);
  if (action === 'upload-photo' && state.selectedOrder) {
    const button = event.target.closest('button');
    const file = $('#order-photo-file')?.files?.[0];
    if (!file) { showToast('Selecciona una fotografía.', 'error'); return; }
    button.disabled = true;
    button.textContent = 'Procesando...';
    try {
      const image = await loadPhotoBytes(file);
      await api(`/api/orders/${state.selectedOrder.id}/photos`, {
        method: 'POST',
        body: JSON.stringify({ fileName: file.name, mimeType: image.mimeType, category: $('#order-photo-category').value, caption: $('#order-photo-caption').value, data: image.data })
      });
      const order = await api(`/api/orders/${state.selectedOrder.id}`);
      state.selectedOrder = order;
      renderOrderDetail(order);
      showToast('Fotografía agregada a la orden.');
    } catch (error) {
      showToast(error.message, 'error');
      if (button.isConnected) { button.disabled = false; button.textContent = 'Agregar'; }
    }
  }
  if (action === 'delete-photo' && state.selectedOrder) {
    const photoId = Number(event.target.closest('[data-photo-id]').dataset.photoId);
    if (window.confirm('¿Eliminar esta fotografía de la orden?')) {
      try {
        await api(`/api/orders/${state.selectedOrder.id}/photos/${photoId}`, { method: 'DELETE' });
        const order = await api(`/api/orders/${state.selectedOrder.id}`);
        state.selectedOrder = order;
        renderOrderDetail(order);
        showToast('Fotografía eliminada.');
      } catch (error) { showToast(error.message, 'error'); }
    }
  }
  if (action === 'new-order') openOrderDrawer();
  if (action === 'close-order-drawer') closeOrderDrawer();
  if (action === 'edit-order' && state.selectedOrder) {
    const order = state.selectedOrder;
    setLayer($('#detail-drawer-layer'), false);
    openOrderDrawer(order);
  }
  if (action === 'open-quote' && state.selectedOrder) openQuoteDrawer(state.selectedOrder);
  if (action === 'close-quote') closeQuoteDrawer();
  if (action === 'open-resources' && state.selectedOrder) openResourcesDrawer(state.selectedOrder);
  if (action === 'close-resources') closeResourcesDrawer();
  if (action === 'open-payment' && state.selectedOrder) openPaymentDrawer(state.selectedOrder);
  if (action === 'close-payment') closePaymentDrawer();
  if (action === 'edit-warranty' && state.selectedOrder) openWarrantyDrawer(state.selectedOrder);
  if (action === 'close-warranty') closeWarrantyDrawer();
  if (action === 'new-warranty-return' && state.selectedOrder?.warranty) openWarrantyReturnDrawer(state.selectedOrder);
  if (action === 'close-warranty-return') closeWarrantyReturnDrawer();
  if (action === 'open-linked-order') {
    const orderId = Number(event.target.closest('[data-order-id]')?.dataset.orderId);
    if (orderId) {
      closeDetail();
      await openOrderDetail(orderId);
    }
  }
  if (action === 'open-order') {
    const orderId = Number(event.target.closest('[data-order-id]')?.dataset.orderId);
    if (orderId) await openOrderDetail(orderId);
  }
  if (action === 'save-warranty-decision' && state.selectedOrder?.warrantyClaim) {
    const button = event.target.closest('button');
    button.disabled = true;
    try {
      const order = await api(`/api/orders/${state.selectedOrder.id}/warranty-claim`, {
        method: 'PUT',
        body: JSON.stringify({
          decision: $('#warranty-decision').value,
          resolution: $('#warranty-resolution').value
        })
      });
      state.selectedOrder = order;
      renderOrderDetail(order);
      await refreshAll();
      showToast('Decisión de garantía guardada.');
    } catch (error) {
      showToast(error.message, 'error');
      button.disabled = false;
    }
  }
  if (action === 'print-payment' && state.selectedOrder) {
    const paymentId = Number(event.target.closest('[data-payment-id]')?.dataset.paymentId);
    const payment = state.selectedOrder.payments?.find((candidate) => candidate.id === paymentId);
    if (payment) printDocument('payment', { order: state.selectedOrder, payment });
  }
  if (action === 'cancel-payment' && state.selectedOrder) {
    const paymentId = Number(event.target.closest('[data-payment-id]')?.dataset.paymentId);
    const payment = state.selectedOrder.payments?.find((candidate) => candidate.id === paymentId);
    if (payment && window.confirm(`¿Anular el recibo ${payment.receiptNumber}? El monto dejará de contar como pagado.`)) {
      const note = window.prompt('Motivo de la anulación:', '');
      if (note === null) return;
      if (note.trim().length < 3) {
        showToast('Indica un motivo de al menos 3 caracteres.', 'error');
        return;
      }
      try {
        await api(`/api/orders/${state.selectedOrder.id}/payments/${paymentId}/cancel`, {
          method: 'POST', body: JSON.stringify({ note })
        });
        const order = await api(`/api/orders/${state.selectedOrder.id}`);
        state.selectedOrder = order;
        renderOrderDetail(order);
        await refreshAll();
        showToast(`${payment.receiptNumber} anulado. El saldo fue recalculado.`);
      } catch (error) {
        showToast(error.message, 'error');
      }
    }
  }
  if (action === 'add-order-part') {
    $('#order-parts').insertAdjacentHTML('beforeend', orderPartTemplate());
    updateResourceTotals();
    $$('.resource-part-select', $('#order-parts')).at(-1)?.focus();
  }
  if (action === 'remove-order-part') {
    event.target.closest('.resource-part-line')?.remove();
    updateResourceTotals();
  }
  if (action === 'add-order-labor') {
    $('#order-labor').insertAdjacentHTML('beforeend', orderLaborTemplate());
    updateResourceTotals();
    $$('.resource-labor-description', $('#order-labor')).at(-1)?.focus();
  }
  if (action === 'remove-order-labor') {
    event.target.closest('.resource-labor-line')?.remove();
    updateResourceTotals();
  }
  if (action === 'add-quote-item') {
    $('#quote-items').insertAdjacentHTML('beforeend', quoteItemTemplate());
    updateQuoteTotals();
    $$('.quote-description', $('#quote-items')).at(-1)?.focus();
  }
  if (action === 'remove-quote-item') {
    event.target.closest('.quote-line')?.remove();
    if (!$('#quote-items').children.length) $('#quote-items').innerHTML = quoteItemTemplate();
    updateQuoteTotals();
  }
  if (action === 'request-quote-authorization') {
    const button = event.target.closest('button');
    button.disabled = true;
    await decideQuote('pending');
    if (button.isConnected) button.disabled = false;
  }
  if (action === 'revise-quote') {
    const button = event.target.closest('button');
    button.disabled = true;
    const order = await saveDiagnosisQuote(false);
    if (order) showToast('La solicitud se retiró. Ya puedes editar la cotización.');
    if (button.isConnected && state.selectedOrder?.quoteStatus === 'pending') button.disabled = false;
  }
  if (action === 'approve-quote' || action === 'reject-quote') {
    const button = event.target.closest('button');
    button.disabled = true;
    await decideQuote(action === 'approve-quote' ? 'approved' : 'rejected');
    if (button.isConnected && state.selectedOrder?.quoteStatus === 'pending') button.disabled = false;
  }
  if (action === 'print-quote' && state.selectedOrder) {
    if (!state.selectedOrder.diagnosis || !state.selectedOrder.quoteItems?.length) {
      showToast('Guarda el diagnóstico y los conceptos antes de imprimir.', 'error');
    } else {
      printDocument('quote', state.selectedOrder);
    }
  }
  if (action === 'new-customer') openCustomerDrawer();
  if (action === 'edit-customer') {
    const id = Number(event.target.closest('[data-customer-id]')?.dataset.customerId);
    const customer = state.customers.find((candidate) => candidate.id === id);
    if (customer) openCustomerDrawer(customer);
  }
  if (action === 'customer-orders') {
    const id = Number(event.target.closest('[data-customer-id]')?.dataset.customerId);
    const customer = state.customers.find((candidate) => candidate.id === id);
    if (customer) {
      $('#order-search').value = customer.name;
      switchView('orders');
      await refreshOrdersWithFilters();
    }
  }
  if (action === 'close-customer-drawer') closeCustomerDrawer();
  if (action === 'new-inventory-item') openInventoryDrawer();
  if (action === 'edit-inventory-item') {
    const id = Number(event.target.closest('[data-inventory-id]')?.dataset.inventoryId);
    const item = state.inventory.find((candidate) => candidate.id === id);
    if (item) openInventoryDrawer(item);
  }
  if (action === 'close-inventory-drawer') closeInventoryDrawer();
  if (action === 'new-purchase-report') openPurchaseReport();
  if (action === 'close-purchase-report') closePurchaseReport();
  if (action === 'receive-purchase') {
    const id = Number(event.target.closest('[data-report-id]')?.dataset.reportId);
    const report = state.purchaseReports.find((candidate) => candidate.id === id);
    if (report) openPurchaseReceipt(report);
  }
  if (action === 'close-purchase-receipt') closePurchaseReceipt();
  if (action === 'print-purchase-report') {
    const id = Number(event.target.closest('[data-report-id]')?.dataset.reportId);
    const report = state.purchaseReports.find((candidate) => candidate.id === id);
    if (report) printDocument('purchase', report);
  }
  if (action === 'new-sale') openSaleDrawer();
  if (action === 'close-sale') closeSaleDrawer();
  if (action === 'add-sale-line') {
    $('#sale-lines').insertAdjacentHTML('beforeend', saleLineTemplate());
    updateSaleTotals();
  }
  if (action === 'remove-sale-line') {
    event.target.closest('.sale-line')?.remove();
    if (!$('#sale-lines').children.length) $('#sale-lines').innerHTML = saleLineTemplate();
    updateSaleTotals();
  }
  if (action === 'print-sale') {
    const id = Number(event.target.closest('[data-sale-id]')?.dataset.saleId);
    const sale = state.sales.find((candidate) => candidate.id === id);
    if (sale) printDocument('sale', sale);
  }
  if (action === 'cancel-sale') {
    const id = Number(event.target.closest('[data-sale-id]')?.dataset.saleId);
    const sale = state.sales.find((candidate) => candidate.id === id);
    if (sale && window.confirm(`¿Anular ${sale.saleNumber}? Las unidades vendidas volverán al inventario.`)) {
      const note = window.prompt('Motivo de la anulación (opcional):', '');
      if (note === null) return;
      try {
        await api(`/api/sales/${id}/cancel`, { method: 'POST', body: JSON.stringify({ note }) });
        await refreshAll();
        showToast(`${sale.saleNumber} anulada. Las existencias fueron restituidas.`);
      } catch (error) {
        showToast(error.message, 'error');
      }
    }
  }
  if (action === 'close-detail') closeDetail();
  if (action === 'print-reception' && state.selectedOrder) {
    printDocument('reception', state.selectedOrder);
  }
  if (action === 'save-delivery') {
    const button = event.target.closest('button');
    button.disabled = true;
    const order = await saveDeliveryDetails();
    if (order) showToast('Datos de entrega guardados.');
    else button.disabled = false;
    if (button.isConnected) button.disabled = false;
  }
  if (action === 'print-delivery') {
    const button = event.target.closest('button');
    button.disabled = true;
    const order = await saveDeliveryDetails(true);
    if (order) printDocument('delivery', order);
    else button.disabled = false;
    if (button.isConnected) button.disabled = false;
  }
  if (action === 'refresh-backups') {
    const button = event.target.closest('button');
    button.disabled = true;
    try {
      await refreshBackupCenter();
      showToast('Lista de respaldos actualizada.');
    } catch (error) {
      showToast(error.message, 'error');
    } finally {
      button.disabled = false;
    }
  }
  if (action === 'create-manual-backup') {
    const button = event.target.closest('button');
    button.disabled = true;
    button.textContent = 'Creando copia…';
    try {
      const backup = await api('/api/backups/manual', { method: 'POST' });
      await refreshBackupCenter();
      showToast(`Respaldo verificado: ${backup.fileName}`);
    } catch (error) {
      showToast(error.message, 'error');
    } finally {
      button.disabled = false;
      button.innerHTML = '<span aria-hidden="true">＋</span> Crear respaldo ahora';
    }
  }
  if (action === 'restore-backup') {
    const button = event.target.closest('button');
    const fileName = button.dataset.backupFile;
    const backup = state.backupOverview?.backups?.find((item) => item.fileName === fileName);
    if (!backup?.valid) return;
    const confirmed = window.confirm(
      `¿Restaurar ${fileName}?\n\nLa información actual será reemplazada por esta copia (${backup.workOrderCount} órdenes, ${backup.customerCount} clientes y ${backup.inventoryItemCount} productos).\n\nAntes de hacerlo, Geecko guardará una copia de emergencia de los datos actuales.`
    );
    if (!confirmed) return;
    button.disabled = true;
    button.textContent = 'Restaurando…';
    try {
      const result = await api('/api/backups/restore', {
        method: 'POST',
        body: JSON.stringify({ fileName, confirmed: true })
      });
      state.selectedOrder = null;
      releasePhotoUrls();
      try { await api('/api/auth/logout', { method: 'POST' }); } catch (logoutError) { console.error(logoutError); }
      showAuthentication(false);
      await bootstrapApplication();
      showToast(`Datos restaurados. Copia preventiva: ${result.safetyBackupFileName}`);
    } catch (error) {
      showToast(error.message, 'error');
      button.disabled = false;
      button.textContent = 'Restaurar';
    }
  }
  if (action === 'export-backup') {
    tauriInvoke('export_backup')
      .then((path) => showToast(`Exportación JSON guardada en ${path}`))
      .catch((reason) => showToast(reason?.error || reason || 'No fue posible crear la exportación.', 'error'));
  }

  const row = event.target.closest('tr[data-order-id]');
  if (row) openOrderDetail(Number(row.dataset.orderId));
});

document.addEventListener('keydown', (event) => {
  if (event.key === 'Escape') {
    const openLayer = $$('.drawer-layer.is-open').at(-1);
    if (openLayer && $('form', openLayer)) {
      event.preventDefault();
      showToast('Usa Cancelar o la X para cerrar el formulario.', 'error');
      return;
    }
    if (openLayer?.id === 'detail-drawer-layer') closeDetail();
  }
  if ((event.key === 'Enter' || event.key === ' ') && event.target.matches('tr[data-order-id]')) {
    event.preventDefault();
    openOrderDetail(Number(event.target.dataset.orderId));
  }
});

let searchTimer;
$('#order-search').addEventListener('input', () => {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(refreshOrdersWithFilters, 220);
});
$('#status-filter').addEventListener('change', refreshOrdersWithFilters);
$('#priority-filter').addEventListener('change', refreshOrdersWithFilters);
$('#statistics-range').addEventListener('change', () => refreshStatistics().catch((error) => showToast(error.message, 'error')));
$('#customer-search').addEventListener('input', renderCustomers);
$('#inventory-search').addEventListener('input', renderInventory);
$('#purchase-report-form').addEventListener('input', (event) => {
  if (event.target.matches('.purchase-quantity, .purchase-cost')) updatePurchaseReportTotal();
});
$('#purchase-receipt-form').addEventListener('input', (event) => {
  if (event.target.matches('.receipt-quantity')) updatePurchaseReceiptTotal();
});
$('#sale-form').addEventListener('input', (event) => {
  if (event.target.matches('.sale-quantity, .sale-unit-price, [name="discount"]')) updateSaleTotals();
});
$('#quote-form').addEventListener('input', (event) => {
  if (event.target.matches('.quote-quantity, .quote-unit-price, [name="discount"]')) updateQuoteTotals();
});
$('#resources-form').addEventListener('input', (event) => {
  if (event.target.matches('.resource-part-quantity, .resource-part-price, .resource-labor-hours, .resource-labor-rate')) {
    updateResourceTotals();
  }
});
syncAssetMode();
syncCustomerMode();

$('#menu-button').addEventListener('click', () => {
  $('#sidebar').classList.add('is-open');
  $('#mobile-overlay').classList.add('is-open');
});
$('#mobile-overlay').addEventListener('click', () => {
  $('#sidebar').classList.remove('is-open');
  $('#mobile-overlay').classList.remove('is-open');
});

window.addEventListener('afterprint', () => {
  document.body.classList.remove('is-printing');
  $('#print-root').innerHTML = '';
  $('#print-root').setAttribute('aria-hidden', 'true');
});

window.addEventListener('online', () => scheduleSynchronization(100));
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') scheduleSynchronization(100);
});

const today = new Intl.DateTimeFormat('es-GT', { weekday: 'short', day: 'numeric', month: 'short' }).format(new Date()).replaceAll('.', '');
$('#today-label').textContent = today.charAt(0).toUpperCase() + today.slice(1);

bootstrapApplication();

setInterval(() => {
  if (!state.session) return;
  refreshBackupCenter(true).catch((error) => console.error('Respaldo automático:', error));
}, 60 * 60 * 1000);
