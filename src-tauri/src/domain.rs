use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const ORDER_STATUSES: [&str; 8] = [
    "solicitada",
    "planificada",
    "pendiente_autorizacion",
    "en_progreso",
    "en_espera",
    "completada",
    "entregada",
    "cancelada",
];
const ORDER_PRIORITIES: [&str; 4] = ["critica", "alta", "media", "baja"];
const ORDER_TYPES: [&str; 5] = [
    "correctivo",
    "preventivo",
    "inspeccion",
    "mejora",
    "garantia",
];
const ASSET_STATUSES: [&str; 6] = [
    "operativo",
    "detenido",
    "mantenimiento",
    "listo",
    "entregado",
    "baja",
];
const DELIVERY_CONDITIONS: [&str; 5] = [
    "reparado",
    "funcionamiento_parcial",
    "sin_reparar",
    "falla_no_reproducida",
    "no_reparable",
];
const DELIVERY_TESTERS: [&str; 2] = ["cliente", "tecnico"];
const QUOTE_ITEM_KINDS: [&str; 3] = ["mano_obra", "repuesto", "otro"];
const QUOTE_DECISIONS: [&str; 3] = ["pending", "approved", "rejected"];
const AUTHORIZATION_METHODS: [&str; 5] = ["firma", "presencial", "telefono", "whatsapp", "correo"];
const PAYMENT_TYPES: [&str; 2] = ["advance", "payment"];
const PAYMENT_METHODS: [&str; 5] = ["cash", "card", "transfer", "deposit", "other"];
const WARRANTY_DECISIONS: [&str; 3] = ["pending", "covered", "not_covered"];
const PHOTO_CATEGORIES: [&str; 4] = ["recepcion", "diagnostico", "reparacion", "entrega"];
pub const USER_ROLES: [&str; 3] = ["admin", "reception", "technician"];

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub error: String,
    pub fields: BTreeMap<String, String>,
}

impl CommandError {
    pub fn internal() -> Self {
        Self {
            error: "Ocurrió un error interno al acceder a los datos locales.".into(),
            fields: BTreeMap::new(),
        }
    }

    pub fn message(message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
            fields: BTreeMap::new(),
        }
    }

    fn validation(message: &str, fields: BTreeMap<String, String>) -> Self {
        Self {
            error: message.into(),
            fields,
        }
    }
}

impl From<rusqlite::Error> for CommandError {
    fn from(error: rusqlite::Error) -> Self {
        eprintln!("Error de SQLite: {error}");
        Self::internal()
    }
}

impl From<std::io::Error> for CommandError {
    fn from(error: std::io::Error) -> Self {
        eprintln!("Error de archivo: {error}");
        Self::internal()
    }
}

