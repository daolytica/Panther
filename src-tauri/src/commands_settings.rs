// Application settings commands

use crate::db::Database;
use serde::{Deserialize, Serialize};
use tauri::State;
use serde_json;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSettings {
    pub max_size_gb: u64,
    pub eviction_threshold_percent: u64,
    pub enable_compression: bool,
    pub enable_memory_mapped_files: bool,
    pub memory_mapped_threshold_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingSettings {
    pub streaming_chunk_size: usize,
    pub enable_adaptive_memory: bool,
    pub min_chunk_size: usize,
    pub max_chunk_size: usize,
    pub memory_pressure_threshold_mb: u64,
    pub enable_progress_tracking: bool,
    pub progress_update_interval: usize,
    pub enable_parallel_hashing: bool,
    pub parallel_hash_threshold: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoTrainingSettings {
    /// Master toggle for converting activities into training data.
    pub auto_training_enabled: bool,
    /// Whether to ingest profile chats into training_data.
    pub train_from_chat: bool,
    /// Whether to ingest Coder IDE conversations into training_data.
    pub train_from_coder: bool,
    /// Whether to ingest Debate Room conversations into training_data.
    pub train_from_debate: bool,
}

impl Default for AutoTrainingSettings {
    fn default() -> Self {
        Self {
            auto_training_enabled: true,
            train_from_chat: true,
            train_from_coder: true,
            train_from_debate: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub cache: CacheSettings,
    pub training: TrainingSettings,
    #[serde(default)]
    pub auto_training: AutoTrainingSettings,
    /// Path to file containing global system prompt (prepended to all LLM calls).
    /// Relative paths are resolved from the workspace/project root.
    #[serde(default)]
    pub global_system_prompt_file: Option<String>,
}

impl AppSettings {
    /// Default with IDE_prompt.txt linked as global system prompt (for first-run seed).
    pub fn default_with_ide_prompt() -> Self {
        let mut s = Self::default();
        s.global_system_prompt_file = Some("IDE_prompt.txt".to_string());
        s
    }
}

/// Read global prompt from the linked file. Returns None if file path is unset or read fails.
pub fn read_global_prompt_from_file(settings: &AppSettings) -> Option<String> {
    let path_str = settings.global_system_prompt_file.as_ref()?.trim();
    if path_str.is_empty() {
        return None;
    }
    let path = Path::new(path_str);
    let full_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        // Relative to workspace root (parent of src-tauri when running from project)
        std::env::current_dir()
            .ok()
            .and_then(|cwd| {
                // If cwd is src-tauri, go up to project root
                let root = if cwd.ends_with("src-tauri") {
                    cwd.parent()?.to_path_buf()
                } else {
                    cwd
                };
                Some(root.join(path_str))
            })?
    };
    std::fs::read_to_string(&full_path).ok().filter(|s| !s.trim().is_empty())
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            cache: CacheSettings {
                max_size_gb: 10,
                eviction_threshold_percent: 80,
                enable_compression: true,
                enable_memory_mapped_files: true,
                memory_mapped_threshold_mb: 100,
            },
            training: TrainingSettings {
                streaming_chunk_size: 1000,
                enable_adaptive_memory: true,
                min_chunk_size: 100,
                max_chunk_size: 10000,
                memory_pressure_threshold_mb: 2048,
                enable_progress_tracking: true,
                progress_update_interval: 1000,
                enable_parallel_hashing: true,
                parallel_hash_threshold: 10000,
            },
            auto_training: AutoTrainingSettings {
                auto_training_enabled: true,
                train_from_chat: true,
                train_from_coder: true,
                train_from_debate: true,
            },
            global_system_prompt_file: None,
        }
    }
}

#[tauri::command]
pub async fn get_app_settings(
    db: State<'_, Database>,
) -> Result<AppSettings, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let result: Option<String> = conn_guard
        .query_row(
            "SELECT settings_json FROM app_settings WHERE id = 'default'",
            [],
            |row| row.get(0),
        )
        .ok();
    
    match result {
        Some(json) => {
            serde_json::from_str(&json)
                .map_err(|e| format!("Failed to parse settings: {}", e))
                .or_else(|_| Ok(AppSettings::default()))
        }
        None => Ok(AppSettings::default_with_ide_prompt()),
    }
}

#[tauri::command]
pub async fn save_app_settings(
    db: State<'_, Database>,
    settings: AppSettings,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let settings_json = serde_json::to_string(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    
    conn_guard.execute(
        "INSERT OR REPLACE INTO app_settings (id, settings_json, updated_at) VALUES ('default', ?1, datetime('now'))",
        rusqlite::params![settings_json],
    ).map_err(|e| format!("Failed to save settings: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn update_cache_settings(
    db: State<'_, Database>,
    cache_settings: CacheSettings,
) -> Result<AppSettings, String> {
    let mut settings = get_app_settings(db.clone()).await?;
    settings.cache = cache_settings;
    save_app_settings(db, settings.clone()).await?;
    Ok(settings)
}

/// Returns the default file path for the global prompt (IDE_prompt.txt).
#[tauri::command]
pub async fn get_default_global_prompt_path() -> Result<String, String> {
    Ok("IDE_prompt.txt".to_string())
}

#[tauri::command]
pub async fn update_global_system_prompt_file(
    db: State<'_, Database>,
    file_path: Option<String>,
) -> Result<AppSettings, String> {
    let mut settings = get_app_settings(db.clone()).await?;
    settings.global_system_prompt_file = file_path.filter(|s| !s.trim().is_empty());
    save_app_settings(db, settings.clone()).await?;
    Ok(settings)
}

/// Read the current global prompt from the linked file (for preview in UI).
#[tauri::command]
pub async fn read_global_prompt_file(db: State<'_, Database>) -> Result<Option<String>, String> {
    let settings = get_app_settings(db).await?;
    Ok(read_global_prompt_from_file(&settings))
}

#[tauri::command]
pub async fn update_training_settings(
    db: State<'_, Database>,
    training_settings: TrainingSettings,
) -> Result<AppSettings, String> {
    let mut settings = get_app_settings(db.clone()).await?;
    settings.training = training_settings;
    save_app_settings(db, settings.clone()).await?;
    Ok(settings)
}

/// Load settings synchronously (for use in non-async contexts)
pub fn load_settings_sync(db: &Database) -> AppSettings {
    let conn = db.get_connection();
    if let Ok(conn_guard) = conn.lock() {
        match conn_guard
            .query_row(
                "SELECT settings_json FROM app_settings WHERE id = 'default'",
                [],
                |row| Ok(row.get::<_, String>(0)?),
            ) {
            Ok(json) => {
                if let Ok(settings) = serde_json::from_str::<AppSettings>(&json) {
                    return settings;
                }
            }
            Err(_) => {}
        }
    }
    AppSettings::default_with_ide_prompt()
}
