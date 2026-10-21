mod auth;
mod backup;
mod database;
mod domain;
mod sync;

use domain::{
    Asset, AuthStatus, BackupFileSummary, BackupOverview, CancelOrderPaymentInput, CancelSaleInput,
    CommandError, CreateAssetInput, CreateOrderInput, CreateOrderPaymentInput,
    CreatePurchaseReportInput, CreateSaleInput, CreateWarrantyReturnInput, Customer, Dashboard,
    InventoryItem, InventoryMovement, LoginInput, OrderFilters, OrderPayment, OrderPhoto, OrderQr,
    OrderWarranty, PurchaseReceipt, PurchaseReport, QuoteDecisionInput, ReceivePurchaseInput,
    ResetUserPasswordInput, ResolveWarrantyClaimInput, RestoreBackupInput, RestoreBackupResult,
    Sale, SaveCustomerInput, SaveDiagnosisQuoteInput, SaveInventoryItemInput, SaveOrderPhotoInput,
    SaveOrderResourcesInput, SaveUserInput, SaveWarrantyInput, SessionUser, SetUserActiveInput,
    UpdateOrderInput, UserAccount, UserActivity, WorkOrder, WorkshopStatistics,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, RwLock, RwLockReadGuard};
use tauri::{AppHandle, Manager, State};

struct AppState {
    database_path: PathBuf,
    database_access: RwLock<()>,
    session: Mutex<Option<SessionUser>>,
    sync_running: AtomicBool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInfo {
    platform: &'static str,
    mobile: bool,
    app_version: String,
    storage_mode: &'static str,
}

#[tauri::command]
fn get_runtime_info(app: AppHandle) -> RuntimeInfo {
    let mobile = cfg!(target_os = "android") || cfg!(target_os = "ios");
    RuntimeInfo {
        platform: if cfg!(target_os = "android") {
            "android"
        } else if cfg!(target_os = "ios") {
            "ios"
        } else {
            "desktop"
        },
        mobile,
        app_version: app.package_info().version.to_string(),
        storage_mode: "local",
    }
}

#[tauri::command]
fn get_sync_status(state: State<'_, AppState>) -> Result<sync::LocalSyncStatus, CommandError> {
    current_user(&state)?;
    let connection = connection(&state)?;
    sync::status(&connection)
}

struct SyncRunGuard<'a>(&'a AtomicBool);

impl Drop for SyncRunGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[tauri::command(async)]
async fn configure_sync(
    state: State<'_, AppState>,
    mut input: sync::ConfigureSyncInput,
) -> Result<sync::LocalSyncStatus, CommandError> {
    require_roles(&state, &["admin"])?;
    if input.token.trim().is_empty() {
        let connection = database::open(&state.database_path)?;
        if let Some(current) = sync::credentials(&connection)? {
            input.token = current.token;
        }
    }
    let credentials = sync::validate_configuration(input)?;
    verify_sync_server(&credentials).await?;
    let connection = database::open(&state.database_path)?;
    sync::save_configuration(&connection, &credentials)
}

#[tauri::command(async)]
async fn sync_now(state: State<'_, AppState>) -> Result<sync::SyncRunResult, CommandError> {
    current_user(&state)?;
    if state
        .sync_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        let connection = database::open(&state.database_path)?;
        return Ok(sync::SyncRunResult {
            pushed: 0,
            pulled: 0,
            remote_changes: 0,
            conflicts: 0,
            status: sync::status(&connection)?,
        });
    }
    let _guard = SyncRunGuard(&state.sync_running);
    let database_path = state.database_path.clone();
    let credentials = {
        let connection = database::open(&database_path)?;
        sync::credentials(&connection)?.ok_or_else(|| {
            CommandError::message("Configura primero el servidor de sincronizaciÃ³n.")
        })?
    };
    let result = run_sync_cycle(&database_path, &credentials).await;
    if let Err(error) = &result {
        if let Ok(connection) = database::open(&database_path) {
            let _ = sync::record_error(&connection, &error.error);
        }
    }
    result
}

