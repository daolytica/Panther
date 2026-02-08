use crate::db::Database;
use chrono::Utc;
use uuid::Uuid;

/// Record token usage for a single LLM call.
///
/// - `provider_id`: optional provider_accounts.id
/// - `model_name`: model identifier used for the call
/// - `usage_json`: normalized usage blob from the adapter (may be provider-specific)
/// - `source`: short label like "profile_chat", "coder_chat", "debate", "training_chat", "coder_auto_local", "coder_auto_remote"
/// - `context_hash`: optional hash of the prompt/context for aggregation
/// - `metadata`: extra JSON to store alongside, if any
pub fn record_token_usage(
    db: &Database,
    provider_id: Option<&str>,
    model_name: &str,
    usage_json: &Option<serde_json::Value>,
    source: &str,
    context_hash: Option<&str>,
    metadata: Option<serde_json::Value>,
) -> Result<(), String> {
    // If there is no usage info, don't record anything.
    let usage = match usage_json {
        Some(v) => v,
        None => return Ok(()),
    };

    // Try to read common fields; fall back to 0.
    let prompt_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let completion_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let total_tokens = usage
        .get("total_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or_else(|| prompt_tokens + completion_tokens);

    // If everything is zero, skip recording to avoid noise.
    if prompt_tokens == 0 && completion_tokens == 0 && total_tokens == 0 {
        return Ok(());
    }

    let id = Uuid::new_v4().to_string();
    let timestamp = Utc::now().to_rfc3339();
    let provider_id_str = provider_id.map(|s| s.to_string());
    let context_hash_str = context_hash.map(|s| s.to_string());
    let metadata_str = metadata
        .and_then(|m| serde_json::to_string(&m).ok())
        .unwrap_or_else(|| "{}".to_string());

    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    conn_guard
        .execute(
            "INSERT INTO token_usage (
                id, timestamp, provider_id, model_name,
                prompt_tokens, completion_tokens, total_tokens,
                context_hash, source, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                id,
                timestamp,
                provider_id_str,
                model_name,
                prompt_tokens,
                completion_tokens,
                total_tokens,
                context_hash_str,
                source,
                metadata_str
            ],
        )
        .map_err(|e| format!("Failed to insert token usage: {}", e))?;

    Ok(())
}

