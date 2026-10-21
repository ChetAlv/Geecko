use axum::{
    extract::{DefaultBodyLimit, Query, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use geecko_sync_protocol::{
    EntityKind, HealthResponse, MutationResult, MutationStatus, PullResponse, PushRequest,
    PushResponse, ServerChange, DEFAULT_PULL_LIMIT, MAX_PULL_LIMIT, MAX_PUSH_MUTATIONS,
    PROTOCOL_VERSION,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Row, Transaction};
use std::{env, net::SocketAddr};
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pool: PgPool,
    postgres_version: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    error: String,
}

pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: "Dispositivo no autorizado.".into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        eprintln!("Error del servidor de sincronización: {error}");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "Error interno de sincronización.".into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug, Clone)]
struct DeviceContext {
    id: Uuid,
    workshop_id: Uuid,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PullQuery {
    #[serde(default)]
    cursor: i64,
    #[serde(default = "default_pull_limit")]
    limit: u32,
}

const fn default_pull_limit() -> u32 {
    DEFAULT_PULL_LIMIT
}

pub async fn connect(database_url: &str) -> Result<AppState, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(12)
        .connect(database_url)
        .await?;
    let postgres_version: i32 =
        sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
            .fetch_one(&pool)
            .await?;
    if postgres_version < 180_000 {
        return Err(sqlx::Error::Protocol(format!(
            "Geecko Sync requiere PostgreSQL 18 o posterior; el servidor reportó {postgres_version}."
        )));
    }
    sqlx::migrate!()
        .run(&pool)
        .await
        .map_err(|error| sqlx::Error::Migrate(Box::new(error)))?;
    Ok(AppState {
        pool,
        postgres_version,
    })
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/sync/push", post(push))
        .route("/v1/sync/pull", get(pull))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .with_state(state)
}

pub async fn serve_from_environment() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let database_url = env::var("DATABASE_URL")
        .map_err(|_| "Falta la variable DATABASE_URL para PostgreSQL 18.")?;
    let address: SocketAddr = env::var("GEECKO_SYNC_BIND")
        .unwrap_or_else(|_| "127.0.0.1:8787".into())
        .parse()?;
    let state = connect(&database_url).await?;
    let listener = tokio::net::TcpListener::bind(address).await?;
    println!("Geecko Sync escuchando en http://{address}");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    println!("Geecko Sync se detuvo correctamente.");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            eprintln!("No fue posible escuchar Ctrl+C: {error}");
        }
    };

    #[cfg(unix)]
    {
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(error) => {
                    eprintln!("No fue posible escuchar SIGTERM: {error}");
                    std::future::pending::<()>().await;
                }
            }
        };

        tokio::select! {
            _ = ctrl_c => {},
            _ = terminate => {},
        }
    }

    #[cfg(not(unix))]
    ctrl_c.await;

    println!("Geecko Sync recibió la señal de apagado; cerrando solicitudes activas.");
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        service: "geecko-sync".into(),
        protocol_version: PROTOCOL_VERSION,
        postgres_version: state.postgres_version,
    })
}

async fn authenticate(pool: &PgPool, headers: &HeaderMap) -> ApiResult<DeviceContext> {
    let authorization = headers
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .filter(|value| !value.is_empty())
        .ok_or_else(ApiError::unauthorized)?;
    let token_hash = Sha256::digest(authorization.as_bytes()).to_vec();
    let row = sqlx::query(
        "UPDATE geecko.devices
         SET last_seen_at = clock_timestamp()
         WHERE token_hash = $1 AND enabled = true AND revoked_at IS NULL
         RETURNING id, workshop_id",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(ApiError::internal)?
    .ok_or_else(ApiError::unauthorized)?;
    Ok(DeviceContext {
        id: row.get("id"),
        workshop_id: row.get("workshop_id"),
    })
}

async fn push(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PushRequest>,
) -> ApiResult<Json<PushResponse>> {
    let device = authenticate(&state.pool, &headers).await?;
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(ApiError::bad_request(
            "Versión de sincronización incompatible.",
        ));
    }
    if request.device_id != device.id {
        return Err(ApiError::unauthorized());
    }
    if request.mutations.len() > MAX_PUSH_MUTATIONS {
        return Err(ApiError::bad_request(format!(
            "Cada envío admite como máximo {MAX_PUSH_MUTATIONS} cambios."
        )));
    }

    let mut transaction = state.pool.begin().await.map_err(ApiError::internal)?;
    let mut results = Vec::with_capacity(request.mutations.len());
    for mutation in request.mutations {
        if !mutation.payload.is_object() {
            return Err(ApiError::bad_request(
                "El contenido de una entidad debe ser un objeto JSON.",
            ));
        }
        results.push(apply_mutation(&mut transaction, &device, mutation).await?);
    }
    let server_cursor: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(cursor), 0) FROM geecko.sync_changes WHERE workshop_id = $1",
    )
    .bind(device.workshop_id)
    .fetch_one(&mut *transaction)
    .await
    .map_err(ApiError::internal)?;
    transaction.commit().await.map_err(ApiError::internal)?;
    Ok(Json(PushResponse {
        protocol_version: PROTOCOL_VERSION,
        server_cursor,
        results,
    }))
}