async fn verify_sync_server(credentials: &sync::RemoteCredentials) -> Result<(), CommandError> {
    let client = sync_http_client()?;
    let health = client
        .get(format!("{}/health", credentials.server_url))
        .send()
        .await
        .map_err(sync_network_error)?;
    let health: geecko_sync_protocol::HealthResponse = decode_sync_response(health).await?;
    if health.protocol_version != geecko_sync_protocol::PROTOCOL_VERSION {
        return Err(CommandError::message(
            "El servidor usa una versiÃ³n de sincronizaciÃ³n incompatible.",
        ));
    }
    let response = client
        .get(format!(
            "{}/v1/sync/pull?cursor=0&limit=1",
            credentials.server_url
        ))
        .bearer_auth(&credentials.token)
        .send()
        .await
        .map_err(sync_network_error)?;
    let _: geecko_sync_protocol::PullResponse = decode_sync_response(response).await?;
    Ok(())
}

async fn run_sync_cycle(
    database_path: &std::path::Path,
    credentials: &sync::RemoteCredentials,
) -> Result<sync::SyncRunResult, CommandError> {
    let client = sync_http_client()?;
    let mut pushed = 0;
    let mut pulled = 0;
    let mut remote_changes = 0;
    let mut conflicts = 0;

    for _ in 0..4 {
        let request = {
            let connection = database::open(database_path)?;
            sync::prepare_push(&connection)?
        };
        let Some(request) = request else { break };
        let sent = request.mutations.len();
        let response = client
            .post(format!("{}/v1/sync/push", credentials.server_url))
            .bearer_auth(&credentials.token)
            .json(&request)
            .send()
            .await
            .map_err(sync_network_error)?;
        let response: geecko_sync_protocol::PushResponse = decode_sync_response(response).await?;
        let (applied, found_conflicts) = {
            let mut connection = database::open(database_path)?;
            sync::apply_push_response(&mut connection, &response)?
        };
        pushed += applied;
        conflicts += found_conflicts;
        if sent < geecko_sync_protocol::MAX_PUSH_MUTATIONS && found_conflicts == 0 {
            break;
        }
    }

    loop {
        let cursor = {
            let connection = database::open(database_path)?;
            sync::cursor(&connection)?
        };
        let response = client
            .get(format!(
                "{}/v1/sync/pull?cursor={cursor}&limit={}",
                credentials.server_url,
                geecko_sync_protocol::DEFAULT_PULL_LIMIT
            ))
            .bearer_auth(&credentials.token)
            .send()
            .await
            .map_err(sync_network_error)?;
        let response: geecko_sync_protocol::PullResponse = decode_sync_response(response).await?;
        pulled += response.changes.len();
        let has_more = response.has_more;
        let (applied, found_conflicts) = {
            let mut connection = database::open(database_path)?;
            sync::apply_pull_response(&mut connection, credentials.device_id, &response)?
        };
        remote_changes += applied;
        conflicts += found_conflicts;
        if !has_more {
            break;
        }
    }

    let connection = database::open(database_path)?;
    Ok(sync::SyncRunResult {
        pushed,
        pulled,
        remote_changes,
        conflicts,
        status: sync::status(&connection)?,
    })
}

fn sync_http_client() -> Result<reqwest::Client, CommandError> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(8))
        .timeout(std::time::Duration::from_secs(45))
        .user_agent(concat!("Geecko/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(sync_network_error)
}

async fn decode_sync_response<T: DeserializeOwned>(
    response: reqwest::Response,
) -> Result<T, CommandError> {
    let status = response.status();
    let body = response.text().await.map_err(sync_network_error)?;
    if !status.is_success() {
        let message = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| value.get("error")?.as_str().map(str::to_owned))
            .unwrap_or_else(|| format!("El servidor respondiÃ³ con el estado {status}."));
        return Err(CommandError::message(message));
    }
    serde_json::from_str(&body)
        .map_err(|_| CommandError::message("El servidor devolviÃ³ una respuesta invÃ¡lida."))
}

fn sync_network_error(error: impl std::fmt::Display) -> CommandError {
    eprintln!("Error de red durante la sincronizaciÃ³n: {error}");
    CommandError::message(
        "No fue posible conectar con el servidor de Geecko. Revisa la direcciÃ³n y tu conexiÃ³n.",
    )
}

struct LockedConnection<'a> {
    connection: rusqlite::Connection,
    _guard: RwLockReadGuard<'a, ()>,
}

impl Deref for LockedConnection<'_> {
    type Target = rusqlite::Connection;

    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for LockedConnection<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

fn unchecked_connection<'a>(
    state: &'a State<'_, AppState>,
) -> Result<LockedConnection<'a>, CommandError> {
    let guard = state
        .database_access
        .read()
        .map_err(|_| CommandError::message("El acceso a los datos locales quedó bloqueado."))?;
    Ok(LockedConnection {
        connection: database::open(&state.database_path)?,
        _guard: guard,
    })
}

