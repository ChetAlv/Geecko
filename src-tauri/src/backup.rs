use crate::database;
use crate::domain::{BackupFileSummary, BackupOverview, CommandError, RestoreBackupResult};
use rusqlite::{Connection, OpenFlags};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

pub const AUTOMATIC_RETENTION: usize = 30;
const SAFETY_RETENTION: usize = 10;

#[derive(Debug)]
struct DatabaseStats {
    version: i64,
    work_orders: i64,
    customers: i64,
    inventory_items: i64,
}

fn read_only_connection(path: &Path) -> Result<Connection, CommandError> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    connection.busy_timeout(Duration::from_secs(5))?;
    Ok(connection)
}

fn inspect_database(path: &Path, full_check: bool) -> Result<DatabaseStats, CommandError> {
    let connection = read_only_connection(path).map_err(|_| {
        CommandError::message("El archivo no puede abrirse como una base de datos de Geecko.")
    })?;
    let required_tables: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master
             WHERE type = 'table' AND name IN ('work_orders', 'customers', 'assets', 'inventory_items')",
            [],
            |row| row.get(0),
        )
        .map_err(|_| CommandError::message("El respaldo no contiene la estructura de Geecko."))?;
    if required_tables != 4 {
        return Err(CommandError::message(
            "El respaldo no contiene la estructura completa de Geecko.",
        ));
    }
    let version: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap_or(0);
    if version > database::CURRENT_DATABASE_VERSION {
        return Err(CommandError::message(format!(
            "El respaldo pertenece a una versión más reciente de Geecko ({version})."
        )));
    }
    if full_check {
        let integrity: String = connection
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .map_err(|_| CommandError::message("No fue posible verificar el respaldo."))?;
        if integrity != "ok" {
            return Err(CommandError::message(format!(
                "El respaldo está dañado: {integrity}"
            )));
        }
        let has_foreign_key_errors = connection
            .prepare("PRAGMA foreign_key_check")?
            .query([])?
            .next()?
            .is_some();
        if has_foreign_key_errors {
            return Err(CommandError::message(
                "El respaldo contiene relaciones incompletas y no es seguro restaurarlo.",
            ));
        }
    }
    Ok(DatabaseStats {
        version,
        work_orders: connection
            .query_row("SELECT COUNT(*) FROM work_orders", [], |row| row.get(0))?,
        customers: connection.query_row("SELECT COUNT(*) FROM customers", [], |row| row.get(0))?,
        inventory_items: connection.query_row(
            "SELECT COUNT(*) FROM inventory_items",
            [],
            |row| row.get(0),
        )?,
    })
}

fn database_clock(database_path: &Path, expression: &str) -> Result<String, CommandError> {
    let connection = database::open(database_path)?;
    let sql = format!("SELECT strftime('{expression}', 'now', 'localtime')");
    Ok(connection.query_row(&sql, [], |row| row.get(0))?)
}

fn next_available_path(directory: &Path, stem: &str) -> PathBuf {
    let initial = directory.join(format!("{stem}.db"));
    if !initial.exists() {
        return initial;
    }
    for suffix in 2..10_000 {
        let candidate = directory.join(format!("{stem}-{suffix}.db"));
        if !candidate.exists() {
            return candidate;
        }
    }
    directory.join(format!("{stem}-{}.db", std::process::id()))
}

fn create_snapshot(source: &Path, destination: &Path) -> Result<(), CommandError> {
    if destination.exists() {
        return Err(CommandError::message(
            "Ya existe un respaldo con el mismo nombre.",
        ));
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let connection = database::open(source)?;
    let destination_text = destination.to_string_lossy().into_owned();
    if let Err(error) = connection.execute("VACUUM INTO ?1", [destination_text]) {
        let _ = fs::remove_file(destination);
        return Err(error.into());
    }
    drop(connection);
    if let Err(error) = inspect_database(destination, true) {
        let _ = fs::remove_file(destination);
        return Err(error);
    }
    Ok(())
}

fn managed_kind(file_name: &str) -> Option<&'static str> {
    if !file_name.ends_with(".db") {
        return None;
    }
    if file_name.starts_with("geecko-auto-") {
        Some("automatic")
    } else if file_name.starts_with("geecko-manual-") {
        Some("manual")
    } else if file_name.starts_with("geecko-pre-restauracion-") {
        Some("safety")
    } else {
        None
    }
}