impl From<serde_json::Error> for CommandError {
    fn from(error: serde_json::Error) -> Self {
        eprintln!("Error al serializar el respaldo: {error}");
        Self::internal()
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateOrderInput {
    pub title: String,
    pub description: String,
    #[serde(alias = "type")]
    pub kind: String,
    pub priority: String,
    pub status: String,
    pub asset_mode: String,
    pub asset_id: String,
    pub incoming_asset: Option<IncomingAssetInput>,
    pub customer_mode: String,
    pub customer_id: String,
    pub incoming_customer: Option<SaveCustomerInput>,
    pub location: String,
    pub assigned_to: String,
    pub requested_by: String,
    pub scheduled_date: String,
    pub estimated_hours: String,
    pub safety_notes: String,
    pub observations: String,
    pub accessories: Vec<String>,
    pub tasks: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct IncomingAssetInput {
    pub code: String,
    pub name: String,
    pub category: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateOrderInput {
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(alias = "type")]
    pub kind: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub asset_id: Option<String>,
    pub customer_id: Option<String>,
    pub location: Option<String>,
    pub assigned_to: Option<String>,
    pub requested_by: Option<String>,
    pub scheduled_date: Option<String>,
    pub estimated_hours: Option<String>,
    pub safety_notes: Option<String>,
    pub observations: Option<String>,
    pub accessories: Option<Vec<String>>,
    pub tasks: Option<Vec<String>>,
    pub work_performed: Option<String>,
    pub delivery_condition: Option<String>,
    pub delivery_tested_by: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QuoteItemInput {
    pub kind: String,
    pub description: String,
    pub quantity: String,
    pub unit_price: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveDiagnosisQuoteInput {
    pub diagnosis: String,
    pub recommendation: String,
    pub discount: String,
    pub items: Vec<QuoteItemInput>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct QuoteDecisionInput {
    pub decision: String,
    pub method: String,
    pub authorized_by: String,
    pub note: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OrderPartInput {
    pub inventory_item_id: String,
    pub quantity: String,
    pub unit_price: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OrderLaborInput {
    pub technician: String,
    pub description: String,
    pub hours: String,
    pub hourly_rate: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveOrderResourcesInput {
    pub parts: Vec<OrderPartInput>,
    pub labor: Vec<OrderLaborInput>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveCustomerInput {
    pub name: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub company: String,
    pub tax_id: String,
    pub notes: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateAssetInput {
    pub code: String,
    pub name: String,
    pub category: String,
    pub location: String,
    pub status: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveInventoryItemInput {
    pub code: String,
    pub name: String,
    pub category: String,
    pub quantity: String,
    pub minimum_stock: String,
    pub unit: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PurchaseReportLineInput {
    pub item_id: String,
    pub purchase_quantity: String,
    pub unit_cost: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreatePurchaseReportInput {
    pub supplier: String,
    pub notes: String,
    pub lines: Vec<PurchaseReportLineInput>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PurchaseReceiptLineInput {
    pub purchase_report_line_id: String,
    pub received_quantity: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReceivePurchaseInput {
    pub notes: String,
    pub lines: Vec<PurchaseReceiptLineInput>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaleLineInput {
    pub inventory_item_id: String,
    pub quantity: String,
    pub unit_price: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateSaleInput {
    pub customer_name: String,
    pub notes: String,
    pub discount: String,
    pub lines: Vec<SaleLineInput>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CancelSaleInput {
    pub note: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateOrderPaymentInput {
    pub payment_type: String,
    pub method: String,
    pub amount: String,
    pub reference: String,
    pub received_by: String,
    pub note: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CancelOrderPaymentInput {
    pub note: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveWarrantyInput {
    pub starts_at: String,
    pub duration_days: String,
    pub coverage: String,
    pub exclusions: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateWarrantyReturnInput {
    pub reported_issue: String,
    pub received_condition: String,
    pub priority: String,
    pub accessories: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ResolveWarrantyClaimInput {
    pub decision: String,
    pub resolution: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveOrderPhotoInput {
    pub file_name: String,
    pub mime_type: String,
    pub category: String,
    pub caption: String,
    pub data: Vec<u8>,
}

#[derive(Debug)]
pub struct ValidatedOrderPhoto {
    pub file_name: String,
    pub mime_type: String,
    pub category: String,
    pub caption: String,
    pub data: Vec<u8>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LoginInput {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveUserInput {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ResetUserPasswordInput {
    pub password: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SetUserActiveInput {
    pub active: bool,
}

#[derive(Debug)]
pub struct ValidatedUser {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct OrderFilters {
    pub status: String,
    pub priority: String,
    pub search: String,
}

#[derive(Debug)]
pub struct ValidatedOrder {
    pub title: String,
    pub description: String,
    pub kind: String,
    pub priority: String,
    pub status: String,
    pub asset_id: Option<i64>,
    pub incoming_asset: Option<ValidatedAsset>,
    pub customer_id: Option<i64>,
    pub incoming_customer: Option<ValidatedCustomer>,
    pub location: String,
    pub assigned_to: String,
    pub requested_by: String,
    pub scheduled_date: Option<String>,
    pub estimated_hours: Option<f64>,
    pub safety_notes: String,
    pub observations: String,
    pub accessories: Vec<String>,
    pub tasks: Vec<String>,
}

#[derive(Debug, Default)]
pub struct ValidatedOrderPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub kind: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
    pub asset_id: Option<Option<i64>>,
    pub customer_id: Option<i64>,
    pub location: Option<String>,
    pub assigned_to: Option<String>,
    pub requested_by: Option<String>,
    pub scheduled_date: Option<Option<String>>,
    pub estimated_hours: Option<Option<f64>>,
    pub safety_notes: Option<String>,
    pub observations: Option<String>,
    pub accessories: Option<Vec<String>>,
    pub tasks: Option<Vec<String>>,
    pub work_performed: Option<String>,
    pub delivery_condition: Option<String>,
    pub delivery_tested_by: Option<String>,
}

#[derive(Debug)]
pub struct ValidatedQuoteItem {
    pub kind: String,
    pub description: String,
    pub quantity: f64,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
}

#[derive(Debug)]
pub struct ValidatedDiagnosisQuote {
    pub diagnosis: String,
    pub recommendation: String,
    pub discount_cents: i64,
    pub items: Vec<ValidatedQuoteItem>,
}

#[derive(Debug)]
pub struct ValidatedQuoteDecision {
    pub decision: String,
    pub method: String,
    pub authorized_by: String,
    pub note: String,
}

#[derive(Debug)]
pub struct ValidatedOrderPart {
    pub inventory_item_id: i64,
    pub quantity: i64,
    pub unit_price_cents: i64,
    pub line_total_cents: i64,
}

#[derive(Debug)]
pub struct ValidatedOrderLabor {
    pub technician: String,
    pub description: String,
    pub hours: f64,
    pub hourly_rate_cents: i64,
    pub line_total_cents: i64,
}

#[derive(Debug)]
pub struct ValidatedOrderResources {
    pub parts: Vec<ValidatedOrderPart>,
    pub labor: Vec<ValidatedOrderLabor>,
}

#[derive(Debug)]
pub struct ValidatedCustomer {
    pub name: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub company: String,
    pub tax_id: String,
    pub notes: String,
}

#[derive(Debug)]
pub struct ValidatedAsset {
    pub code: String,
    pub name: String,
    pub category: String,
    pub location: String,
    pub status: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
}

#[derive(Debug)]
pub struct ValidatedInventoryItem {
    pub code: String,
    pub name: String,
    pub category: String,
    pub quantity: i64,
    pub minimum_stock: i64,
    pub unit: String,
}

#[derive(Debug)]
pub struct ValidatedPurchaseReportLine {
    pub item_id: i64,
    pub purchase_quantity: i64,
    pub unit_cost_cents: i64,
}

#[derive(Debug)]
pub struct ValidatedPurchaseReport {
    pub supplier: String,
    pub notes: String,
    pub lines: Vec<ValidatedPurchaseReportLine>,
}

#[derive(Debug)]
pub struct ValidatedPurchaseReceiptLine {
    pub purchase_report_line_id: i64,
    pub received_quantity: i64,
}

#[derive(Debug)]
pub struct ValidatedPurchaseReceipt {
    pub notes: String,
    pub lines: Vec<ValidatedPurchaseReceiptLine>,
}

#[derive(Debug)]
pub struct ValidatedSaleLine {
    pub inventory_item_id: i64,
    pub quantity: i64,
    pub unit_price_cents: i64,
}

#[derive(Debug)]
pub struct ValidatedSale {
    pub customer_name: String,
    pub notes: String,
    pub discount_cents: i64,
    pub lines: Vec<ValidatedSaleLine>,
}

#[derive(Debug)]
pub struct ValidatedOrderPayment {
    pub payment_type: String,
    pub method: String,
    pub amount_cents: i64,
    pub reference: String,
    pub received_by: String,
    pub note: String,
}

#[derive(Debug)]
pub struct ValidatedWarranty {
    pub starts_at: String,
    pub duration_days: i64,
    pub coverage: String,
    pub exclusions: String,
}

#[derive(Debug)]
pub struct ValidatedWarrantyReturn {
    pub reported_issue: String,
    pub received_condition: String,
    pub priority: String,
    pub accessories: Vec<String>,
}

#[derive(Debug)]
pub struct ValidatedWarrantyDecision {
    pub decision: String,
    pub resolution: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub category: String,
    pub location: String,
    pub status: String,
    pub manufacturer: String,
    pub model: String,
    pub serial_number: String,
    pub created_at: String,
    pub updated_at: String,
    pub order_count: i64,
    pub open_order_count: i64,
    pub customer_id: Option<i64>,
    pub customer_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Customer {
    pub id: i64,
    pub name: String,
    pub phone: String,
    pub email: String,
    pub address: String,
    pub company: String,
    pub tax_id: String,
    pub notes: String,
    pub created_at: String,
    pub updated_at: String,
    pub order_count: i64,
    pub active_order_count: i64,
    pub equipment_count: i64,
    pub last_service_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryItem {
    pub id: i64,
    pub code: String,
    pub name: String,
    pub category: String,
    pub quantity: i64,
    pub minimum_stock: i64,
    pub unit: String,
    pub created_at: String,
    pub updated_at: String,
    pub order_usage_count: i64,
    pub consumed_quantity: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReportLine {
    pub id: i64,
    pub inventory_item_id: Option<i64>,
    pub item_code: String,
    pub item_name: String,
    pub unit: String,
    pub stock_quantity: i64,
    pub purchase_quantity: i64,
    pub received_quantity: i64,
    pub remaining_quantity: i64,
    pub unit_cost: f64,
    pub line_total: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReport {
    pub id: i64,
    pub report_number: String,
    pub supplier: String,
    pub notes: String,
    pub total_cost: f64,
    pub created_at: String,
    pub status: String,
    pub ordered_quantity: i64,
    pub received_quantity: i64,
    pub lines: Vec<PurchaseReportLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReceiptLine {
    pub id: i64,
    pub inventory_item_id: Option<i64>,
    pub item_code: String,
    pub item_name: String,
    pub unit: String,
    pub received_quantity: i64,
    pub unit_cost: f64,
    pub line_total: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurchaseReceipt {
    pub id: i64,
    pub receipt_number: String,
    pub purchase_report_id: i64,
    pub report_number: String,
    pub supplier: String,
    pub notes: String,
    pub total_cost: f64,
    pub created_at: String,
    pub lines: Vec<PurchaseReceiptLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InventoryMovement {
    pub id: i64,
    pub inventory_item_id: Option<i64>,
    pub item_code: String,
    pub item_name: String,
    pub unit: String,
    pub movement_type: String,
    pub quantity_delta: i64,
    pub balance_after: i64,
    pub reference_type: String,
    pub reference_id: Option<i64>,
    pub reference_number: String,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaleLine {
    pub id: i64,
    pub inventory_item_id: Option<i64>,
    pub item_code: String,
    pub item_name: String,
    pub unit: String,
    pub quantity: i64,
    pub unit_price: f64,
    pub line_total: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sale {
    pub id: i64,
    pub sale_number: String,
    pub customer_name: String,
    pub notes: String,
    pub subtotal: f64,
    pub discount: f64,
    pub total: f64,
    pub status: String,
    pub cancellation_note: String,
    pub created_at: String,
    pub cancelled_at: Option<String>,
    pub lines: Vec<SaleLine>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrderTask {
    pub id: i64,
    pub description: String,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub position: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrderEvent {
    pub id: i64,
    pub action: String,
    pub previous_status: Option<String>,
    pub new_status: Option<String>,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteItem {
    pub id: i64,
    pub kind: String,
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub line_total: f64,
    pub position: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrderPart {
    pub id: i64,
    pub inventory_item_id: Option<i64>,
    pub item_code: String,
    pub item_name: String,
    pub unit: String,
    pub quantity: i64,
    pub unit_price: f64,
    pub line_total: f64,
    pub position: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrderLabor {
    pub id: i64,
    pub technician: String,
    pub description: String,
    pub hours: f64,
    pub hourly_rate: f64,
    pub line_total: f64,
    pub position: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderPayment {
    pub id: i64,
    pub receipt_number: String,
    pub payment_type: String,
    pub method: String,
    pub amount: f64,
    pub reference: String,
    pub received_by: String,
    pub note: String,
    pub status: String,
    pub order_total: f64,
    pub balance_before: f64,
    pub balance_after: f64,
    pub created_at: String,
    pub cancelled_at: Option<String>,
    pub cancellation_note: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderWarranty {
    pub id: i64,
    pub warranty_number: String,
    pub duration_days: i64,
    pub starts_at: String,
    pub expires_at: String,
    pub coverage: String,
    pub exclusions: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderPhoto {
    pub id: i64,
    pub work_order_id: i64,
    pub file_name: String,
    pub mime_type: String,
    pub category: String,
    pub caption: String,
    pub size_bytes: i64,
    pub created_by: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WarrantyClaim {
    pub id: i64,
    pub claim_number: String,
    pub warranty_id: i64,
    pub original_order_id: i64,
    pub original_order_number: String,
    pub return_order_id: i64,
    pub return_order_number: String,
    pub reported_issue: String,
    pub received_condition: String,
    pub decision: String,
    pub resolution: String,
    pub warranty_expires_at: String,
    pub within_warranty: bool,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkOrder {
    pub id: i64,
    pub order_number: String,
    pub title: String,
    pub description: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub priority: String,
    pub status: String,
    pub asset_id: Option<i64>,
    pub asset_code: Option<String>,
    pub asset_name: Option<String>,
    pub asset_category: Option<String>,
    pub asset_manufacturer: Option<String>,
    pub asset_model: Option<String>,
    pub asset_serial_number: Option<String>,
    pub customer_id: Option<i64>,
    pub customer_phone: String,
    pub customer_email: String,
    pub customer_address: String,
    pub customer_company: String,
    pub customer_tax_id: String,
    pub location: String,
    pub assigned_to: String,
    pub requested_by: String,
    pub scheduled_date: Option<String>,
    pub estimated_hours: Option<f64>,
    pub safety_notes: String,
    pub observations: String,
    pub work_performed: String,
    pub delivery_condition: String,
    pub delivery_tested_by: String,
    pub diagnosis: String,
    pub recommendation: String,
    pub quote_status: String,
    pub quote_subtotal: f64,
    pub quote_discount: f64,
    pub quote_total: f64,
    pub authorized_by: String,
    pub authorization_method: String,
    pub authorization_note: String,
    pub authorization_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub task_count: i64,
    pub completed_task_count: i64,
    pub accessories: Vec<String>,
    pub tasks: Vec<WorkOrderTask>,
    pub events: Vec<WorkOrderEvent>,
    pub quote_items: Vec<QuoteItem>,
    pub parts: Vec<WorkOrderPart>,
    pub labor: Vec<WorkOrderLabor>,
    pub parts_total: f64,
    pub labor_total: f64,
    pub resources_total: f64,
    pub payments: Vec<OrderPayment>,
    pub financial_total: f64,
    pub payments_total: f64,
    pub balance_due: f64,
    pub credit_balance: f64,
    pub warranty: Option<OrderWarranty>,
    pub warranty_claim: Option<WarrantyClaim>,
    pub warranty_returns: Vec<WarrantyClaim>,
    pub photos: Vec<OrderPhoto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardTotals {
    pub active_orders: i64,
    pub critical_open: i64,
    pub overdue: i64,
    pub due_today: i64,
    pub completed_this_month: i64,
    pub total_orders: i64,
}

#[derive(Debug, Serialize)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct PriorityCount {
    pub priority: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dashboard {
    pub totals: DashboardTotals,
    pub by_status: Vec<StatusCount>,
    pub by_priority: Vec<PriorityCount>,
    pub upcoming: Vec<WorkOrder>,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUser {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub requires_setup: bool,
    pub current_user: Option<SessionUser>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAccount {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub active: bool,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserActivity {
    pub id: i64,
    pub user_display_name: String,
    pub user_role: String,
    pub action: String,
    pub entity_type: String,
    pub entity_id: Option<i64>,
    pub summary: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatisticsStatus {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyStatistic {
    pub month: String,
    pub order_count: i64,
    pub service_revenue: f64,
    pub sales_revenue: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TechnicianStatistic {
    pub technician: String,
    pub jobs: i64,
    pub hours: f64,
    pub labor_total: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkshopStatistics {
    pub range_days: i64,
    pub orders_received: i64,
    pub orders_delivered: i64,
    pub open_orders: i64,
    pub service_revenue: f64,
    pub sales_revenue: f64,
    pub average_turnaround_days: f64,
    pub warranty_returns: i64,
    pub warranty_covered: i64,
    pub by_status: Vec<StatisticsStatus>,
    pub monthly: Vec<MonthlyStatistic>,
    pub technicians: Vec<TechnicianStatistic>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderQr {
    pub payload: String,
    pub svg: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub format: String,
    pub version: u8,
    pub exported_at: String,
    pub assets: Vec<Asset>,
    pub customers: Vec<Customer>,
    pub work_orders: Vec<WorkOrder>,
    pub inventory_items: Vec<InventoryItem>,
    pub purchase_reports: Vec<PurchaseReport>,
    pub purchase_receipts: Vec<PurchaseReceipt>,
    pub inventory_movements: Vec<InventoryMovement>,
    pub sales: Vec<Sale>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileSummary {
    pub file_name: String,
    pub kind: String,
    pub path: String,
    pub size_bytes: u64,
    pub modified_at_epoch_ms: u64,
    pub database_version: i64,
    pub work_order_count: i64,
    pub customer_count: i64,
    pub inventory_item_count: i64,
    pub valid: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOverview {
    pub directory: String,
    pub automatic_enabled: bool,
    pub automatic_retention: usize,
    pub backups: Vec<BackupFileSummary>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RestoreBackupInput {
    pub file_name: String,
    pub confirmed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupResult {
    pub restored_file_name: String,
    pub safety_backup_file_name: String,
}

fn clean(value: &str, max_chars: usize) -> String {
    value.trim().chars().take(max_chars).collect()
}

fn check_enum(
    value: &str,
    allowed: &[&str],
    fallback: &str,
    field: &str,
    errors: &mut BTreeMap<String, String>,
) -> String {
    let normalized = if value.trim().is_empty() {
        fallback
    } else {
        value.trim()
    };
    if allowed.contains(&normalized) {
        normalized.to_owned()
    } else {
        errors.insert(field.into(), "Selecciona una opción válida.".into());
        fallback.into()
    }
}

fn parse_optional_id(
    value: &str,
    field: &str,
    errors: &mut BTreeMap<String, String>,
) -> Option<i64> {
    if value.trim().is_empty() {
        return None;
    }
    match value.parse::<i64>() {
        Ok(id) if id > 0 => Some(id),
        _ => {
            errors.insert(field.into(), "Selecciona un equipo válido.".into());
            None
        }
    }
}

fn parse_hours(value: &str, errors: &mut BTreeMap<String, String>) -> Option<f64> {
    if value.trim().is_empty() {
        return None;
    }
    match value.parse::<f64>() {
        Ok(hours) if hours.is_finite() && (0.0..=10_000.0).contains(&hours) => {
            Some((hours * 100.0).round() / 100.0)
        }
        _ => {
            errors.insert(
                "estimatedHours".into(),
                "Ingresa una cantidad de horas válida.".into(),
            );
            None
        }
    }
}

fn parse_date(value: &str, errors: &mut BTreeMap<String, String>) -> Option<String> {
    if value.trim().is_empty() {
        return None;
    }
    let parts: Vec<_> = value.split('-').collect();
    let valid = if parts.len() == 3 {
        match (
            parts[0].parse::<i32>(),
            parts[1].parse::<u32>(),
            parts[2].parse::<u32>(),
        ) {
            (Ok(year), Ok(month), Ok(day)) => {
                let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
                let max_day = match month {
                    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                    4 | 6 | 9 | 11 => 30,
                    2 if leap => 29,
                    2 => 28,
                    _ => 0,
                };
                parts[0].len() == 4 && day >= 1 && day <= max_day
            }
            _ => false,
        }
    } else {
        false
    };
    if valid {
        Some(value.to_owned())
    } else {
        errors.insert("scheduledDate".into(), "Usa una fecha válida.".into());
        None
    }
}

fn validate_customer_fields(
    input: &SaveCustomerInput,
    name_field: &str,
    email_field: &str,
    errors: &mut BTreeMap<String, String>,
) -> ValidatedCustomer {
    let name = clean(&input.name, 120);
    let email = clean(&input.email, 160).to_lowercase();
    if name.chars().count() < 2 {
        errors.insert(name_field.into(), "Ingresa el nombre del cliente.".into());
    }
    if !email.is_empty()
        && (!email.contains('@')
            || !email
                .split('@')
                .nth(1)
                .is_some_and(|domain| domain.contains('.')))
    {
        errors.insert(
            email_field.into(),
            "Ingresa un correo electrónico válido.".into(),
        );
    }
    ValidatedCustomer {
        name,
        phone: clean(&input.phone, 40),
        email,
        address: clean(&input.address, 240),
        company: clean(&input.company, 120),
        tax_id: clean(&input.tax_id, 30).to_uppercase(),
        notes: clean(&input.notes, 2000),
    }
}

fn clean_list(values: &[String], max_chars: usize, max_items: usize) -> Vec<String> {
    let mut result = Vec::new();
    for value in values.iter().map(|item| clean(item, max_chars)) {
        if !value.is_empty() && !result.contains(&value) && result.len() < max_items {
            result.push(value);
        }
    }
    result
}

pub fn validate_order(input: CreateOrderInput) -> Result<ValidatedOrder, CommandError> {
    let mut errors = BTreeMap::new();
    let title = clean(&input.title, 120);
    let description = clean(&input.description, 4000);
    if title.chars().count() < 3 {
        errors.insert(
            "title".into(),
            "Escribe un título de al menos 3 caracteres.".into(),
        );
    }
    if description.chars().count() < 5 {
        errors.insert(
            "description".into(),
            "Describe brevemente el trabajo requerido.".into(),
        );
    }
    let kind = check_enum(&input.kind, &ORDER_TYPES, "correctivo", "type", &mut errors);
    let priority = check_enum(
        &input.priority,
        &ORDER_PRIORITIES,
        "media",
        "priority",
        &mut errors,
    );
    let status = check_enum(
        &input.status,
        &ORDER_STATUSES,
        "solicitada",
        "status",
        &mut errors,
    );
    let asset_id = parse_optional_id(&input.asset_id, "assetId", &mut errors);
    let use_existing_asset = input.asset_mode == "existing";
    if use_existing_asset && asset_id.is_none() {
        errors.insert(
            "assetId".into(),
            "Selecciona el equipo que regresó al taller.".into(),
        );
    }
    let incoming_asset = if !use_existing_asset && asset_id.is_none() {
        match input.incoming_asset {
            Some(asset) => {
                let code = clean(&asset.code, 40).to_uppercase();
                let name = clean(&asset.name, 120);
                let category = clean(&asset.category, 80);
                if name.chars().count() < 2 {
                    errors.insert("assetName".into(), "Identifica el equipo recibido.".into());
                }
                if category.is_empty() {
                    errors.insert(
                        "assetCategory".into(),
                        "Selecciona el tipo de equipo.".into(),
                    );
                }
                Some(ValidatedAsset {
                    code,
                    name,
                    category,
                    location: {
                        let location = clean(&input.location, 160);
                        if location.is_empty() {
                            "Taller Geecko".into()
                        } else {
                            location
                        }
                    },
                    status: "mantenimiento".into(),
                    manufacturer: clean(&asset.manufacturer, 100),
                    model: clean(&asset.model, 100),
                    serial_number: clean(&asset.serial_number, 100),
                })
            }
            None => {
                errors.insert(
                    "assetName".into(),
                    "Registra el equipo recibido o selecciona uno existente.".into(),
                );
                None
            }
        }
    } else {
        None
    };
    let use_existing_customer = input.customer_mode == "existing";
    let customer_id = if use_existing_customer {
        match input.customer_id.trim().parse::<i64>() {
            Ok(id) if id > 0 => Some(id),
            _ => {
                errors.insert(
                    "customerId".into(),
                    "Selecciona un cliente registrado.".into(),
                );
                None
            }
        }
    } else {
        None
    };
    let fallback_customer;
    let customer_input = if use_existing_customer {
        None
    } else if let Some(customer) = input.incoming_customer.as_ref() {
        Some(customer)
    } else if !input.requested_by.trim().is_empty() {
        fallback_customer = SaveCustomerInput {
            name: input.requested_by.clone(),
            ..SaveCustomerInput::default()
        };
        Some(&fallback_customer)
    } else {
        errors.insert(
            "customerName".into(),
            "Ingresa o selecciona un cliente.".into(),
        );
        None
    };
    let incoming_customer = customer_input.map(|customer| {
        validate_customer_fields(customer, "customerName", "customerEmail", &mut errors)
    });
    let scheduled_date = parse_date(&input.scheduled_date, &mut errors);
    let estimated_hours = parse_hours(&input.estimated_hours, &mut errors);
    let tasks = clean_list(&input.tasks, 300, 100);
    let accessories = clean_list(&input.accessories, 100, 30);

    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos de la orden.",
            errors,
        ));
    }
    Ok(ValidatedOrder {
        title,
        description,
        kind,
        priority,
        status,
        asset_id,
        incoming_asset,
        customer_id,
        incoming_customer,
        location: clean(&input.location, 160),
        assigned_to: clean(&input.assigned_to, 120),
        requested_by: clean(&input.requested_by, 120),
        scheduled_date,
        estimated_hours,
        safety_notes: clean(&input.safety_notes, 2000),
        observations: clean(&input.observations, 4000),
        accessories,
        tasks,
    })
}

pub fn validate_order_patch(input: UpdateOrderInput) -> Result<ValidatedOrderPatch, CommandError> {
    let mut errors = BTreeMap::new();
    let title = input.title.map(|value| {
        let value = clean(&value, 120);
        if value.chars().count() < 3 {
            errors.insert(
                "title".into(),
                "Escribe un título de al menos 3 caracteres.".into(),
            );
        }
        value
    });
    let description = input.description.map(|value| {
        let value = clean(&value, 4000);
        if value.chars().count() < 5 {
            errors.insert(
                "description".into(),
                "Describe brevemente el trabajo requerido.".into(),
            );
        }
        value
    });
    let kind = input
        .kind
        .map(|value| check_enum(&value, &ORDER_TYPES, "correctivo", "type", &mut errors));
    let priority = input
        .priority
        .map(|value| check_enum(&value, &ORDER_PRIORITIES, "media", "priority", &mut errors));
    let status = input
        .status
        .map(|value| check_enum(&value, &ORDER_STATUSES, "solicitada", "status", &mut errors));
    let asset_id = input.asset_id.map(|value| {
        let id = parse_optional_id(&value, "assetId", &mut errors);
        if id.is_none() {
            errors.insert("assetId".into(), "Selecciona un equipo registrado.".into());
        }
        id
    });
    let customer_id = input
        .customer_id
        .map(|value| match value.trim().parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                errors.insert(
                    "customerId".into(),
                    "Selecciona un cliente registrado.".into(),
                );
                0
            }
        });
    let scheduled_date = input
        .scheduled_date
        .map(|value| parse_date(&value, &mut errors));
    let estimated_hours = input
        .estimated_hours
        .map(|value| parse_hours(&value, &mut errors));
    let delivery_condition = input.delivery_condition.map(|value| {
        let value = value.trim();
        if value.is_empty() {
            String::new()
        } else {
            check_enum(
                value,
                &DELIVERY_CONDITIONS,
                "reparado",
                "deliveryCondition",
                &mut errors,
            )
        }
    });
    let delivery_tested_by = input.delivery_tested_by.map(|value| {
        let value = value.trim();
        if value.is_empty() {
            String::new()
        } else {
            check_enum(
                value,
                &DELIVERY_TESTERS,
                "cliente",
                "deliveryTestedBy",
                &mut errors,
            )
        }
    });

    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos de la orden.",
            errors,
        ));
    }
    Ok(ValidatedOrderPatch {
        title,
        description,
        kind,
        priority,
        status,
        asset_id,
        customer_id,
        location: input.location.map(|value| clean(&value, 160)),
        assigned_to: input.assigned_to.map(|value| clean(&value, 120)),
        requested_by: input.requested_by.map(|value| clean(&value, 120)),
        scheduled_date,
        estimated_hours,
        safety_notes: input.safety_notes.map(|value| clean(&value, 2000)),
        observations: input.observations.map(|value| clean(&value, 4000)),
        accessories: input
            .accessories
            .as_ref()
            .map(|values| clean_list(values, 100, 30)),
        tasks: input
            .tasks
            .as_ref()
            .map(|values| clean_list(values, 300, 100)),
        work_performed: input.work_performed.map(|value| clean(&value, 6000)),
        delivery_condition,
        delivery_tested_by,
    })
}

pub fn validate_customer(input: SaveCustomerInput) -> Result<ValidatedCustomer, CommandError> {
    let mut errors = BTreeMap::new();
    let customer = validate_customer_fields(&input, "name", "email", &mut errors);
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos del cliente.",
            errors,
        ));
    }
    Ok(customer)
}

pub fn validate_asset(input: CreateAssetInput) -> Result<ValidatedAsset, CommandError> {
    let mut errors = BTreeMap::new();
    let code = clean(&input.code, 40).to_uppercase();
    let name = clean(&input.name, 120);
    let location = clean(&input.location, 160);
    let status = check_enum(
        &input.status,
        &ASSET_STATUSES,
        "operativo",
        "status",
        &mut errors,
    );
    if code.chars().count() < 2 {
        errors.insert(
            "code".into(),
            "Ingresa un código de al menos 2 caracteres.".into(),
        );
    }
    if name.chars().count() < 2 {
        errors.insert("name".into(), "Ingresa el nombre del equipo.".into());
    }
    if location.is_empty() {
        errors.insert("location".into(), "Ingresa la ubicación del equipo.".into());
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos del equipo.",
            errors,
        ));
    }
    Ok(ValidatedAsset {
        code,
        name,
        category: clean(&input.category, 80),
        location,
        status,
        manufacturer: clean(&input.manufacturer, 100),
        model: clean(&input.model, 100),
        serial_number: clean(&input.serial_number, 100),
    })
}

fn parse_stock_quantity(value: &str, field: &str, errors: &mut BTreeMap<String, String>) -> i64 {
    match value.trim().parse::<i64>() {
        Ok(quantity) if (0..=1_000_000_000).contains(&quantity) => quantity,
        _ => {
            errors.insert(
                field.into(),
                "Ingresa una cantidad entera igual o mayor que cero.".into(),
            );
            0
        }
    }
}

pub fn validate_inventory_item(
    input: SaveInventoryItemInput,
) -> Result<ValidatedInventoryItem, CommandError> {
    let mut errors = BTreeMap::new();
    let code = clean(&input.code, 40).to_uppercase();
    let name = clean(&input.name, 140);
    if name.chars().count() < 2 {
        errors.insert("name".into(), "Ingresa el nombre del producto.".into());
    }
    let quantity = parse_stock_quantity(&input.quantity, "quantity", &mut errors);
    let minimum_stock = parse_stock_quantity(&input.minimum_stock, "minimumStock", &mut errors);
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos del producto.",
            errors,
        ));
    }
    let unit = clean(&input.unit, 30);
    Ok(ValidatedInventoryItem {
        code,
        name,
        category: clean(&input.category, 80),
        quantity,
        minimum_stock,
        unit: if unit.is_empty() {
            "unidad".into()
        } else {
            unit
        },
    })
}

pub fn validate_purchase_report(
    input: CreatePurchaseReportInput,
) -> Result<ValidatedPurchaseReport, CommandError> {
    let mut errors = BTreeMap::new();
    let mut seen_items = std::collections::BTreeSet::new();
    let mut lines = Vec::new();

    for line in input.lines.into_iter().take(500) {
        let item_id = match line.item_id.trim().parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                errors.insert(
                    "lines".into(),
                    "El reporte contiene un producto inválido.".into(),
                );
                continue;
            }
        };
        if !seen_items.insert(item_id) {
            continue;
        }
        let purchase_quantity = match line.purchase_quantity.trim().parse::<i64>() {
            Ok(quantity) if (1..=1_000_000_000).contains(&quantity) => quantity,
            _ => {
                errors.insert(
                    "lines".into(),
                    "La cantidad a comprar debe ser un número entero mayor que cero.".into(),
                );
                continue;
            }
        };
        let normalized_cost = line.unit_cost.trim().replace(',', ".");
        let unit_cost_cents = match normalized_cost.parse::<f64>() {
            Ok(cost) if cost.is_finite() && cost > 0.0 && cost <= 1_000_000_000.0 => {
                (cost * 100.0).round() as i64
            }
            _ => {
                errors.insert(
                    "lines".into(),
                    "Ingresa un costo unitario válido para cada producto seleccionado.".into(),
                );
                continue;
            }
        };
        lines.push(ValidatedPurchaseReportLine {
            item_id,
            purchase_quantity,
            unit_cost_cents,
        });
    }

    if lines.is_empty() && !errors.contains_key("lines") {
        errors.insert(
            "lines".into(),
            "Selecciona al menos un producto para el reporte.".into(),
        );
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los productos del reporte.",
            errors,
        ));
    }
    Ok(ValidatedPurchaseReport {
        supplier: clean(&input.supplier, 160),
        notes: clean(&input.notes, 2000),
        lines,
    })
}

fn parse_money_cents(
    value: &str,
    field: &str,
    allow_zero: bool,
    errors: &mut BTreeMap<String, String>,
) -> i64 {
    let normalized = value.trim().replace(',', ".");
    match normalized.parse::<f64>() {
        Ok(amount)
            if amount.is_finite()
                && amount >= if allow_zero { 0.0 } else { 0.01 }
                && amount <= 1_000_000_000.0 =>
        {
            (amount * 100.0).round() as i64
        }
        _ => {
            errors.insert(field.into(), "Ingresa un monto válido.".into());
            0
        }
    }
}

pub fn validate_purchase_receipt(
    input: ReceivePurchaseInput,
) -> Result<ValidatedPurchaseReceipt, CommandError> {
    let mut errors = BTreeMap::new();
    let mut seen_lines = std::collections::BTreeSet::new();
    let mut lines = Vec::new();
    for line in input.lines.into_iter().take(500) {
        let line_id = match line.purchase_report_line_id.trim().parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                errors.insert(
                    "lines".into(),
                    "La recepción contiene un producto inválido.".into(),
                );
                continue;
            }
        };
        if !seen_lines.insert(line_id) {
            continue;
        }
        let received_quantity = match line.received_quantity.trim().parse::<i64>() {
            Ok(quantity) if (1..=1_000_000_000).contains(&quantity) => quantity,
            _ => {
                errors.insert(
                    "lines".into(),
                    "Cada cantidad recibida debe ser un número entero mayor que cero.".into(),
                );
                continue;
            }
        };
        lines.push(ValidatedPurchaseReceiptLine {
            purchase_report_line_id: line_id,
            received_quantity,
        });
    }
    if lines.is_empty() && !errors.contains_key("lines") {
        errors.insert(
            "lines".into(),
            "Selecciona al menos un producto recibido.".into(),
        );
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa las cantidades recibidas.",
            errors,
        ));
    }
    Ok(ValidatedPurchaseReceipt {
        notes: clean(&input.notes, 2000),
        lines,
    })
}

pub fn validate_sale(input: CreateSaleInput) -> Result<ValidatedSale, CommandError> {
    let mut errors = BTreeMap::new();
    let mut seen_items = std::collections::BTreeSet::new();
    let mut lines = Vec::new();
    for (index, line) in input.lines.into_iter().take(200).enumerate() {
        let item_id = match line.inventory_item_id.trim().parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                errors.insert(
                    "lines".into(),
                    format!("Selecciona un producto válido en la línea {}.", index + 1),
                );
                continue;
            }
        };
        if !seen_items.insert(item_id) {
            errors.insert(
                "lines".into(),
                "Cada producto sólo puede aparecer una vez.".into(),
            );
            continue;
        }
        let quantity = match line.quantity.trim().parse::<i64>() {
            Ok(quantity) if (1..=1_000_000).contains(&quantity) => quantity,
            _ => {
                errors.insert(
                    "lines".into(),
                    format!("Ingresa una cantidad válida en la línea {}.", index + 1),
                );
                continue;
            }
        };
        let unit_price_cents = parse_money_cents(&line.unit_price, "lines", true, &mut errors);
        lines.push(ValidatedSaleLine {
            inventory_item_id: item_id,
            quantity,
            unit_price_cents,
        });
    }
    if lines.is_empty() && !errors.contains_key("lines") {
        errors.insert(
            "lines".into(),
            "Agrega al menos un producto a la venta.".into(),
        );
    }
    let discount_cents = parse_money_cents(
        if input.discount.trim().is_empty() {
            "0"
        } else {
            &input.discount
        },
        "discount",
        true,
        &mut errors,
    );
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos de la venta.",
            errors,
        ));
    }
    Ok(ValidatedSale {
        customer_name: clean(&input.customer_name, 160),
        notes: clean(&input.notes, 2000),
        discount_cents,
        lines,
    })
}

pub fn validate_cancellation_note(input: CancelSaleInput) -> String {
    clean(&input.note, 1000)
}

pub fn validate_order_payment(
    input: CreateOrderPaymentInput,
) -> Result<ValidatedOrderPayment, CommandError> {
    let mut errors = BTreeMap::new();
    let payment_type = check_enum(
        &input.payment_type,
        &PAYMENT_TYPES,
        "payment",
        "paymentType",
        &mut errors,
    );
    let method = check_enum(
        &input.method,
        &PAYMENT_METHODS,
        "cash",
        "method",
        &mut errors,
    );
    let amount_cents = parse_money_cents(&input.amount, "amount", false, &mut errors);
    let received_by = clean(&input.received_by, 160);
    if received_by.chars().count() < 2 {
        errors.insert("receivedBy".into(), "Indica quién recibió el pago.".into());
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos del pago.",
            errors,
        ));
    }
    Ok(ValidatedOrderPayment {
        payment_type,
        method,
        amount_cents,
        reference: clean(&input.reference, 160),
        received_by,
        note: clean(&input.note, 1000),
    })
}

pub fn validate_payment_cancellation(
    input: CancelOrderPaymentInput,
) -> Result<String, CommandError> {
    let note = clean(&input.note, 1000);
    if note.chars().count() < 3 {
        let mut errors = BTreeMap::new();
        errors.insert("note".into(), "Indica el motivo de la anulación.".into());
        return Err(CommandError::validation(
            "Se necesita un motivo para anular el pago.",
            errors,
        ));
    }
    Ok(note)
}

pub fn validate_warranty(input: SaveWarrantyInput) -> Result<ValidatedWarranty, CommandError> {
    let mut errors = BTreeMap::new();
    let starts_at = parse_date(&input.starts_at, &mut errors);
    if let Some(message) = errors.remove("scheduledDate") {
        errors.insert("startsAt".into(), message);
    }
    if starts_at.is_none() && !errors.contains_key("startsAt") {
        errors.insert(
            "startsAt".into(),
            "Indica la fecha de inicio de la garantía.".into(),
        );
    }
    let duration_days = match input.duration_days.trim().parse::<i64>() {
        Ok(days) if (1..=3650).contains(&days) => days,
        _ => {
            errors.insert(
                "durationDays".into(),
                "La vigencia debe estar entre 1 y 3650 días.".into(),
            );
            0
        }
    };
    let coverage = clean(&input.coverage, 4000);
    if coverage.chars().count() < 5 {
        errors.insert(
            "coverage".into(),
            "Describe qué trabajos o piezas cubre la garantía.".into(),
        );
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa las condiciones de garantía.",
            errors,
        ));
    }
    Ok(ValidatedWarranty {
        starts_at: starts_at.unwrap_or_default(),
        duration_days,
        coverage,
        exclusions: clean(&input.exclusions, 4000),
    })
}

pub fn validate_warranty_return(
    input: CreateWarrantyReturnInput,
) -> Result<ValidatedWarrantyReturn, CommandError> {
    let mut errors = BTreeMap::new();
    let reported_issue = clean(&input.reported_issue, 4000);
    if reported_issue.chars().count() < 5 {
        errors.insert(
            "reportedIssue".into(),
            "Describe la falla por la que regresa el equipo.".into(),
        );
    }
    let priority = check_enum(
        &input.priority,
        &ORDER_PRIORITIES,
        "media",
        "priority",
        &mut errors,
    );
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos del regreso.",
            errors,
        ));
    }
    Ok(ValidatedWarrantyReturn {
        reported_issue,
        received_condition: clean(&input.received_condition, 4000),
        priority,
        accessories: clean_list(&input.accessories, 100, 30),
    })
}

pub fn validate_warranty_decision(
    input: ResolveWarrantyClaimInput,
) -> Result<ValidatedWarrantyDecision, CommandError> {
    let mut errors = BTreeMap::new();
    let decision = check_enum(
        &input.decision,
        &WARRANTY_DECISIONS,
        "pending",
        "decision",
        &mut errors,
    );
    let resolution = clean(&input.resolution, 4000);
    if decision != "pending" && resolution.chars().count() < 5 {
        errors.insert(
            "resolution".into(),
            "Documenta por qué se acepta o rechaza la cobertura.".into(),
        );
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa la decisión de garantía.",
            errors,
        ));
    }
    Ok(ValidatedWarrantyDecision {
        decision,
        resolution,
    })
}

pub fn validate_order_photo(
    input: SaveOrderPhotoInput,
) -> Result<ValidatedOrderPhoto, CommandError> {
    let mut errors = BTreeMap::new();
    let file_name = clean(&input.file_name, 180);
    let mime_type = clean(&input.mime_type.to_lowercase(), 40);
    let category = check_enum(
        &input.category,
        &PHOTO_CATEGORIES,
        "recepcion",
        "category",
        &mut errors,
    );
    let caption = clean(&input.caption, 500);
    if file_name.is_empty() {
        errors.insert("file".into(), "Selecciona una fotografía.".into());
    }
    if !matches!(
        mime_type.as_str(),
        "image/jpeg" | "image/png" | "image/webp"
    ) {
        errors.insert("file".into(), "Usa una imagen JPEG, PNG o WebP.".into());
    }
    if input.data.is_empty() || input.data.len() > 8 * 1024 * 1024 {
        errors.insert(
            "file".into(),
            "La fotografía debe pesar menos de 8 MB.".into(),
        );
    }
    let signature_matches = match mime_type.as_str() {
        "image/jpeg" => input.data.starts_with(&[0xff, 0xd8, 0xff]),
        "image/png" => input.data.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/webp" => {
            input.data.len() >= 12
                && input.data.starts_with(b"RIFF")
                && input.data.get(8..12) == Some(b"WEBP")
        }
        _ => false,
    };
    if !input.data.is_empty() && !signature_matches {
        errors.insert(
            "file".into(),
            "El contenido de la fotografía no coincide con su formato.".into(),
        );
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa la fotografía seleccionada.",
            errors,
        ));
    }
    Ok(ValidatedOrderPhoto {
        file_name,
        mime_type,
        category,
        caption,
        data: input.data,
    })
}

pub fn validate_login(input: LoginInput) -> Result<(String, String), CommandError> {
    let username = input.username.trim().to_lowercase();
    if username.is_empty() || input.password.is_empty() {
        return Err(CommandError::message("Ingresa tu usuario y contraseña."));
    }
    Ok((username, input.password))
}

pub fn validate_password(password: String) -> Result<String, CommandError> {
    if password.chars().count() < 8 || password.chars().count() > 128 {
        return Err(CommandError::message(
            "La contraseña debe tener entre 8 y 128 caracteres.",
        ));
    }
    Ok(password)
}

pub fn validate_user(input: SaveUserInput) -> Result<ValidatedUser, CommandError> {
    let mut errors = BTreeMap::new();
    let username = input.username.trim().to_lowercase();
    let display_name = clean(&input.display_name, 120);
    let role = check_enum(&input.role, &USER_ROLES, "technician", "role", &mut errors);
    if username.chars().count() < 3
        || username.chars().count() > 32
        || !username
            .chars()
            .all(|value| value.is_ascii_alphanumeric() || matches!(value, '.' | '_' | '-'))
    {
        errors.insert(
            "username".into(),
            "Usa entre 3 y 32 letras, números, puntos, guiones o guiones bajos.".into(),
        );
    }
    if display_name.chars().count() < 2 {
        errors.insert("displayName".into(), "Indica el nombre visible.".into());
    }
    if input.password.chars().count() < 8 || input.password.chars().count() > 128 {
        errors.insert(
            "password".into(),
            "La contraseña debe tener entre 8 y 128 caracteres.".into(),
        );
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos del usuario.",
            errors,
        ));
    }
    Ok(ValidatedUser {
        username,
        display_name,
        role,
        password: input.password,
    })
}

pub fn validate_statistics_range(range_days: i64) -> Result<i64, CommandError> {
    if matches!(range_days, 0 | 30 | 90 | 365) {
        Ok(range_days)
    } else {
        Err(CommandError::message(
            "Selecciona un periodo válido para las estadísticas.",
        ))
    }
}

pub fn validate_diagnosis_quote(
    input: SaveDiagnosisQuoteInput,
) -> Result<ValidatedDiagnosisQuote, CommandError> {
    let mut errors = BTreeMap::new();
    let diagnosis = clean(&input.diagnosis, 6000);
    if diagnosis.chars().count() < 5 {
        errors.insert(
            "diagnosis".into(),
            "Describe el diagnóstico con al menos 5 caracteres.".into(),
        );
    }
    let discount_cents = parse_money_cents(
        if input.discount.trim().is_empty() {
            "0"
        } else {
            &input.discount
        },
        "discount",
        true,
        &mut errors,
    );
    let mut items = Vec::new();
    for (index, item) in input.items.into_iter().take(200).enumerate() {
        let description = clean(&item.description, 300);
        let empty_line = description.is_empty()
            && item.quantity.trim().is_empty()
            && item.unit_price.trim().is_empty();
        if empty_line {
            continue;
        }
        if description.chars().count() < 2 {
            errors.insert(
                "items".into(),
                format!("Describe el concepto de la línea {}.", index + 1),
            );
            continue;
        }
        let kind = check_enum(
            &item.kind,
            &QUOTE_ITEM_KINDS,
            "mano_obra",
            "items",
            &mut errors,
        );
        let quantity = match item.quantity.trim().replace(',', ".").parse::<f64>() {
            Ok(quantity) if quantity.is_finite() && quantity > 0.0 && quantity <= 1_000_000.0 => {
                quantity
            }
            _ => {
                errors.insert(
                    "items".into(),
                    format!("Ingresa una cantidad válida en la línea {}.", index + 1),
                );
                continue;
            }
        };
        let unit_price_cents = parse_money_cents(&item.unit_price, "items", true, &mut errors);
        let line_total_cents = (quantity * unit_price_cents as f64).round() as i64;
        items.push(ValidatedQuoteItem {
            kind,
            description,
            quantity,
            unit_price_cents,
            line_total_cents,
        });
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa el diagnóstico y la cotización.",
            errors,
        ));
    }
    Ok(ValidatedDiagnosisQuote {
        diagnosis,
        recommendation: clean(&input.recommendation, 4000),
        discount_cents,
        items,
    })
}

pub fn validate_quote_decision(
    input: QuoteDecisionInput,
) -> Result<ValidatedQuoteDecision, CommandError> {
    let mut errors = BTreeMap::new();
    let decision = check_enum(
        &input.decision,
        &QUOTE_DECISIONS,
        "pending",
        "decision",
        &mut errors,
    );
    let method = clean(&input.method, 40);
    let authorized_by = clean(&input.authorized_by, 160);
    if matches!(decision.as_str(), "approved" | "rejected") {
        if !AUTHORIZATION_METHODS.contains(&method.as_str()) {
            errors.insert(
                "method".into(),
                "Selecciona cómo se recibió la decisión del cliente.".into(),
            );
        }
        if authorized_by.chars().count() < 2 {
            errors.insert(
                "authorizedBy".into(),
                "Indica quién autorizó o rechazó la cotización.".into(),
            );
        }
    }
    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los datos de autorización.",
            errors,
        ));
    }
    Ok(ValidatedQuoteDecision {
        decision,
        method,
        authorized_by,
        note: clean(&input.note, 2000),
    })
}

pub fn validate_order_resources(
    input: SaveOrderResourcesInput,
) -> Result<ValidatedOrderResources, CommandError> {
    let mut errors = BTreeMap::new();
    let mut seen_items = std::collections::BTreeSet::new();
    let mut parts = Vec::new();
    for (index, part) in input.parts.into_iter().take(100).enumerate() {
        if part.inventory_item_id.trim().is_empty() {
            continue;
        }
        let inventory_item_id = match part.inventory_item_id.trim().parse::<i64>() {
            Ok(id) if id > 0 => id,
            _ => {
                errors.insert(
                    "parts".into(),
                    format!("Selecciona un repuesto válido en la línea {}.", index + 1),
                );
                continue;
            }
        };
        if !seen_items.insert(inventory_item_id) {
            errors.insert(
                "parts".into(),
                "Cada producto del inventario sólo puede aparecer una vez.".into(),
            );
            continue;
        }
        let quantity = match part.quantity.trim().parse::<i64>() {
            Ok(quantity) if (1..=1_000_000).contains(&quantity) => quantity,
            _ => {
                errors.insert(
                    "parts".into(),
                    format!(
                        "Ingresa una cantidad entera válida en la línea {}.",
                        index + 1
                    ),
                );
                continue;
            }
        };
        let unit_price_cents = parse_money_cents(
            if part.unit_price.trim().is_empty() {
                "0"
            } else {
                &part.unit_price
            },
            "parts",
            true,
            &mut errors,
        );
        parts.push(ValidatedOrderPart {
            inventory_item_id,
            quantity,
            unit_price_cents,
            line_total_cents: quantity * unit_price_cents,
        });
    }

    let mut labor = Vec::new();
    for (index, line) in input.labor.into_iter().take(100).enumerate() {
        let description = clean(&line.description, 500);
        if description.is_empty() {
            continue;
        }
        let technician = clean(&line.technician, 160);
        if technician.chars().count() < 2 {
            errors.insert(
                "labor".into(),
                format!("Indica el técnico de la línea {}.", index + 1),
            );
            continue;
        }
        let hours = match line.hours.trim().replace(',', ".").parse::<f64>() {
            Ok(hours) if hours.is_finite() && hours > 0.0 && hours <= 10_000.0 => hours,
            _ => {
                errors.insert(
                    "labor".into(),
                    format!("Ingresa horas válidas en la línea {}.", index + 1),
                );
                continue;
            }
        };
        let hourly_rate_cents = parse_money_cents(
            if line.hourly_rate.trim().is_empty() {
                "0"
            } else {
                &line.hourly_rate
            },
            "labor",
            true,
            &mut errors,
        );
        labor.push(ValidatedOrderLabor {
            technician,
            description,
            hours,
            hourly_rate_cents,
            line_total_cents: (hours * hourly_rate_cents as f64).round() as i64,
        });
    }

    if !errors.is_empty() {
        return Err(CommandError::validation(
            "Revisa los repuestos y la mano de obra.",
            errors,
        ));
    }
    Ok(ValidatedOrderResources { parts, labor })
}