fn current_user(state: &State<'_, AppState>) -> Result<SessionUser, CommandError> {
    state
        .session
        .lock()
        .map_err(|_| CommandError::message("La sesión local quedó bloqueada."))?
        .clone()
        .ok_or_else(|| CommandError::message("Inicia sesión para continuar."))
}

fn require_roles(state: &State<'_, AppState>, roles: &[&str]) -> Result<SessionUser, CommandError> {
    let user = current_user(state)?;
    if !roles.contains(&user.role.as_str()) {
        return Err(CommandError::message(
            "Tu cuenta no tiene permiso para realizar esta acción.",
        ));
    }
    Ok(user)
}

fn connection<'a>(state: &'a State<'_, AppState>) -> Result<LockedConnection<'a>, CommandError> {
    current_user(state)?;
    unchecked_connection(state)
}

fn log_activity(
    state: &State<'_, AppState>,
    user: &SessionUser,
    action: &str,
    entity_type: &str,
    entity_id: Option<i64>,
    summary: &str,
) {
    if let Ok(connection) = connection(state) {
        if let Err(error) =
            auth::record_activity(&connection, user, action, entity_type, entity_id, summary)
        {
            eprintln!(
                "No fue posible registrar la actividad local: {}",
                error.error
            );
        }
    }
}

#[tauri::command(async)]
fn get_auth_status(state: State<'_, AppState>) -> Result<AuthStatus, CommandError> {
    let requires_setup = auth::requires_setup(&*unchecked_connection(&state)?)?;
    let current_user = state
        .session
        .lock()
        .map_err(|_| CommandError::message("La sesión local quedó bloqueada."))?
        .clone();
    Ok(AuthStatus {
        requires_setup,
        current_user,
    })
}

#[tauri::command(async)]
fn create_initial_admin(
    state: State<'_, AppState>,
    mut input: SaveUserInput,
) -> Result<SessionUser, CommandError> {
    let connection = unchecked_connection(&state)?;
    if !auth::requires_setup(&connection)? {
        return Err(CommandError::message(
            "La cuenta administradora inicial ya fue creada.",
        ));
    }
    input.role = "admin".into();
    let data = domain::validate_user(input)?;
    let account = auth::create_user(&connection, data)?;
    let user = SessionUser {
        id: account.id,
        username: account.username,
        display_name: account.display_name,
        role: account.role,
    };
    auth::record_activity(
        &connection,
        &user,
        "initial_setup",
        "user",
        Some(user.id),
        "Cuenta administradora inicial creada",
    )?;
    *state
        .session
        .lock()
        .map_err(|_| CommandError::message("La sesión local quedó bloqueada."))? =
        Some(user.clone());
    Ok(user)
}

#[tauri::command(async)]
fn login(state: State<'_, AppState>, input: LoginInput) -> Result<SessionUser, CommandError> {
    let (username, password) = domain::validate_login(input)?;
    let connection = unchecked_connection(&state)?;
    let user = auth::authenticate(&connection, &username, &password)?;
    auth::record_activity(
        &connection,
        &user,
        "login",
        "session",
        None,
        "Inicio de sesión",
    )?;
    *state
        .session
        .lock()
        .map_err(|_| CommandError::message("La sesión local quedó bloqueada."))? =
        Some(user.clone());
    Ok(user)
}

#[tauri::command(async)]
fn logout(state: State<'_, AppState>) -> Result<(), CommandError> {
    let user = current_user(&state)?;
    if let Ok(connection) = unchecked_connection(&state) {
        let _ = auth::record_activity(
            &connection,
            &user,
            "logout",
            "session",
            None,
            "Cierre de sesión",
        );
    }
    *state
        .session
        .lock()
        .map_err(|_| CommandError::message("La sesión local quedó bloqueada."))? = None;
    Ok(())
}

#[tauri::command(async)]
fn get_dashboard(state: State<'_, AppState>) -> Result<Dashboard, CommandError> {
    database::get_dashboard(&*connection(&state)?)
}

#[tauri::command(async)]
fn list_orders(
    state: State<'_, AppState>,
    filters: OrderFilters,
) -> Result<Vec<WorkOrder>, CommandError> {
    database::list_orders(&*connection(&state)?, &filters)
}

