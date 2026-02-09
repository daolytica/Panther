// Local GPT interaction commands

use crate::db::Database;
use crate::providers::get_adapter;
use crate::types::{ProviderAccount, PromptPacket, Message};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalGptRequest {
    pub message: String,
    pub conversation_history: Option<Vec<serde_json::Value>>,
    pub model_name: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LocalGptResponse {
    pub response: String,
    pub model_used: String,
    pub tokens_used: Option<u32>,
}

#[tauri::command]
pub async fn chat_with_local_gpt(
    db: State<'_, Database>,
    request: LocalGptRequest,
) -> Result<LocalGptResponse, String> {
    // Find or create a local Ollama provider
    let provider_account = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        // Try to find an existing Ollama provider
        let provider_result: Result<Option<ProviderAccount>, _> = conn_guard.query_row(
            "SELECT id, provider_type, display_name, base_url, region, auth_ref, created_at, updated_at, provider_metadata_json FROM provider_accounts WHERE provider_type = 'ollama' LIMIT 1",
            [],
            |row| {
                let metadata_json_str: Option<String> = row.get(8)?;
                let metadata_json = metadata_json_str.and_then(|s| serde_json::from_str(&s).ok());
                Ok(Some(ProviderAccount {
                    id: row.get(0)?,
                    provider_type: row.get(1)?,
                    display_name: row.get(2)?,
                    base_url: row.get(3)?,
                    region: row.get(4)?,
                    auth_ref: row.get(5)?,
                    created_at: row.get(6)?,
                    updated_at: row.get(7)?,
                    provider_metadata_json: metadata_json,
                }))
            },
        );
        
        match provider_result {
            Ok(Some(provider)) => provider,
            _ => {
                // Create a default Ollama provider if none exists
                return Err("No Ollama provider found. Please add an Ollama provider in Settings first.".to_string());
            }
        }
    };
    
    let model_name = request.model_name.unwrap_or_else(|| "llama2".to_string());
    
    // Build conversation context from history
    let mut conversation_context: Option<Vec<Message>> = None;
    if let Some(history) = request.conversation_history {
        let mut messages = Vec::new();
        for msg in history {
            if let (Some(role), Some(content)) = (msg.get("role").and_then(|r| r.as_str()), msg.get("content").and_then(|c| c.as_str())) {
                messages.push(Message {
                    id: Uuid::new_v4().to_string(),
                    run_id: String::new(), // Not needed for local GPT
                    author_type: if role == "user" { "user".to_string() } else { "assistant".to_string() },
                    profile_id: None,
                    round_index: None,
                    turn_index: None,
                    text: content.to_string(),
                    created_at: Utc::now().to_rfc3339(),
                    provider_metadata_json: None,
                });
            }
        }
        if !messages.is_empty() {
            conversation_context = Some(messages);
        }
    }
    
    // Build prompt packet
    let packet = PromptPacket {
        global_instructions: Some("You are a helpful AI assistant. Provide clear, concise, and helpful responses.".to_string()),
        persona_instructions: String::new(),
        user_message: request.message.clone(),
        conversation_context,
        params_json: serde_json::json!({
            "temperature": request.temperature.unwrap_or(0.7),
            "max_tokens": request.max_tokens.unwrap_or(1000),
        }),
        stream: false,
    };
    
    // Get adapter and generate response
    let adapter = get_adapter(&provider_account.provider_type)
        .map_err(|e| format!("Failed to get adapter: {}", e))?;
    
    let response = adapter.complete(&packet, &provider_account, &model_name)
        .await
        .map_err(|e| format!("Failed to generate response: {}", e))?;
    
    // Extract tokens from usage_json
    let tokens_used = response.usage_json
        .and_then(|u| u.get("total_tokens").and_then(|t| t.as_u64()))
        .map(|t| t as u32);
    
    Ok(LocalGptResponse {
        response: response.text,
        model_used: model_name,
        tokens_used,
    })
}
