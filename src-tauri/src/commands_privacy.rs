// Privacy commands - User controls for privacy settings

use crate::db::Database;
use crate::privacy::{PiiRedactor, RedactionStats, PseudonymManager};
use serde::{Deserialize, Serialize};
use tauri::State;

/// Privacy settings for a user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacySettings {
    pub redact_pii: bool,          // Enable PII redaction before sending to LLM
    pub private_mode: bool,         // Don't store message content server-side
    pub custom_identifiers: Vec<String>,  // User-defined words to redact
    pub retention_days: Option<u32>,       // How long to keep encrypted messages
}

impl Default for PrivacySettings {
    fn default() -> Self {
        Self {
            redact_pii: true,       // PII redaction on by default
            private_mode: false,    // History on by default
            custom_identifiers: Vec::new(),
            retention_days: Some(30),
        }
    }
}

impl PrivacySettings {
    /// Synchronous helper to load privacy settings in non-async contexts.
    pub fn load_sync(db: &Database) -> Result<Self, String> {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;

        let result: Option<String> = conn_guard
            .query_row(
                "SELECT settings_json FROM privacy_settings WHERE id = 'default'",
                [],
                |row| row.get(0),
            )
            .ok();

        match result {
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| format!("Failed to parse settings: {}", e)),
            None => Ok(PrivacySettings::default()),
        }
    }
}

/// Result from testing redaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionPreview {
    pub original_text: String,
    pub redacted_text: String,
    pub stats: RedactionStats,
}

#[tauri::command]
pub async fn get_privacy_settings(
    db: State<'_, Database>,
) -> Result<PrivacySettings, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Try to load from database
    let result: Option<String> = conn_guard
        .query_row(
            "SELECT settings_json FROM privacy_settings WHERE id = 'default'",
            [],
            |row| row.get(0),
        )
        .ok();
    
    match result {
        Some(json) => serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse settings: {}", e)),
        None => Ok(PrivacySettings::default()),
    }
}

#[tauri::command]
pub async fn save_privacy_settings(
    db: State<'_, Database>,
    settings: PrivacySettings,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let settings_json = serde_json::to_string(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;
    
    conn_guard.execute(
        "INSERT OR REPLACE INTO privacy_settings (id, settings_json, updated_at) VALUES ('default', ?1, datetime('now'))",
        rusqlite::params![settings_json],
    ).map_err(|e| format!("Failed to save settings: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn add_custom_identifier(
    db: State<'_, Database>,
    identifier: String,
) -> Result<PrivacySettings, String> {
    let mut settings = get_privacy_settings(db.clone()).await?;
    
    // Avoid duplicates (case-insensitive)
    let lower_id = identifier.to_lowercase();
    if !settings.custom_identifiers.iter().any(|x| x.to_lowercase() == lower_id) {
        settings.custom_identifiers.push(identifier);
    }
    
    save_privacy_settings(db, settings.clone()).await?;
    Ok(settings)
}

#[tauri::command]
pub async fn remove_custom_identifier(
    db: State<'_, Database>,
    identifier: String,
) -> Result<PrivacySettings, String> {
    let mut settings = get_privacy_settings(db.clone()).await?;
    
    let lower_id = identifier.to_lowercase();
    settings.custom_identifiers.retain(|x| x.to_lowercase() != lower_id);
    
    save_privacy_settings(db, settings.clone()).await?;
    Ok(settings)
}

#[tauri::command]
pub async fn preview_redaction(
    db: State<'_, Database>,
    text: String,
) -> Result<RedactionPreview, String> {
    let settings = get_privacy_settings(db).await?;
    
    let redactor = PiiRedactor::new();
    let result = redactor.redact_text(&text, &settings.custom_identifiers, "preview");
    
    Ok(RedactionPreview {
        original_text: text,
        redacted_text: result.redacted_text,
        stats: result.stats,
    })
}

#[tauri::command]
pub async fn delete_all_conversations(
    db: State<'_, Database>,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Delete all chat messages
    conn_guard.execute("DELETE FROM chat_messages", [])
        .map_err(|e| format!("Failed to delete chat messages: {}", e))?;
    
    // Delete all coder chats
    conn_guard.execute("DELETE FROM coder_chats", [])
        .map_err(|e| format!("Failed to delete coder chats: {}", e))?;
    
    // Delete encrypted conversation data if it exists
    conn_guard.execute("DELETE FROM encrypted_conversations", []).ok();
    
    Ok(())
}

#[tauri::command]
pub async fn delete_conversation(
    db: State<'_, Database>,
    conversation_id: String,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Delete from chat_messages (profile chats)
    conn_guard.execute(
        "DELETE FROM chat_messages WHERE profile_id = ?1",
        rusqlite::params![conversation_id],
    ).map_err(|e| format!("Failed to delete messages: {}", e))?;
    
    // Delete from coder_chats
    conn_guard.execute(
        "DELETE FROM coder_chats WHERE id = ?1",
        rusqlite::params![conversation_id],
    ).ok();
    
    // Delete encrypted data if exists
    conn_guard.execute(
        "DELETE FROM encrypted_conversations WHERE conversation_id = ?1",
        rusqlite::params![conversation_id],
    ).ok();
    
    Ok(())
}

#[tauri::command]
pub async fn get_pseudonym_for_conversation(
    _conversation_id: String,
) -> Result<String, String> {
    // Generate ephemeral pseudonym (no stable identifier)
    let manager = PseudonymManager::with_random_secret();
    Ok(manager.generate_ephemeral_pseudonym())
}