#[tauri::command(async)]
fn get_order(state: State<'_, AppState>, id: i64) -> Result<WorkOrder, CommandError> {
    database::get_order(&*connection(&state)?, id, true)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))
}

#[tauri::command(async)]
fn create_order(
    state: State<'_, AppState>,
    input: CreateOrderInput,
) -> Result<WorkOrder, CommandError> {
    let actor = require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_order(input)?;
    let order = database::create_order(&mut *connection(&state)?, data)?;
    log_activity(
        &state,
        &actor,
        "order_created",
        "work_order",
        Some(order.id),
        &format!("Orden {} creada", order.order_number),
    );
    Ok(order)
}

#[tauri::command(async)]
fn update_order(
    state: State<'_, AppState>,
    id: i64,
    changes: UpdateOrderInput,
) -> Result<WorkOrder, CommandError> {
    let actor = require_roles(&state, &["admin", "reception", "technician"])?;
    let patch = domain::validate_order_patch(changes)?;
    let order = database::update_order(&mut *connection(&state)?, id, patch)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))?;
    log_activity(
        &state,
        &actor,
        "order_updated",
        "work_order",
        Some(id),
        &format!("Orden {} actualizada", order.order_number),
    );
    Ok(order)
}

#[tauri::command(async)]
fn save_diagnosis_quote(
    state: State<'_, AppState>,
    id: i64,
    input: SaveDiagnosisQuoteInput,
) -> Result<WorkOrder, CommandError> {
    let actor = require_roles(&state, &["admin", "technician"])?;
    let quote = domain::validate_diagnosis_quote(input)?;
    let order = database::save_diagnosis_quote(&mut *connection(&state)?, id, quote)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))?;
    log_activity(
        &state,
        &actor,
        "diagnosis_saved",
        "work_order",
        Some(id),
        &format!("Diagnóstico actualizado en {}", order.order_number),
    );
    Ok(order)
}

#[tauri::command(async)]
fn set_quote_decision(
    state: State<'_, AppState>,
    id: i64,
    input: QuoteDecisionInput,
) -> Result<WorkOrder, CommandError> {
    let decision = domain::validate_quote_decision(input)?;
    let actor = if decision.decision == "pending" {
        require_roles(&state, &["admin", "reception", "technician"])?
    } else {
        require_roles(&state, &["admin", "reception"])?
    };
    let order = database::set_quote_decision(&mut *connection(&state)?, id, decision)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))?;
    log_activity(
        &state,
        &actor,
        "quote_decision",
        "work_order",
        Some(id),
        &format!(
            "Cotización de {}: {}",
            order.order_number, order.quote_status
        ),
    );
    Ok(order)
}

#[tauri::command(async)]
fn save_order_resources(
    state: State<'_, AppState>,
    id: i64,
    input: SaveOrderResourcesInput,
) -> Result<WorkOrder, CommandError> {
    let actor = require_roles(&state, &["admin", "technician"])?;
    let resources = domain::validate_order_resources(input)?;
    let order = database::save_order_resources(&mut *connection(&state)?, id, resources)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))?;
    log_activity(
        &state,
        &actor,
        "resources_saved",
        "work_order",
        Some(id),
        &format!(
            "Repuestos y mano de obra actualizados en {}",
            order.order_number
        ),
    );
    Ok(order)
}

#[tauri::command(async)]
fn create_order_payment(
    state: State<'_, AppState>,
    order_id: i64,
    input: CreateOrderPaymentInput,
) -> Result<OrderPayment, CommandError> {
    let actor = require_roles(&state, &["admin", "reception"])?;
    let payment = domain::validate_order_payment(input)?;
    let payment = database::create_order_payment(&mut *connection(&state)?, order_id, payment)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))?;
    log_activity(
        &state,
        &actor,
        "payment_received",
        "work_order",
        Some(order_id),
        &format!("Pago {} registrado", payment.receipt_number),
    );
    Ok(payment)
}

#[tauri::command(async)]
fn cancel_order_payment(
    state: State<'_, AppState>,
    order_id: i64,
    payment_id: i64,
    input: CancelOrderPaymentInput,
) -> Result<OrderPayment, CommandError> {
    let actor = require_roles(&state, &["admin", "reception"])?;
    let note = domain::validate_payment_cancellation(input)?;
    let payment =
        database::cancel_order_payment(&mut *connection(&state)?, order_id, payment_id, note)?
            .ok_or_else(|| CommandError::message("No encontramos ese pago."))?;
    log_activity(
        &state,
        &actor,
        "payment_cancelled",
        "work_order",
        Some(order_id),
        &format!("Pago {} anulado", payment.receipt_number),
    );
    Ok(payment)
}

