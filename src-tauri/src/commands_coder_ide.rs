// Coder IDE specific commands - for the new IDE interface

use crate::db::Database;
use crate::training_ingest;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoderIDEMessage {
    pub id: String,
    pub role: String,  // "user" | "assistant" | "system"
    pub content: String,
    pub timestamp: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub context_files: Option<Vec<String>>,  // Files that were open/edited during this conversation
    pub terminal_output: Option<String>,      // Terminal output relevant to this message
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoderIDEConversation {
    pub id: String,
    pub title: String,
    pub messages: Vec<CoderIDEMessage>,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub async fn save_coder_ide_conversation(
    db: State<'_, Database>,
    conversation: CoderIDEConversation,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let messages_json = serde_json::to_string(&conversation.messages)
        .map_err(|e| format!("Failed to serialize messages: {}", e))?;
    
    conn_guard.execute(
        "INSERT OR REPLACE INTO coder_ide_conversations (id, title, messages_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            conversation.id,
            conversation.title,
            messages_json,
            conversation.created_at,
            conversation.updated_at
        ],
    )
    .map_err(|e| format!("Failed to save conversation: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn load_coder_ide_conversations(
    db: State<'_, Database>,
) -> Result<Vec<CoderIDEConversation>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let mut stmt = conn_guard
        .prepare("SELECT id, title, messages_json, created_at, updated_at FROM coder_ide_conversations ORDER BY updated_at DESC")
        .map_err(|e| format!("Database error: {}", e))?;
    
    let rows = stmt.query_map([], |row| {
        let messages_json: String = row.get(2)?;
        let messages: Vec<CoderIDEMessage> = serde_json::from_str(&messages_json)
            .unwrap_or_default();
        
        Ok(CoderIDEConversation {
            id: row.get(0)?,
            title: row.get(1)?,
            messages,
            created_at: row.get(3)?,
            updated_at: row.get(4)?,
        })
    })
    .map_err(|e| format!("Database error: {}", e))?;
    
    let mut conversations = Vec::new();
    for row in rows {
        conversations.push(row.map_err(|e| format!("Row error: {}", e))?);
    }
    
    Ok(conversations)
}

#[tauri::command]
pub async fn delete_coder_ide_conversation(
    db: State<'_, Database>,
    conversation_id: String,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    conn_guard.execute(
        "DELETE FROM coder_ide_conversations WHERE id = ?1",
        [&conversation_id],
    )
    .map_err(|e| format!("Failed to delete conversation: {}", e))?;
    
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportCoderIDEConversationsRequest {
    pub conversation_ids: Vec<String>,
    pub message_ids: Option<Vec<String>>,  // If None, export all messages
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub include_context: bool,  // Include file context and terminal output
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngestCoderTurnRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub user_text: String,
    pub assistant_text: String,
    pub context_files: Vec<String>,
    pub terminal_output: Option<String>,
}

/// Lightweight wrapper to ingest a single Coder IDE turn into training_data.
#[tauri::command]
pub async fn ingest_coder_turn_command(
    db: State<'_, Database>,
    request: IngestCoderTurnRequest,
) -> Result<(), String> {
    training_ingest::ingest_coder_turn(
        &db,
        &request.project_id,
        request.local_model_id.as_deref(),
        &request.user_text,
        &request.assistant_text,
        &request.context_files,
        request.terminal_output.as_deref(),
    )
}

#[tauri::command]
pub async fn export_coder_ide_conversations_to_training(
    db: State<'_, Database>,
    request: ExportCoderIDEConversationsRequest,
) -> Result<usize, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    if request.conversation_ids.is_empty() {
        return Err("No conversations selected for export".to_string());
    }
    
    let mut exported_count = 0;
    
    // Load conversations
    let placeholders: String = request.conversation_ids.iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    
    let query = format!(
        "SELECT id, messages_json FROM coder_ide_conversations WHERE id IN ({})",
        placeholders
    );
    
    let mut stmt = conn_guard
        .prepare(&query)
        .map_err(|e| format!("Database error: {}", e))?;
    
    let params: Vec<&dyn rusqlite::ToSql> = request.conversation_ids.iter()
        .map(|id| id as &dyn rusqlite::ToSql)
        .collect();
    
    let rows = stmt.query_map(rusqlite::params_from_iter(params.iter()), |row| {
        Ok((
            row.get::<_, String>(0)?,  // id
            row.get::<_, String>(1)?,  // messages_json
        ))
    })
    .map_err(|e| format!("Database error: {}", e))?;
    
    for row in rows {
        let (conversation_id, messages_json) = row.map_err(|e| format!("Row error: {}", e))?;
        
        let messages: Vec<CoderIDEMessage> = serde_json::from_str(&messages_json)
            .map_err(|e| format!("Failed to parse messages: {}", e))?;
        
        // Filter messages if specific IDs provided
        let messages_to_export: Vec<&CoderIDEMessage> = if let Some(selected_ids) = &request.message_ids {
            messages.iter()
                .filter(|msg| selected_ids.contains(&msg.id))
                .collect()
        } else {
            messages.iter().collect()
        };
        
        // Pair user messages with assistant responses
        let mut current_user_message: Option<&str> = None;
        let mut current_context: Option<String> = None;
        let mut current_terminal: Option<String> = None;
        
        for msg in messages_to_export {
            match msg.role.as_str() {
                "user" => {
                    current_user_message = Some(&msg.content);
                    if request.include_context {
                        if let Some(files) = &msg.context_files {
                            current_context = Some(format!("Context files: {}", files.join(", ")));
                        }
                        if let Some(term) = &msg.terminal_output {
                            current_terminal = Some(term.clone());
                        }
                    }
                }
                "assistant" => {
                    if let Some(user_msg) = current_user_message.take() {
                        // Build enhanced input with context if requested
                        let mut input_text = user_msg.to_string();
                        if request.include_context {
                            if let Some(context) = current_context.take() {
                                input_text = format!("{}\n\n{}", input_text, context);
                            }
                            if let Some(terminal) = current_terminal.take() {
                                input_text = format!("{}\n\nTerminal output:\n{}", input_text, terminal);
                            }
                        }
                        
                        // Create training data entry
                        let training_id = Uuid::new_v4().to_string();
                        let now = Utc::now().to_rfc3339();
                        
                        let metadata = serde_json::json!({
                            "source": "coder_ide_export",
                            "conversation_id": conversation_id,
                            "message_id": msg.id,
                            "model": msg.model,
                            "provider": msg.provider,
                            "exported_at": now,
                            "include_context": request.include_context
                        });
                        
                        conn_guard.execute(
                            "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            rusqlite::params![
                                training_id,
                                request.project_id,
                                request.local_model_id,
                                input_text,
                                msg.content,
                                serde_json::to_string(&metadata).unwrap_or_default(),
                                now
                            ],
                        )
                        .map_err(|e| format!("Failed to insert training data: {}", e))?;
                        
                        exported_count += 1;
                    }
                }
                _ => {} // Skip system messages
            }
        }
    }
    
    Ok(exported_count)
}