fn modified_at_epoch_ms(metadata: &fs::Metadata) -> u64 {
    metadata
        .modified()
        .ok()
        .and_then(|value| value.duration_since(UNIX_EPOCH).ok())
        .map(|value| value.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn summarize(path: &Path, kind: &str) -> Result<BackupFileSummary, CommandError> {
    let metadata = fs::metadata(path)?;
    let stats = inspect_database(path, false);
    let valid = stats.is_ok();
    let stats = stats.unwrap_or(DatabaseStats {
        version: 0,
        work_orders: 0,
        customers: 0,
        inventory_items: 0,
    });
    Ok(BackupFileSummary {
        file_name: path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_owned(),
        kind: kind.to_owned(),
        path: path.to_string_lossy().into_owned(),
        size_bytes: metadata.len(),
        modified_at_epoch_ms: modified_at_epoch_ms(&metadata),
        database_version: stats.version,
        work_order_count: stats.work_orders,
        customer_count: stats.customers,
        inventory_item_count: stats.inventory_items,
        valid,
    })
}

pub fn list_backups(directory: &Path) -> Result<BackupOverview, CommandError> {
    fs::create_dir_all(directory)?;
    let mut backups = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if let Some(kind) = managed_kind(&file_name) {
            backups.push(summarize(&entry.path(), kind)?);
        }
    }
    backups.sort_by(|left, right| {
        right
            .modified_at_epoch_ms
            .cmp(&left.modified_at_epoch_ms)
            .then_with(|| right.file_name.cmp(&left.file_name))
    });
    Ok(BackupOverview {
        directory: directory.to_string_lossy().into_owned(),
        automatic_enabled: true,
        automatic_retention: AUTOMATIC_RETENTION,
        backups,
    })
}

fn prune(directory: &Path, kind: &str, retention: usize) -> Result<(), CommandError> {
    let overview = list_backups(directory)?;
    for backup in overview
        .backups
        .into_iter()
        .filter(|backup| backup.kind == kind)
        .skip(retention)
    {
        fs::remove_file(directory.join(backup.file_name))?;
    }
    Ok(())
}

pub fn ensure_automatic_backup(
    database_path: &Path,
    directory: &Path,
) -> Result<BackupOverview, CommandError> {
    fs::create_dir_all(directory)?;
    let date = database_clock(database_path, "%Y-%m-%d")?;
    let prefix = format!("geecko-auto-{date}");
    let already_created = list_backups(directory)?.backups.into_iter().any(|backup| {
        backup.kind == "automatic" && backup.file_name.starts_with(&prefix) && backup.valid
    });
    if !already_created {
        let destination = next_available_path(directory, &prefix);
        create_snapshot(database_path, &destination)?;
        prune(directory, "automatic", AUTOMATIC_RETENTION)?;
    }
    list_backups(directory)
}

pub fn create_manual_backup(
    database_path: &Path,
    directory: &Path,
) -> Result<BackupFileSummary, CommandError> {
    fs::create_dir_all(directory)?;
    let stamp = database_clock(database_path, "%Y-%m-%d-%H%M%S")?;
    let destination = next_available_path(directory, &format!("geecko-manual-{stamp}"));
    create_snapshot(database_path, &destination)?;
    summarize(&destination, "manual")
}

fn sidecar_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value: OsString = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}

fn remove_file_if_present(path: &Path) -> Result<(), CommandError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn checkpoint_and_remove_sidecars(path: &Path) -> Result<(), CommandError> {
    {
        let connection = database::open(path)?;
        let _: (i64, i64, i64) =
            connection.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?;
    }
    remove_file_if_present(&sidecar_path(path, "-wal"))?;
    remove_file_if_present(&sidecar_path(path, "-shm"))?;
    Ok(())
}

fn resolve_managed_backup(directory: &Path, file_name: &str) -> Result<PathBuf, CommandError> {
    if file_name.is_empty()
        || Path::new(file_name)
            .file_name()
            .and_then(|name| name.to_str())
            != Some(file_name)
        || managed_kind(file_name).is_none()
    {
        return Err(CommandError::message(
            "Selecciona un respaldo creado por Geecko.",
        ));
    }
    let source = directory.join(file_name);
    if !source.is_file() {
        return Err(CommandError::message(
            "El archivo de respaldo ya no está disponible.",
        ));
    }
    Ok(source)
}