#[tauri::command(async)]
fn save_order_warranty(
    state: State<'_, AppState>,
    order_id: i64,
    input: SaveWarrantyInput,
) -> Result<OrderWarranty, CommandError> {
    require_roles(&state, &["admin", "technician"])?;
    let warranty = domain::validate_warranty(input)?;
    database::save_order_warranty(&mut *connection(&state)?, order_id, warranty)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))
}

#[tauri::command(async)]
fn create_warranty_return(
    state: State<'_, AppState>,
    original_order_id: i64,
    input: CreateWarrantyReturnInput,
) -> Result<WorkOrder, CommandError> {
    require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_warranty_return(input)?;
    database::create_warranty_return(&mut *connection(&state)?, original_order_id, data)?
        .ok_or_else(|| CommandError::message("No encontramos la orden original."))
}

#[tauri::command(async)]
fn resolve_warranty_claim(
    state: State<'_, AppState>,
    return_order_id: i64,
    input: ResolveWarrantyClaimInput,
) -> Result<WorkOrder, CommandError> {
    require_roles(&state, &["admin", "technician"])?;
    let decision = domain::validate_warranty_decision(input)?;
    database::resolve_warranty_claim(&mut *connection(&state)?, return_order_id, decision)?
        .ok_or_else(|| CommandError::message("No encontramos la orden de regreso."))
}

#[tauri::command(async)]
fn set_task_completed(
    state: State<'_, AppState>,
    order_id: i64,
    task_id: i64,
    completed: bool,
) -> Result<WorkOrder, CommandError> {
    require_roles(&state, &["admin", "technician"])?;
    database::set_task_completed(&*connection(&state)?, order_id, task_id, completed)?
        .ok_or_else(|| CommandError::message("No encontramos esa tarea."))
}

#[tauri::command(async)]
fn list_assets(state: State<'_, AppState>) -> Result<Vec<Asset>, CommandError> {
    database::list_assets(&*connection(&state)?)
}

#[tauri::command(async)]
fn create_asset(
    state: State<'_, AppState>,
    input: CreateAssetInput,
) -> Result<Asset, CommandError> {
    require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_asset(input)?;
    database::create_asset(&*connection(&state)?, data)
}

#[tauri::command(async)]
fn list_customers(state: State<'_, AppState>) -> Result<Vec<Customer>, CommandError> {
    database::list_customers(&*connection(&state)?)
}

#[tauri::command(async)]
fn create_customer(
    state: State<'_, AppState>,
    input: SaveCustomerInput,
) -> Result<Customer, CommandError> {
    require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_customer(input)?;
    database::create_customer(&*connection(&state)?, data)
}

#[tauri::command(async)]
fn update_customer(
    state: State<'_, AppState>,
    id: i64,
    input: SaveCustomerInput,
) -> Result<Customer, CommandError> {
    require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_customer(input)?;
    database::update_customer(&*connection(&state)?, id, data)?
        .ok_or_else(|| CommandError::message("No encontramos ese cliente."))
}

#[tauri::command(async)]
fn list_inventory_items(state: State<'_, AppState>) -> Result<Vec<InventoryItem>, CommandError> {
    database::list_inventory_items(&*connection(&state)?)
}

#[tauri::command(async)]
fn create_inventory_item(
    state: State<'_, AppState>,
    input: SaveInventoryItemInput,
) -> Result<InventoryItem, CommandError> {
    require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_inventory_item(input)?;
    database::create_inventory_item(&mut *connection(&state)?, data)
}

#[tauri::command(async)]
fn update_inventory_item(
    state: State<'_, AppState>,
    id: i64,
    input: SaveInventoryItemInput,
) -> Result<InventoryItem, CommandError> {
    require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_inventory_item(input)?;
    database::update_inventory_item(&mut *connection(&state)?, id, data)?
        .ok_or_else(|| CommandError::message("No encontramos ese producto."))
}

#[tauri::command(async)]
fn create_purchase_report(
    state: State<'_, AppState>,
    input: CreatePurchaseReportInput,
) -> Result<PurchaseReport, CommandError> {
    let actor = require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_purchase_report(input)?;
    let report = database::create_purchase_report(&mut *connection(&state)?, data)?;
    log_activity(
        &state,
        &actor,
        "purchase_report_created",
        "purchase_report",
        Some(report.id),
        &format!("Reporte {} creado", report.report_number),
    );
    Ok(report)
}

