import assert from 'node:assert/strict';
import test from 'node:test';

import { buildOrderPayload } from '../public/order-payload.js';

class FormDataStub {
  constructor(entries) {
    this.items = entries;
  }

  entries() {
    return this.items[Symbol.iterator]();
  }

  getAll(name) {
    return this.items.filter(([key]) => key === name).map(([, value]) => value);
  }
}

test('conserva los modos y los identificadores al crear con registros existentes', () => {
  const formData = new FormDataStub([
    ['orderId', ''],
    ['title', 'Laptop no enciende'],
    ['description', 'Se reinicia después del POST'],
    ['assetMode', 'existing'],
    ['assetId', '1'],
    ['customerMode', 'existing'],
    ['customerId', '7'],
    ['tasks', 'Diagnosticar encendido'],
    ['accessories', 'Cargador']
  ]);

  const { orderId, values } = buildOrderPayload(formData);

  assert.equal(orderId, null);
  assert.equal(values.assetMode, 'existing');
  assert.equal(values.assetId, '1');
  assert.equal(values.customerMode, 'existing');
  assert.equal(values.customerId, '7');
  assert.deepEqual(values.tasks, ['Diagnosticar encendido']);
  assert.deepEqual(values.accessories, ['Cargador']);
});

test('construye los registros nuevos y conserva sus modos al crear', () => {
  const formData = new FormDataStub([
    ['orderId', ''],
    ['assetMode', 'new'],
    ['assetId', '99'],
    ['assetName', 'Laptop HP'],
    ['assetCategory', 'portatil'],
    ['customerMode', 'new'],
    ['customerId', '88'],
    ['customerName', 'Cliente de prueba']
  ]);

  const { values } = buildOrderPayload(formData);

  assert.equal(values.assetMode, 'new');
  assert.equal(values.assetId, '');
  assert.deepEqual(values.incomingAsset, {
    code: '',
    name: 'Laptop HP',
    category: 'portatil',
    manufacturer: '',
    model: '',
    serialNumber: ''
  });
  assert.equal(values.customerMode, 'new');
  assert.equal(values.customerId, '');
  assert.equal(values.requestedBy, 'Cliente de prueba');
  assert.equal(values.incomingCustomer.name, 'Cliente de prueba');
});

test('omite los modos internos al editar una orden', () => {
  const formData = new FormDataStub([
    ['orderId', '12'],
    ['assetMode', 'existing'],
    ['customerMode', 'existing'],
    ['assetId', '1'],
    ['customerId', '7']
  ]);

  const { orderId, values } = buildOrderPayload(formData);

  assert.equal(orderId, 12);
  assert.equal('assetMode' in values, false);
  assert.equal('customerMode' in values, false);
});