pub fn restore_backup(
    database_path: &Path,
    directory: &Path,
    file_name: &str,
    confirmed: bool,
) -> Result<RestoreBackupResult, CommandError> {
    if !confirmed {
        return Err(CommandError::message(
            "Confirma la restauración antes de reemplazar los datos actuales.",
        ));
    }
    fs::create_dir_all(directory)?;
    let source = resolve_managed_backup(directory, file_name)?;
    inspect_database(&source, true)?;

    let stamp = database_clock(database_path, "%Y-%m-%d-%H%M%S")?;
    let safety_path = next_available_path(directory, &format!("geecko-pre-restauracion-{stamp}"));
    create_snapshot(database_path, &safety_path)?;

    let staged_path = database_path.with_file_name(format!(
        "geecko-restauracion-{}-{stamp}.tmp.db",
        std::process::id()
    ));
    remove_file_if_present(&staged_path)?;
    remove_file_if_present(&sidecar_path(&staged_path, "-wal"))?;
    remove_file_if_present(&sidecar_path(&staged_path, "-shm"))?;
    fs::copy(&source, &staged_path)?;

    if let Err(error) = database::initialize(&staged_path)
        .and_then(|_| inspect_database(&staged_path, true).map(|_| ()))
        .and_then(|_| checkpoint_and_remove_sidecars(&staged_path))
    {
        let _ = remove_file_if_present(&staged_path);
        let _ = remove_file_if_present(&sidecar_path(&staged_path, "-wal"));
        let _ = remove_file_if_present(&sidecar_path(&staged_path, "-shm"));
        return Err(error);
    }

    checkpoint_and_remove_sidecars(database_path)?;
    let rollback_path = database_path.with_file_name(format!(
        "geecko-anterior-{}-{stamp}.tmp.db",
        std::process::id()
    ));
    remove_file_if_present(&rollback_path)?;
    fs::rename(database_path, &rollback_path)?;
    if let Err(error) = fs::rename(&staged_path, database_path) {
        let _ = fs::rename(&rollback_path, database_path);
        return Err(CommandError::message(format!(
            "No fue posible activar el respaldo; los datos actuales se conservaron. {error}"
        )));
    }
    if let Err(error) = remove_file_if_present(&rollback_path) {
        eprintln!(
            "La restauración terminó, pero no se pudo limpiar el archivo temporal: {}",
            error.error
        );
    }
    if let Err(error) = prune(directory, "safety", SAFETY_RETENTION) {
        eprintln!(
            "La restauración terminó, pero no se pudo rotar las copias preventivas: {}",
            error.error
        );
    }

    Ok(RestoreBackupResult {
        restored_file_name: file_name.to_owned(),
        safety_backup_file_name: safety_path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_restores_a_complete_database_snapshot() {
        let suffix = std::time::SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "geecko-backup-test-{}-{suffix}",
            std::process::id()
        ));
        let backup_directory = root.join("backups");
        let database_path = root.join("geecko.db");
        fs::create_dir_all(&root).unwrap();
        database::initialize(&database_path).unwrap();
        {
            let connection = database::open(&database_path).unwrap();
            connection
                .execute("INSERT INTO customers (name) VALUES ('Antes')", [])
                .unwrap();
        }

        let backup = create_manual_backup(&database_path, &backup_directory).unwrap();
        assert!(backup.valid);
        assert_eq!(backup.customer_count, 1);
        let automatic = ensure_automatic_backup(&database_path, &backup_directory).unwrap();
        assert_eq!(
            automatic
                .backups
                .iter()
                .filter(|item| item.kind == "automatic")
                .count(),
            1
        );
        let automatic_again = ensure_automatic_backup(&database_path, &backup_directory).unwrap();
        assert_eq!(
            automatic_again
                .backups
                .iter()
                .filter(|item| item.kind == "automatic")
                .count(),
            1
        );
        {
            let connection = database::open(&database_path).unwrap();
            connection
                .execute("UPDATE customers SET name = 'Después'", [])
                .unwrap();
        }
        let restored =
            restore_backup(&database_path, &backup_directory, &backup.file_name, true).unwrap();
        assert_eq!(restored.restored_file_name, backup.file_name);
        assert!(restored
            .safety_backup_file_name
            .starts_with("geecko-pre-restauracion-"));
        let connection = database::open(&database_path).unwrap();
        let customer_name: String = connection
            .query_row("SELECT name FROM customers LIMIT 1", [], |row| row.get(0))
            .unwrap();
        assert_eq!(customer_name, "Antes");
        drop(connection);
        fs::remove_dir_all(&root).unwrap();
    }
}
