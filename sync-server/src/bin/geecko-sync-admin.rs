use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, Row};
use std::env;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let arguments = env::args().skip(1).collect::<Vec<_>>();
    if arguments.len() != 4 || arguments[0] != "create-device" {
        eprintln!("Uso: geecko-sync-admin create-device <taller> <dispositivo> <windows|android>");
        std::process::exit(2);
    }
    let database_url = env::var("DATABASE_URL")?;
    let platform = arguments[3].as_str();
    if !matches!(platform, "windows" | "android") {
        return Err("La plataforma debe ser windows o android.".into());
    }
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await?;
    sqlx::migrate!().run(&pool).await?;
    let workshop_id: Uuid = sqlx::query_scalar(
        "INSERT INTO geecko.workshops (name) VALUES ($1)
         ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name
         RETURNING id",
    )
    .bind(&arguments[1])
    .fetch_one(&pool)
    .await?;
    let prefix_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM geecko.devices WHERE workshop_id = $1 AND platform = $2",
    )
    .bind(workshop_id)
    .bind(platform)
    .fetch_one(&pool)
    .await?;
    let prefix = if platform == "android" {
        format!("M{:02}", prefix_count + 1)
    } else {
        format!("PC{:02}", prefix_count + 1)
    };
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = Sha256::digest(token.as_bytes()).to_vec();
    let row = sqlx::query(
        "INSERT INTO geecko.devices
           (workshop_id, name, platform, device_prefix, token_hash)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id",
    )
    .bind(workshop_id)
    .bind(&arguments[2])
    .bind(platform)
    .bind(&prefix)
    .bind(token_hash)
    .fetch_one(&pool)
    .await?;
    let device_id: Uuid = row.get("id");
    println!("Taller: {workshop_id}");
    println!("Dispositivo: {device_id}");
    println!("Prefijo: {prefix}");
    println!("Token (se muestra una sola vez): {token}");
    Ok(())
}
