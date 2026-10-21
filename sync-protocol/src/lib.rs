use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 1;
pub const DEFAULT_PULL_LIMIT: u32 = 250;
pub const MAX_PULL_LIMIT: u32 = 1_000;
pub const MAX_PUSH_MUTATIONS: usize = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Customer,
    Asset,
    WorkOrder,
    WorkOrderTask,
    WorkOrderEvent,
    WorkOrderAccessory,
    QuoteItem,
    OrderPart,
    OrderLabor,
    OrderPayment,
    OrderWarranty,
    WarrantyClaim,
    OrderPhoto,
    InventoryItem,
    InventoryMovement,
    PurchaseReport,
    PurchaseReceipt,
    Sale,
}

impl EntityKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Customer => "customer",
            Self::Asset => "asset",
            Self::WorkOrder => "work_order",
            Self::WorkOrderTask => "work_order_task",
            Self::WorkOrderEvent => "work_order_event",
            Self::WorkOrderAccessory => "work_order_accessory",
            Self::QuoteItem => "quote_item",
            Self::OrderPart => "order_part",
            Self::OrderLabor => "order_labor",
            Self::OrderPayment => "order_payment",
            Self::OrderWarranty => "order_warranty",
            Self::WarrantyClaim => "warranty_claim",
            Self::OrderPhoto => "order_photo",
            Self::InventoryItem => "inventory_item",
            Self::InventoryMovement => "inventory_movement",
            Self::PurchaseReport => "purchase_report",
            Self::PurchaseReceipt => "purchase_receipt",
            Self::Sale => "sale",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncMutation {
    pub operation_id: Uuid,
    pub entity: EntityKind,
    pub entity_id: Uuid,
    pub base_revision: i64,
    #[serde(default)]
    pub deleted: bool,
    pub payload: Value,
    pub client_timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushRequest {
    pub protocol_version: u16,
    pub device_id: Uuid,
    pub mutations: Vec<SyncMutation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatus {
    Applied,
    Duplicate,
    Conflict,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MutationResult {
    pub operation_id: Uuid,
    pub entity_id: Uuid,
    pub status: MutationStatus,
    pub server_revision: i64,
    pub change_cursor: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_record: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushResponse {
    pub protocol_version: u16,
    pub server_cursor: i64,
    pub results: Vec<MutationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerChange {
    pub cursor: i64,
    pub entity: EntityKind,
    pub entity_id: Uuid,
    pub revision: i64,
    pub deleted: bool,
    pub payload: Value,
    pub origin_device_id: Uuid,
    pub server_timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PullResponse {
    pub protocol_version: u16,
    pub cursor: i64,
    pub has_more: bool,
    pub changes: Vec<ServerChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub service: String,
    pub protocol_version: u16,
    pub postgres_version: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_serializes_with_camel_case_and_stable_entity_names() {
        let request = PushRequest {
            protocol_version: PROTOCOL_VERSION,
            device_id: Uuid::nil(),
            mutations: vec![SyncMutation {
                operation_id: Uuid::nil(),
                entity: EntityKind::WorkOrder,
                entity_id: Uuid::nil(),
                base_revision: 0,
                deleted: false,
                payload: serde_json::json!({ "title": "Diagnóstico" }),
                client_timestamp: "2026-10-20T00:00:00Z".into(),
            }],
        };
        let value = serde_json::to_value(request).unwrap();
        assert_eq!(value["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(value["mutations"][0]["entity"], "work_order");
        assert_eq!(value["mutations"][0]["baseRevision"], 0);
    }
}
