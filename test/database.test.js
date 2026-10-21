import assert from 'node:assert/strict';
import { describe, it } from 'node:test';
import { createStore } from '../src/database.js';
import { ValidationError, validateAsset, validateWorkOrder } from '../src/domain.js';

function orderInput(overrides = {}) {
  return validateWorkOrder({
    title: 'Inspeccionar motor principal',
    description: 'Revisar vibración, temperatura y estado de rodamientos.',
    type: 'preventivo',
    priority: 'alta',
    status: 'solicitada',
    assetId: '',
    location: 'Planta 1',
    assignedTo: 'Equipo técnico',
    requestedBy: 'Producción',
    scheduledDate: '2026-10-20',
    estimatedHours: '2.5',
    safetyNotes: 'Aplicar bloqueo y etiquetado.',
    tasks: ['Desenergizar', 'Medir vibración'],
    ...overrides
  });
}

describe('Geecko Maintenance store', () => {
  it('crea equipos y evita códigos duplicados', () => {
    const store = createStore(':memory:');
    const input = validateAsset({
      code: 'eq-001',
      name: 'Motor principal',
      category: 'Motores',
      location: 'Planta 1',
      status: 'operativo'
    });

    const asset = store.createAsset(input);
    assert.equal(asset.code, 'EQ-001');
    assert.equal(store.listAssets().length, 1);
    assert.throws(() => store.createAsset(input), /Ya existe/);
    store.close();
  });

  it('genera consecutivo, tareas e historial de la orden', () => {
    const store = createStore(':memory:');
    const order = store.createOrder(orderInput());

    assert.match(order.orderNumber, /^GM-\d{4}-0001$/);
    assert.equal(order.taskCount, 2);
    assert.equal(order.completedTaskCount, 0);
    assert.equal(order.events[0].action, 'created');

    const updated = store.setTaskCompleted(order.id, order.tasks[0].id, true);
    assert.equal(updated.completedTaskCount, 1);

    const completed = store.updateOrder(order.id, validateWorkOrder({ status: 'completada' }, { partial: true }));
    assert.equal(completed.status, 'completada');
    assert.ok(completed.completedAt);
    assert.equal(completed.events[0].action, 'status_changed');
    store.close();
  });

  it('filtra órdenes y calcula indicadores', () => {
    const store = createStore(':memory:');
    store.createOrder(orderInput({ priority: 'critica', title: 'Falla crítica en bomba' }));
    store.createOrder(orderInput({ priority: 'baja', title: 'Lubricación secundaria' }));

    assert.equal(store.listOrders({ priority: 'critica' }).length, 1);
    assert.equal(store.listOrders({ search: 'bomba' })[0].priority, 'critica');
    const dashboard = store.getDashboard();
    assert.equal(dashboard.totals.activeOrders, 2);
    assert.equal(dashboard.totals.criticalOpen, 1);
    assert.equal(dashboard.upcoming.length, 2);
    store.close();
  });
});

describe('Validación de dominio', () => {
  it('rechaza órdenes incompletas y enumeraciones desconocidas', () => {
    assert.throws(
      () => validateWorkOrder({ title: 'X', description: '', priority: 'urgente' }),
      (error) => error instanceof ValidationError && Boolean(error.fields.title) && Boolean(error.fields.priority)
    );
  });

  it('normaliza datos válidos sin aceptar fechas ambiguas', () => {
    const order = orderInput({ title: '  Orden válida  ', estimatedHours: '1.25' });
    assert.equal(order.title, 'Orden válida');
    assert.equal(order.estimatedHours, 1.25);
    assert.throws(
      () => validateWorkOrder({ scheduledDate: '20/10/2026' }, { partial: true }),
      ValidationError
    );
    assert.throws(
      () => validateWorkOrder({ scheduledDate: '2026-02-31' }, { partial: true }),
      ValidationError
    );
  });
});