#[tauri::command(async)]
fn list_purchase_reports(state: State<'_, AppState>) -> Result<Vec<PurchaseReport>, CommandError> {
    database::list_purchase_reports(&*connection(&state)?)
}

#[tauri::command(async)]
fn receive_purchase(
    state: State<'_, AppState>,
    report_id: i64,
    input: ReceivePurchaseInput,
) -> Result<PurchaseReceipt, CommandError> {
    let actor = require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_purchase_receipt(input)?;
    let receipt = database::receive_purchase(&mut *connection(&state)?, report_id, data)?;
    log_activity(
        &state,
        &actor,
        "purchase_received",
        "purchase_report",
        Some(report_id),
        &format!("Recepcion {} registrada", receipt.receipt_number),
    );
    Ok(receipt)
}

#[tauri::command(async)]
fn list_purchase_receipts(
    state: State<'_, AppState>,
) -> Result<Vec<PurchaseReceipt>, CommandError> {
    database::list_purchase_receipts(&*connection(&state)?)
}

#[tauri::command(async)]
fn list_inventory_movements(
    state: State<'_, AppState>,
) -> Result<Vec<InventoryMovement>, CommandError> {
    database::list_inventory_movements(&*connection(&state)?)
}

#[tauri::command(async)]
fn create_sale(state: State<'_, AppState>, input: CreateSaleInput) -> Result<Sale, CommandError> {
    let actor = require_roles(&state, &["admin", "reception"])?;
    let data = domain::validate_sale(input)?;
    let sale = database::create_sale(&mut *connection(&state)?, data)?;
    log_activity(
        &state,
        &actor,
        "sale_created",
        "sale",
        Some(sale.id),
        &format!("Venta {} registrada", sale.sale_number),
    );
    Ok(sale)
}

#[tauri::command(async)]
fn list_sales(state: State<'_, AppState>) -> Result<Vec<Sale>, CommandError> {
    database::list_sales(&*connection(&state)?)
}

#[tauri::command(async)]
fn cancel_sale(
    state: State<'_, AppState>,
    id: i64,
    input: CancelSaleInput,
) -> Result<Sale, CommandError> {
    let actor = require_roles(&state, &["admin", "reception"])?;
    let sale = database::cancel_sale(
        &mut *connection(&state)?,
        id,
        domain::validate_cancellation_note(input),
    )?
    .ok_or_else(|| CommandError::message("No encontramos esa venta."))?;
    log_activity(
        &state,
        &actor,
        "sale_cancelled",
        "sale",
        Some(id),
        &format!("Venta {} anulada", sale.sale_number),
    );
    Ok(sale)
}

#[tauri::command(async)]
fn list_users(state: State<'_, AppState>) -> Result<Vec<UserAccount>, CommandError> {
    require_roles(&state, &["admin"])?;
    auth::list_users(&*connection(&state)?)
}

#[tauri::command(async)]
fn create_user(
    state: State<'_, AppState>,
    input: SaveUserInput,
) -> Result<UserAccount, CommandError> {
    let actor = require_roles(&state, &["admin"])?;
    let data = domain::validate_user(input)?;
    let connection = connection(&state)?;
    let user = auth::create_user(&connection, data)?;
    auth::record_activity(
        &connection,
        &actor,
        "user_created",
        "user",
        Some(user.id),
        &format!("Cuenta creada para {}", user.display_name),
    )?;
    Ok(user)
}

#[tauri::command(async)]
fn set_user_active(
    state: State<'_, AppState>,
    id: i64,
    input: SetUserActiveInput,
) -> Result<UserAccount, CommandError> {
    let actor = require_roles(&state, &["admin"])?;
    let connection = connection(&state)?;
    let user = auth::set_user_active(&connection, id, input.active, actor.id)?
        .ok_or_else(|| CommandError::message("No encontramos ese usuario."))?;
    auth::record_activity(
        &connection,
        &actor,
        if user.active {
            "user_enabled"
        } else {
            "user_disabled"
        },
        "user",
        Some(user.id),
        &format!(
            "Cuenta de {} {}",
            user.display_name,
            if user.active {
                "activada"
            } else {
                "desactivada"
            }
        ),
    )?;
    Ok(user)
}

