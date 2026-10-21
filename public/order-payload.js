export function buildOrderPayload(formData) {
  const values = Object.fromEntries(formData.entries());
  const orderId = Number(values.orderId) || null;
  delete values.orderId;

  values.tasks = (values.tasks || '')
    .split('\n')
    .map((item) => item.trim())
    .filter(Boolean);
  values.accessories = [
    ...formData.getAll('accessories'),
    ...(values.otherAccessories || '').split('\n').map((item) => item.trim()).filter(Boolean)
  ];

  if (!orderId && values.assetMode === 'new') {
    values.assetId = '';
    values.incomingAsset = {
      code: values.assetCode || '',
      name: values.assetName || '',
      category: values.assetCategory || '',
      manufacturer: values.assetManufacturer || '',
      model: values.assetModel || '',
      serialNumber: values.assetSerialNumber || ''
    };
  }

  if (!orderId && values.customerMode === 'new') {
    values.customerId = '';
    values.requestedBy = values.customerName || '';
    values.incomingCustomer = {
      name: values.customerName || '',
      phone: values.customerPhone || '',
      email: values.customerEmail || '',
      address: values.customerAddress || '',
      company: values.customerCompany || '',
      taxId: values.customerTaxId || '',
      notes: ''
    };
  }

  [
    'assetCode', 'assetName', 'assetCategory', 'assetManufacturer', 'assetModel', 'assetSerialNumber',
    'customerName', 'customerPhone', 'customerEmail', 'customerAddress', 'customerCompany', 'customerTaxId',
    'otherAccessories'
  ].forEach((key) => delete values[key]);

  // Los modos son necesarios al crear para que Rust sepa si debe usar los
  // identificadores seleccionados o validar los datos de un registro nuevo.
  if (orderId) {
    delete values.assetMode;
    delete values.customerMode;
  }

  return { orderId, values };
}