async fn apply_mutation(
    transaction: &mut Transaction<'_, Postgres>,
    device: &DeviceContext,
    mutation: geecko_sync_protocol::SyncMutation,
) -> ApiResult<MutationResult> {
    if mutation.base_revision < 0 {
        return Err(ApiError::bad_request(
            "La revisión base no puede ser negativa.",
        ));
    }
    if let Some(row) = sqlx::query(
        "SELECT result_status, server_revision, change_cursor
         FROM geecko.sync_operations
         WHERE operation_id = $1 AND workshop_id = $2",
    )
    .bind(mutation.operation_id)
    .bind(device.workshop_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(ApiError::internal)?
    {
        let status: String = row.get("result_status");
        return Ok(MutationResult {
            operation_id: mutation.operation_id,
            entity_id: mutation.entity_id,
            status: if status == "applied" {
                MutationStatus::Duplicate
            } else {
                MutationStatus::Conflict
            },
            server_revision: row.get("server_revision"),
            change_cursor: row.get("change_cursor"),
            server_record: None,
        });
    }

    let entity_type = mutation.entity.as_str();
    let current = sqlx::query(
        "SELECT revision, payload
         FROM geecko.sync_records
         WHERE workshop_id = $1 AND entity_type = $2 AND entity_id = $3
         FOR UPDATE",
    )
    .bind(device.workshop_id)
    .bind(entity_type)
    .bind(mutation.entity_id)
    .fetch_optional(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;
    let current_revision = current
        .as_ref()
        .map(|row| row.get::<i64, _>("revision"))
        .unwrap_or(0);

    if current_revision != mutation.base_revision {
        let server_record = current.map(|row| row.get::<Value, _>("payload"));
        sqlx::query(
            "INSERT INTO geecko.sync_operations
               (operation_id, workshop_id, device_id, entity_type, entity_id,
                result_status, server_revision)
             VALUES ($1, $2, $3, $4, $5, 'conflict', $6)",
        )
        .bind(mutation.operation_id)
        .bind(device.workshop_id)
        .bind(device.id)
        .bind(entity_type)
        .bind(mutation.entity_id)
        .bind(current_revision)
        .execute(&mut **transaction)
        .await
        .map_err(ApiError::internal)?;
        return Ok(MutationResult {
            operation_id: mutation.operation_id,
            entity_id: mutation.entity_id,
            status: MutationStatus::Conflict,
            server_revision: current_revision,
            change_cursor: None,
            server_record,
        });
    }

    let next_revision = current_revision + 1;
    let deleted_at: Option<String> = mutation.deleted.then(|| mutation.client_timestamp.clone());
    sqlx::query(
        "INSERT INTO geecko.sync_records
           (workshop_id, entity_type, entity_id, revision, payload, origin_device_id, deleted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7::timestamptz)
         ON CONFLICT (workshop_id, entity_type, entity_id) DO UPDATE SET
           revision = EXCLUDED.revision,
           payload = EXCLUDED.payload,
           origin_device_id = EXCLUDED.origin_device_id,
           deleted_at = EXCLUDED.deleted_at,
           updated_at = clock_timestamp()",
    )
    .bind(device.workshop_id)
    .bind(entity_type)
    .bind(mutation.entity_id)
    .bind(next_revision)
    .bind(&mutation.payload)
    .bind(device.id)
    .bind(deleted_at)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;

    let change_cursor: i64 = sqlx::query_scalar(
        "INSERT INTO geecko.sync_changes
           (workshop_id, operation_id, entity_type, entity_id, revision,
            payload, origin_device_id, deleted_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7,
                 CASE WHEN $8 THEN clock_timestamp() ELSE NULL END)
         RETURNING cursor",
    )
    .bind(device.workshop_id)
    .bind(mutation.operation_id)
    .bind(entity_type)
    .bind(mutation.entity_id)
    .bind(next_revision)
    .bind(&mutation.payload)
    .bind(device.id)
    .bind(mutation.deleted)
    .fetch_one(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;

    sqlx::query(
        "INSERT INTO geecko.sync_operations
           (operation_id, workshop_id, device_id, entity_type, entity_id,
            result_status, server_revision, change_cursor)
         VALUES ($1, $2, $3, $4, $5, 'applied', $6, $7)",
    )
    .bind(mutation.operation_id)
    .bind(device.workshop_id)
    .bind(device.id)
    .bind(entity_type)
    .bind(mutation.entity_id)
    .bind(next_revision)
    .bind(change_cursor)
    .execute(&mut **transaction)
    .await
    .map_err(ApiError::internal)?;

    Ok(MutationResult {
        operation_id: mutation.operation_id,
        entity_id: mutation.entity_id,
        status: MutationStatus::Applied,
        server_revision: next_revision,
        change_cursor: Some(change_cursor),
        server_record: None,
    })
}

async fn pull(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PullQuery>,
) -> ApiResult<Json<PullResponse>> {
    let device = authenticate(&state.pool, &headers).await?;
    if query.cursor < 0 {
        return Err(ApiError::bad_request("El cursor no puede ser negativo."));
    }
    let limit = query.limit.clamp(1, MAX_PULL_LIMIT);
    let rows = sqlx::query(
        "SELECT cursor, entity_type, entity_id, revision, payload,
                origin_device_id, deleted_at IS NOT NULL AS deleted,
                changed_at::text AS server_timestamp
         FROM geecko.sync_changes
         WHERE workshop_id = $1 AND cursor > $2
         ORDER BY cursor
         LIMIT $3",
    )
    .bind(device.workshop_id)
    .bind(query.cursor)
    .bind(i64::from(limit) + 1)
    .fetch_all(&state.pool)
    .await
    .map_err(ApiError::internal)?;
    let has_more = rows.len() > limit as usize;
    let mut changes = Vec::with_capacity(rows.len().min(limit as usize));
    for row in rows.into_iter().take(limit as usize) {
        let entity_name: String = row.get("entity_type");
        let entity = parse_entity(&entity_name).ok_or_else(|| {
            ApiError::internal(format!("Entidad desconocida en PostgreSQL: {entity_name}"))
        })?;
        changes.push(ServerChange {
            cursor: row.get("cursor"),
            entity,
            entity_id: row.get("entity_id"),
            revision: row.get("revision"),
            deleted: row.get("deleted"),
            payload: row.get("payload"),
            origin_device_id: row.get("origin_device_id"),
            server_timestamp: row.get("server_timestamp"),
        });
    }
    let cursor = changes
        .last()
        .map(|change| change.cursor)
        .unwrap_or(query.cursor);
    Ok(Json(PullResponse {
        protocol_version: PROTOCOL_VERSION,
        cursor,
        has_more,
        changes,
    }))
}

fn parse_entity(value: &str) -> Option<EntityKind> {
    Some(match value {
        "customer" => EntityKind::Customer,
        "asset" => EntityKind::Asset,
        "work_order" => EntityKind::WorkOrder,
        "work_order_task" => EntityKind::WorkOrderTask,
        "work_order_event" => EntityKind::WorkOrderEvent,
        "work_order_accessory" => EntityKind::WorkOrderAccessory,
        "quote_item" => EntityKind::QuoteItem,
        "order_part" => EntityKind::OrderPart,
        "order_labor" => EntityKind::OrderLabor,
        "order_payment" => EntityKind::OrderPayment,
        "order_warranty" => EntityKind::OrderWarranty,
        "warranty_claim" => EntityKind::WarrantyClaim,
        "order_photo" => EntityKind::OrderPhoto,
        "inventory_item" => EntityKind::InventoryItem,
        "inventory_movement" => EntityKind::InventoryMovement,
        "purchase_report" => EntityKind::PurchaseReport,
        "purchase_receipt" => EntityKind::PurchaseReceipt,
        "sale" => EntityKind::Sale,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_every_entity_exposed_by_the_protocol() {
        let entities = [
            EntityKind::Customer,
            EntityKind::Asset,
            EntityKind::WorkOrder,
            EntityKind::WorkOrderTask,
            EntityKind::WorkOrderEvent,
            EntityKind::WorkOrderAccessory,
            EntityKind::QuoteItem,
            EntityKind::OrderPart,
            EntityKind::OrderLabor,
            EntityKind::OrderPayment,
            EntityKind::OrderWarranty,
            EntityKind::WarrantyClaim,
            EntityKind::OrderPhoto,
            EntityKind::InventoryItem,
            EntityKind::InventoryMovement,
            EntityKind::PurchaseReport,
            EntityKind::PurchaseReceipt,
            EntityKind::Sale,
        ];
        for entity in entities {
            assert_eq!(parse_entity(entity.as_str()), Some(entity));
        }
        assert_eq!(parse_entity("unknown"), None);
    }
}