#[tauri::command(async)]
fn reset_user_password(
    state: State<'_, AppState>,
    id: i64,
    input: ResetUserPasswordInput,
) -> Result<UserAccount, CommandError> {
    let actor = require_roles(&state, &["admin"])?;
    let password = domain::validate_password(input.password)?;
    let connection = connection(&state)?;
    let user = auth::reset_password(&connection, id, &password)?
        .ok_or_else(|| CommandError::message("No encontramos ese usuario."))?;
    auth::record_activity(
        &connection,
        &actor,
        "password_reset",
        "user",
        Some(user.id),
        &format!("Clave restablecida para {}", user.display_name),
    )?;
    Ok(user)
}

#[tauri::command(async)]
fn list_user_activity(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<UserActivity>, CommandError> {
    require_roles(&state, &["admin"])?;
    auth::list_activity(&*connection(&state)?, limit.unwrap_or(100))
}

#[tauri::command(async)]
fn add_order_photo(
    state: State<'_, AppState>,
    order_id: i64,
    input: SaveOrderPhotoInput,
) -> Result<OrderPhoto, CommandError> {
    let actor = require_roles(&state, &["admin", "technician"])?;
    let photo = domain::validate_order_photo(input)?;
    let connection = connection(&state)?;
    let result = database::add_order_photo(&connection, order_id, photo, &actor.display_name)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))?;
    auth::record_activity(
        &connection,
        &actor,
        "photo_added",
        "work_order",
        Some(order_id),
        &format!("Fotografía agregada a la orden #{order_id}"),
    )?;
    Ok(result)
}

#[tauri::command(async)]
fn get_order_photo_data(
    state: State<'_, AppState>,
    photo_id: i64,
) -> Result<Vec<u8>, CommandError> {
    database::get_order_photo_data(&*connection(&state)?, photo_id)?
        .ok_or_else(|| CommandError::message("No encontramos esa fotografía."))
}

#[tauri::command(async)]
fn delete_order_photo(
    state: State<'_, AppState>,
    order_id: i64,
    photo_id: i64,
) -> Result<(), CommandError> {
    let actor = require_roles(&state, &["admin", "technician"])?;
    let connection = connection(&state)?;
    if !database::delete_order_photo(&connection, order_id, photo_id, &actor.display_name)? {
        return Err(CommandError::message("No encontramos esa fotografía."));
    }
    auth::record_activity(
        &connection,
        &actor,
        "photo_deleted",
        "work_order",
        Some(order_id),
        &format!("Fotografía eliminada de la orden #{order_id}"),
    )?;
    Ok(())
}

#[tauri::command(async)]
fn get_statistics(
    state: State<'_, AppState>,
    range_days: i64,
) -> Result<WorkshopStatistics, CommandError> {
    let range = domain::validate_statistics_range(range_days)?;
    database::get_statistics(&*connection(&state)?, range)
}

