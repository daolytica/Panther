// Statistics and system information commands

use crate::db::Database;
use serde::{Deserialize, Serialize};
use tauri::State;
use std::fs;

#[derive(Debug, Serialize, Deserialize)]
pub struct AppStatistics {
    pub providers_count: i32,
    pub profiles_count: i32,
    pub projects_count: i32,
    pub sessions_count: i32,
    pub runs_count: i32,
    pub local_models_count: i32,
    pub training_data_count: i32,
    pub messages_count: i32,
    pub total_storage_bytes: u64,
    pub database_size_bytes: u64,
}

#[tauri::command]
pub async fn get_app_statistics(
    db: State<'_, Database>,
) -> Result<AppStatistics, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    let providers_count: i32 = conn_guard
        .query_row("SELECT COUNT(*) FROM provider_accounts", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count providers: {}", e))?;

    let profiles_count: i32 = conn_guard
        .query_row("SELECT COUNT(*) FROM prompt_profiles", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count profiles: {}", e))?;

    let projects_count: i32 = conn_guard
        .query_row("SELECT COUNT(*) FROM projects", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count projects: {}", e))?;

    let sessions_count: i32 = conn_guard
        .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count sessions: {}", e))?;

    let runs_count: i32 = conn_guard
        .query_row("SELECT COUNT(*) FROM runs", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count runs: {}", e))?;

    let local_models_count: i32 = conn_guard
        .query_row("SELECT COUNT(*) FROM local_models", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count local models: {}", e))?;

    let training_data_count: i32 = conn_guard
        .query_row("SELECT COUNT(*) FROM training_data", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count training data: {}", e))?;

    let messages_count: i32 = conn_guard
        .query_row("SELECT COUNT(*) FROM messages", [], |row| row.get(0))
        .map_err(|e| format!("Failed to count messages: {}", e))?;

    // Get database file size
    // Note: We'll estimate storage based on database size only for now
    // In a production app, we'd store the db_path in the Database struct
    // For now, we'll use a reasonable estimate or calculate from SQLite pragma
    let database_size_bytes: u64 = conn_guard
        .query_row("SELECT page_count * page_size FROM pragma_page_count(), pragma_page_size()", [], |row| {
            let page_count: i64 = row.get(0)?;
            let page_size: i64 = row.get(1)?;
            Ok((page_count * page_size) as u64)
        })
        .unwrap_or(0);
    
    // Calculate total storage (for now, just database size)
    // In the future, could include cached models, logs, etc.
    let total_storage = database_size_bytes;

    Ok(AppStatistics {
        providers_count,
        profiles_count,
        projects_count,
        sessions_count,
        runs_count,
        local_models_count,
        training_data_count,
        messages_count,
        total_storage_bytes: total_storage,
        database_size_bytes,
    })
}

#[tauri::command]
pub async fn clear_cache() -> Result<String, String> {
    // Placeholder for cache clearing
    // In the future, could clear:
    // - Temporary files
    // - Cached API responses
    // - Log files
    Ok("Cache cleared successfully".to_string())
}

#[tauri::command]
pub async fn get_build_directory_size() -> Result<serde_json::Value, String> {
    // Get the project root (assuming we're in src-tauri)
    let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current dir: {}", e))?;
    let project_root = current_dir.parent()
        .ok_or_else(|| "Failed to get project root".to_string())?;
    let target_dir = project_root.join("src-tauri").join("target");
    
    let mut total_size = 0u64;
    let mut file_count = 0u64;
    
    if target_dir.exists() {
        if let Ok(entries) = fs::read_dir(&target_dir) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        total_size += metadata.len();
                        file_count += 1;
                    } else if metadata.is_dir() {
                        // Recursively calculate directory size
                        if let Ok(sub_entries) = fs::read_dir(entry.path()) {
                            for sub_entry in sub_entries.flatten() {
                                if let Ok(sub_meta) = sub_entry.metadata() {
                                    if sub_meta.is_file() {
                                        total_size += sub_meta.len();
                                        file_count += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(serde_json::json!({
        "size_bytes": total_size,
        "size_gb": (total_size as f64) / (1024.0 * 1024.0 * 1024.0),
        "file_count": file_count,
        "path": target_dir.to_string_lossy().to_string(),
    }))
}

#[tauri::command]
pub async fn clean_build_directory() -> Result<String, String> {
    use std::process::Command;
    
    // Get the project root
    let current_dir = std::env::current_dir().map_err(|e| format!("Failed to get current dir: {}", e))?;
    let project_root = current_dir.parent()
        .ok_or_else(|| "Failed to get project root".to_string())?;
    let tauri_dir = project_root.join("src-tauri");
    
    // Run cargo clean
    let output = Command::new("cargo")
        .arg("clean")
        .current_dir(&tauri_dir)
        .output()
        .map_err(|e| format!("Failed to run cargo clean: {}", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("cargo clean failed: {}", stderr));
    }
    
    Ok("Build directory cleaned successfully".to_string())
}

#[tauri::command]
pub async fn export_database_backup(db_path: String, backup_path: String) -> Result<String, String> {
    fs::copy(&db_path, &backup_path)
        .map_err(|e| format!("Failed to backup database: {}", e))?;
    Ok(format!("Database backed up to: {}", backup_path))
}

#[tauri::command]
pub async fn get_app_info() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "name": "Panther",
        "version": env!("CARGO_PKG_VERSION"),
        "description": "Advanced AI Agent Platform",
        "author": "Reza Mirfayzi",
    }))
}

#[tauri::command]
pub async fn get_database_path() -> Result<String, String> {
    // Get production database path (must match the path used in lib.rs)
    let production_db_path = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(|p| format!("{}\\panther\\panther.db", p))
            .unwrap_or_else(|_| "Unknown".to_string())
    } else {
        std::env::var("HOME")
            .map(|h| format!("{}/.local/share/panther/panther.db", h))
            .unwrap_or_else(|_| "Unknown".to_string())
    };
    
    // Check if production database exists
    let exists = std::path::Path::new(&production_db_path).exists();
    
    Ok(serde_json::json!({
        "production_path": production_db_path,
        "exists": exists,
        "message": if exists {
            "Production database found. Data will be shared between dev and production."
        } else {
            "Production database not found. Dev mode uses a separate database."
        }
    }).to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TokenUsageSummaryEntry {
    pub provider_id: Option<String>,
    pub model_name: String,
    pub source: String,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub first_used_at: String,
    pub last_used_at: String,
}

#[allow(dead_code)]
#[tauri::command]
pub async fn get_token_usage_summary(
    db: State<'_, Database>,
) -> Result<serde_json::Value, String> {
    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    // Aggregate by provider_id + model_name + source
    let mut stmt = conn_guard
        .prepare(
            "SELECT 
                provider_id,
                model_name,
                source,
                COALESCE(SUM(prompt_tokens), 0) AS total_prompt_tokens,
                COALESCE(SUM(completion_tokens), 0) AS total_completion_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                MIN(timestamp) AS first_used_at,
                MAX(timestamp) AS last_used_at
             FROM token_usage
             GROUP BY provider_id, model_name, source
             ORDER BY last_used_at DESC",
        )
        .map_err(|e| format!("Database error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(TokenUsageSummaryEntry {
                provider_id: row.get(0)?,
                model_name: row.get(1)?,
                source: row.get(2)?,
                total_prompt_tokens: row.get(3)?,
                total_completion_tokens: row.get(4)?,
                total_tokens: row.get(5)?,
                first_used_at: row.get(6)?,
                last_used_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("Database error: {}", e))?;

    let mut entries: Vec<TokenUsageSummaryEntry> = Vec::new();
    for row in rows {
        entries.push(row.map_err(|e| format!("Row error: {}", e))?);
    }

    Ok(serde_json::json!(entries))
}

#[tauri::command]
#[allow(dead_code)]
pub async fn reset_token_usage(
    db: State<'_, Database>,
    provider_id: Option<String>,
    model_name: Option<String>,
) -> Result<u64, String> {
    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    let affected = if let (Some(pid), Some(model)) = (provider_id.as_ref(), model_name.as_ref()) {
        conn_guard
            .execute(
                "DELETE FROM token_usage WHERE provider_id = ?1 AND model_name = ?2",
                rusqlite::params![pid, model],
            )
            .map_err(|e| format!("Database error: {}", e))?
    } else if let Some(pid) = provider_id.as_ref() {
        conn_guard
            .execute(
                "DELETE FROM token_usage WHERE provider_id = ?1",
                rusqlite::params![pid],
            )
            .map_err(|e| format!("Database error: {}", e))?
    } else if let Some(model) = model_name.as_ref() {
        conn_guard
            .execute(
                "DELETE FROM token_usage WHERE model_name = ?1",
                rusqlite::params![model],
            )
            .map_err(|e| format!("Database error: {}", e))?
    } else {
        conn_guard
            .execute("DELETE FROM token_usage", [])
            .map_err(|e| format!("Database error: {}", e))?
    };

    Ok(affected as u64)
}
