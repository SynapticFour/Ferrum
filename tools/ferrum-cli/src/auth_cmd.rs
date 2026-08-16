// SPDX-License-Identifier: BUSL-1.1
//! Edge operator account management and local login tokens.

use ferrum_core::{
    create_account, list_accounts, mint_local_token, normalize_role, verify_account_pin,
    DatabasePool, FerrumConfig, FerrumPool,
};
use std::path::PathBuf;

async fn edge_pool(config: Option<&PathBuf>) -> Result<FerrumPool, String> {
    let cfg = config
        .and_then(|p| FerrumConfig::load_from_path(p).ok())
        .or_else(|| FerrumConfig::load().ok())
        .ok_or_else(|| "no Ferrum config found (pass --config or set FERRUM_CONFIG)".to_string())?;
    let db = DatabasePool::from_config(&cfg.database)
        .await
        .map_err(|e| e.to_string())?;
    Ok(match db {
        DatabasePool::Sqlite(p) => FerrumPool::Sqlite(p),
        DatabasePool::Postgres(p) => FerrumPool::Postgres(p),
    })
}

fn local_jwt_secret(cfg: &FerrumConfig) -> Result<Vec<u8>, String> {
    cfg.auth
        .jwt_secret
        .as_deref()
        .map(|s| s.as_bytes().to_vec())
        .or_else(|| {
            std::env::var("FERRUM_AUTH__JWT_SECRET")
                .ok()
                .map(|s| s.into_bytes())
        })
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "set auth.jwt_secret or FERRUM_AUTH__JWT_SECRET for local account tokens".into()
        })
}

pub async fn account_add(
    username: &str,
    role: &str,
    pin: &str,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    normalize_role(role).map_err(|e| e.to_string())?;
    let pool = edge_pool(config).await?;
    create_account(&pool, username, role, pin)
        .await
        .map_err(|e| e.to_string())?;
    println!("Created edge account `{username}` with role `{role}`");
    Ok(())
}

pub async fn account_list(config: Option<&PathBuf>) -> Result<(), String> {
    let pool = edge_pool(config).await?;
    let accounts = list_accounts(&pool).await.map_err(|e| e.to_string())?;
    if accounts.is_empty() {
        println!("No edge operator accounts.");
        return Ok(());
    }
    for acct in accounts {
        let flag = if acct.disabled { " (disabled)" } else { "" };
        println!("{} — role={}{flag}", acct.username, acct.role);
    }
    Ok(())
}

pub async fn account_login(
    username: &str,
    pin: &str,
    ttl_hours: u64,
    config: Option<&PathBuf>,
) -> Result<(), String> {
    let cfg = config
        .and_then(|p| FerrumConfig::load_from_path(p).ok())
        .or_else(|| FerrumConfig::load().ok())
        .ok_or_else(|| "no Ferrum config found".to_string())?;
    let pool = edge_pool(config).await?;
    let account = verify_account_pin(&pool, username, pin)
        .await
        .map_err(|e| e.to_string())?;
    let secret = local_jwt_secret(&cfg)?;
    let token = mint_local_token(&account, &secret, ttl_hours).map_err(|e| e.to_string())?;
    println!("{token}");
    Ok(())
}