#[tauri::command(async)]
fn generate_order_qr(state: State<'_, AppState>, order_id: i64) -> Result<OrderQr, CommandError> {
    let order = database::get_order(&*connection(&state)?, order_id, false)?
        .ok_or_else(|| CommandError::message("No encontramos esa orden."))?;
    let code = qrcode::QrCode::new(order.order_number.as_bytes()).map_err(|error| {
        eprintln!("No fue posible generar el QR: {error}");
        CommandError::internal()
    })?;
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(180, 180)
        .dark_color(qrcode::render::svg::Color("#092c1e"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();
    Ok(OrderQr {
        payload: order.order_number,
        svg,
    })
}

fn backup_directory(app: &AppHandle) -> Result<PathBuf, CommandError> {
    #[cfg(mobile)]
    let base_directory = app.path().app_data_dir().map_err(|error| {
        eprintln!("No fue posible resolver la carpeta privada de respaldos: {error}");
        CommandError::internal()
    })?;

    #[cfg(not(mobile))]
    let base_directory = app
        .path()
        .document_dir()
        .or_else(|_| app.path().app_data_dir())
        .map_err(|error| {
            eprintln!("No fue posible resolver la carpeta de respaldos: {error}");
            CommandError::internal()
        })?;
    Ok(base_directory.join("Geecko Backups"))
}

#[tauri::command(async)]
fn list_backups(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BackupOverview, CommandError> {
    current_user(&state)?;
    backup::list_backups(&backup_directory(&app)?)
}

#[tauri::command(async)]
fn ensure_automatic_backup(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BackupOverview, CommandError> {
    current_user(&state)?;
    let _guard = state
        .database_access
        .read()
        .map_err(|_| CommandError::message("El acceso a los datos locales quedó bloqueado."))?;
    backup::ensure_automatic_backup(&state.database_path, &backup_directory(&app)?)
}

#[tauri::command(async)]
fn create_manual_backup(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<BackupFileSummary, CommandError> {
    current_user(&state)?;
    let _guard = state
        .database_access
        .read()
        .map_err(|_| CommandError::message("El acceso a los datos locales quedó bloqueado."))?;
    backup::create_manual_backup(&state.database_path, &backup_directory(&app)?)
}

#[tauri::command(async)]
fn restore_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    input: RestoreBackupInput,
) -> Result<RestoreBackupResult, CommandError> {
    require_roles(&state, &["admin"])?;
    let _guard = state
        .database_access
        .write()
        .map_err(|_| CommandError::message("El acceso a los datos locales quedó bloqueado."))?;
    backup::restore_backup(
        &state.database_path,
        &backup_directory(&app)?,
        &input.file_name,
        input.confirmed,
    )
}

#[tauri::command(async)]
fn export_backup(app: AppHandle, state: State<'_, AppState>) -> Result<String, CommandError> {
    let backup = database::export_all(&*connection(&state)?)?;
    let backup_directory = backup_directory(&app)?;
    fs::create_dir_all(&backup_directory)?;
    let date = backup.exported_at.get(0..10).unwrap_or("respaldo");
    let destination = backup_directory.join(format!("geecko-respaldo-{date}.json"));
    fs::write(&destination, serde_json::to_string_pretty(&backup)?)?;
    Ok(destination.to_string_lossy().into_owned())
}

fn prepare_database(app: &tauri::App) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let data_directory = app.path().app_data_dir()?;
    fs::create_dir_all(&data_directory)?;
    let database_path = data_directory.join("geecko.db");

    if !database_path.exists() {
        if let Ok(current_directory) = std::env::current_dir() {
            let legacy_path = current_directory.join("data").join("geecko.db");
            if legacy_path.is_file() {
                fs::copy(legacy_path, &database_path)?;
            }
        }
    }
    database::initialize(&database_path).map_err(|error| {
        std::io::Error::other(format!(
            "No fue posible inicializar SQLite: {}",
            error.error
        ))
    })?;
    Ok(database_path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let database_path = prepare_database(app)?;
            let setup_complete = database::open(&database_path)
                .and_then(|connection| auth::requires_setup(&connection))
                .map(|required| !required)
                .unwrap_or(false);
            if setup_complete {
                if let Ok(directory) = backup_directory(app.handle()) {
                    if let Err(error) = backup::ensure_automatic_backup(&database_path, &directory)
                    {
                        eprintln!(
                            "No fue posible crear el respaldo automático: {}",
                            error.error
                        );
                    }
                }
            }
            app.manage(AppState {
                database_path,
                database_access: RwLock::new(()),
                session: Mutex::new(None),
                sync_running: AtomicBool::new(false),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_runtime_info,
            get_sync_status,
            configure_sync,
            sync_now,
            get_auth_status,
            create_initial_admin,
            login,
            logout,
            get_dashboard,
            list_orders,
            get_order,
            create_order,
            update_order,
            save_diagnosis_quote,
            set_quote_decision,
            save_order_resources,
            create_order_payment,
            cancel_order_payment,
            save_order_warranty,
            create_warranty_return,
            resolve_warranty_claim,
            set_task_completed,
            list_assets,
            create_asset,
            list_customers,
            create_customer,
            update_customer,
            list_inventory_items,
            create_inventory_item,
            update_inventory_item,
            create_purchase_report,
            list_purchase_reports,
            receive_purchase,
            list_purchase_receipts,
            list_inventory_movements,
            create_sale,
            list_sales,
            cancel_sale,
            list_users,
            create_user,
            set_user_active,
            reset_user_password,
            list_user_activity,
            add_order_photo,
            get_order_photo_data,
            delete_order_photo,
            get_statistics,
            generate_order_qr,
            list_backups,
            ensure_automatic_backup,
            create_manual_backup,
            restore_backup,
            export_backup
        ])
        .run(tauri::generate_context!())
        .expect("error al ejecutar Geecko Maintenance");
}
