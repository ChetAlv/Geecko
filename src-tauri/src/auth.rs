use crate::domain::{CommandError, SessionUser, UserAccount, UserActivity, ValidatedUser};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rusqlite::{params, Connection, OptionalExtension};

fn hash_password(password: &str) -> Result<String, CommandError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|value| value.to_string())
        .map_err(|error| {
            eprintln!("No fue posible proteger la contraseña: {error}");
            CommandError::internal()
        })
}

fn verify_password(password: &str, encoded: &str) -> bool {
    PasswordHash::new(encoded)
        .ok()
        .and_then(|hash| {
            Argon2::default()
                .verify_password(password.as_bytes(), &hash)
                .ok()
        })
        .is_some()
}

fn map_user(row: &rusqlite::Row<'_>) -> rusqlite::Result<UserAccount> {
    Ok(UserAccount {
        id: row.get(0)?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        role: row.get(3)?,
        active: row.get::<_, i64>(4)? == 1,
        created_at: row.get(5)?,
        last_login_at: row.get(6)?,
    })
}

pub fn requires_setup(connection: &Connection) -> Result<bool, CommandError> {
    Ok(connection.query_row("SELECT COUNT(*) = 0 FROM app_users", [], |row| row.get(0))?)
}

pub fn create_user(
    connection: &Connection,
    user: ValidatedUser,
) -> Result<UserAccount, CommandError> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM app_users WHERE username = ?1 COLLATE NOCASE)",
        [&user.username],
        |row| row.get(0),
    )?;
    if exists {
        return Err(CommandError::message(
            "Ya existe una cuenta con ese nombre de usuario.",
        ));
    }
    let password_hash = hash_password(&user.password)?;
    connection.execute(
        "INSERT INTO app_users (username, display_name, role, password_hash)
         VALUES (?1, ?2, ?3, ?4)",
        params![user.username, user.display_name, user.role, password_hash],
    )?;
    let id = connection.last_insert_rowid();
    get_user(connection, id)?.ok_or_else(CommandError::internal)
}

pub fn authenticate(
    connection: &Connection,
    username: &str,
    password: &str,
) -> Result<SessionUser, CommandError> {
    let record: Option<(SessionUser, String, bool)> = connection
        .query_row(
            "SELECT id, username, display_name, role, password_hash, active
             FROM app_users WHERE username = ?1 COLLATE NOCASE",
            [username],
            |row| {
                Ok((
                    SessionUser {
                        id: row.get(0)?,
                        username: row.get(1)?,
                        display_name: row.get(2)?,
                        role: row.get(3)?,
                    },
                    row.get(4)?,
                    row.get::<_, i64>(5)? == 1,
                ))
            },
        )
        .optional()?;
    let (user, password_hash, active) = record
        .ok_or_else(|| CommandError::message("El usuario o la contraseña no son correctos."))?;
    if !active || !verify_password(password, &password_hash) {
        return Err(CommandError::message(
            "El usuario o la contraseña no son correctos.",
        ));
    }
    connection.execute(
        "UPDATE app_users SET last_login_at = CURRENT_TIMESTAMP WHERE id = ?1",
        [user.id],
    )?;
    Ok(user)
}

pub fn get_user(connection: &Connection, id: i64) -> Result<Option<UserAccount>, CommandError> {
    connection
        .query_row(
            "SELECT id, username, display_name, role, active, created_at, last_login_at
             FROM app_users WHERE id = ?1",
            [id],
            map_user,
        )
        .optional()
        .map_err(Into::into)
}

pub fn list_users(connection: &Connection) -> Result<Vec<UserAccount>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, username, display_name, role, active, created_at, last_login_at
         FROM app_users ORDER BY active DESC, display_name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], map_user)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn set_user_active(
    connection: &Connection,
    id: i64,
    active: bool,
    current_user_id: i64,
) -> Result<Option<UserAccount>, CommandError> {
    let target = match get_user(connection, id)? {
        Some(user) => user,
        None => return Ok(None),
    };
    if id == current_user_id && !active {
        return Err(CommandError::message(
            "No puedes desactivar la cuenta con la que estás trabajando.",
        ));
    }
    if target.role == "admin" && target.active && !active {
        let active_admins: i64 = connection.query_row(
            "SELECT COUNT(*) FROM app_users WHERE role = 'admin' AND active = 1",
            [],
            |row| row.get(0),
        )?;
        if active_admins <= 1 {
            return Err(CommandError::message(
                "Debe permanecer al menos un administrador activo.",
            ));
        }
    }
    connection.execute(
        "UPDATE app_users SET active = ?1 WHERE id = ?2",
        params![active as i64, id],
    )?;
    get_user(connection, id)
}

pub fn reset_password(
    connection: &Connection,
    id: i64,
    password: &str,
) -> Result<Option<UserAccount>, CommandError> {
    if get_user(connection, id)?.is_none() {
        return Ok(None);
    }
    connection.execute(
        "UPDATE app_users SET password_hash = ?1 WHERE id = ?2",
        params![hash_password(password)?, id],
    )?;
    get_user(connection, id)
}

pub fn record_activity(
    connection: &Connection,
    user: &SessionUser,
    action: &str,
    entity_type: &str,
    entity_id: Option<i64>,
    summary: &str,
) -> Result<(), CommandError> {
    connection.execute(
        "INSERT INTO user_activity
           (user_id, user_display_name, user_role, action, entity_type, entity_id, summary)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            user.id,
            user.display_name,
            user.role,
            action,
            entity_type,
            entity_id,
            summary
        ],
    )?;
    Ok(())
}

pub fn list_activity(
    connection: &Connection,
    limit: i64,
) -> Result<Vec<UserActivity>, CommandError> {
    let mut statement = connection.prepare(
        "SELECT id, user_display_name, user_role, action, entity_type, entity_id, summary, created_at
         FROM user_activity ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = statement.query_map([limit.clamp(1, 500)], |row| {
        Ok(UserActivity {
            id: row.get(0)?,
            user_display_name: row.get(1)?,
            user_role: row.get(2)?,
            action: row.get(3)?,
            entity_type: row.get(4)?,
            entity_id: row.get(5)?,
            summary: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{database, domain::ValidatedUser};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn protects_local_accounts_and_records_activity() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "geecko-auth-test-{}-{suffix}.db",
            std::process::id()
        ));
        database::initialize(&path).unwrap();
        let connection = database::open(&path).unwrap();
        assert!(requires_setup(&connection).unwrap());
        let account = create_user(
            &connection,
            ValidatedUser {
                username: "admin.local".into(),
                display_name: "Administrador local".into(),
                role: "admin".into(),
                password: "clave-segura-123".into(),
            },
        )
        .unwrap();
        assert!(!requires_setup(&connection).unwrap());
        assert!(authenticate(&connection, "admin.local", "incorrecta").is_err());
        let session = authenticate(&connection, "admin.local", "clave-segura-123").unwrap();
        assert_eq!(session.id, account.id);
        assert_eq!(session.role, "admin");
        record_activity(
            &connection,
            &session,
            "login",
            "session",
            None,
            "Inicio de sesion de prueba",
        )
        .unwrap();
        assert_eq!(list_activity(&connection, 10).unwrap().len(), 1);
        drop(connection);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }
}
