// Unified training data ingestion helpers.
//
// This module centralizes how "activities" (profile chat, Coder IDE, debate)
// are turned into rows in the `training_data` table so that:
// - all sources share consistent metadata
// - privacy / redaction rules can be applied in one place
// - auto‑training can be toggled per source.

use crate::db::Database;
use crate::commands_settings::load_settings_sync;
use crate::commands_privacy::PrivacySettings;
use crate::privacy::PiiRedactor;
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

/// Simple enum to tag where a training pair came from.
#[allow(dead_code)]
pub enum TrainingSource {
    ProfileChat,
    CoderIDE,
    DebateRoom,
    ClineIDE,
}

impl TrainingSource {
    fn as_str(&self) -> &'static str {
        match self {
            TrainingSource::ProfileChat => "profile_chat",
            TrainingSource::CoderIDE => "coder_ide",
            TrainingSource::DebateRoom => "debate_room",
            TrainingSource::ClineIDE => "cline_ide",
        }
    }
}

/// Core helper to insert a single input/output pair into `training_data`
/// with consistent metadata and optional PII redaction.
fn insert_training_row(
    db: &Database,
    project_id: &str,
    local_model_id: Option<&str>,
    input_text: &str,
    output_text: &str,
    source: TrainingSource,
    extra_metadata: Option<serde_json::Value>,
) -> Result<(), String> {
    // Load privacy + app settings to decide whether to redact before storing.
    // IMPORTANT: do this BEFORE taking a DB lock to avoid self-deadlocks.
    let privacy_settings = PrivacySettings::load_sync(db).unwrap_or_default();
    let _app_settings = load_settings_sync(db);

    // NOTE: Training data is stored locally, but we still respect PII redaction
    // if the user has enabled it, so that auto‑training never stores raw PII
    // by surprise.
    let (final_input, final_output) = if privacy_settings.redact_pii {
        let redactor = PiiRedactor::new();
        let ids = &privacy_settings.custom_identifiers;

        let redacted_in = redactor.redact_text(input_text, ids, "training_ingest_input");
        let redacted_out = redactor.redact_text(output_text, ids, "training_ingest_output");

        (redacted_in.redacted_text, redacted_out.redacted_text)
    } else {
        (input_text.to_string(), output_text.to_string())
    };

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    // Build metadata blob.
    let mut metadata = serde_json::json!({
        "source": source.as_str(),
        "auto_training": true,
        "ingested_at": now,
    });

    if let Some(extra) = extra_metadata {
        if let Some(obj) = metadata.as_object_mut() {
            if let Some(extra_obj) = extra.as_object() {
                for (k, v) in extra_obj {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
    }

    let metadata_str =
        serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string());

    {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;

        conn_guard
            .execute(
                "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    id,
                    project_id,
                    local_model_id,
                    final_input,
                    final_output,
                    metadata_str,
                    now
                ],
            )
            .map_err(|e| format!("Failed to insert training data: {}", e))?;
    }

    // App settings already handle cache invalidation via `create_training_data`,
    // but auto‑training writes directly, so we need to invalidate cache here.
    // This is intentionally best‑effort – failures shouldn't break chat.
    if let Some(mid) = local_model_id {
        let cache = crate::cache::training_cache::TrainingCache::new(db.clone());
        let _ = cache.invalidate_cache(project_id, Some(mid));
    } else {
        let cache = crate::cache::training_cache::TrainingCache::new(db.clone());
        let _ = cache.invalidate_cache(project_id, None);
    }

    Ok(())
}

/// Ingest a profile chat turn (user + assistant) into training_data.
///
/// `project_id` and `local_model_id` are passed from the caller – profile chat
/// itself is not project‑scoped, so the UI / higher level decides which
/// project/model should benefit from these examples.
#[allow(dead_code)]
pub fn ingest_chat_turn(
    db: &Database,
    project_id: &str,
    local_model_id: Option<&str>,
    user_text: &str,
    assistant_text: &str,
    profile_id: &str,
) -> Result<(), String> {
    let app_settings = load_settings_sync(db);

    // If auto‑training is disabled globally or for chat, do nothing.
    if !app_settings
        .auto_training
        .auto_training_enabled
        || !app_settings.auto_training.train_from_chat
    {
        return Ok(());
    }

    let metadata = serde_json::json!({
        "profile_id": profile_id,
    });

    insert_training_row(
        db,
        project_id,
        local_model_id,
        user_text,
        assistant_text,
        TrainingSource::ProfileChat,
        Some(metadata),
    )
}

/// Ingest a Coder IDE turn (user + assistant) into training_data.
pub fn ingest_coder_turn(
    db: &Database,
    project_id: &str,
    local_model_id: Option<&str>,
    user_text: &str,
    assistant_text: &str,
    context_files: &[String],
    terminal_output: Option<&str>,
) -> Result<(), String> {
    let app_settings = load_settings_sync(db);

    if !app_settings
        .auto_training
        .auto_training_enabled
        || !app_settings.auto_training.train_from_coder
    {
        return Ok(());
    }

    let metadata = serde_json::json!({
        "context_files": context_files,
        "terminal_output_present": terminal_output.is_some(),
    });

    insert_training_row(
        db,
        project_id,
        local_model_id,
        user_text,
        assistant_text,
        TrainingSource::CoderIDE,
        Some(metadata),
    )
}

/// Ingest a Debate turn (user + agent) into training_data.
pub fn ingest_debate_turn(
    db: &Database,
    project_id: &str,
    local_model_id: Option<&str>,
    user_text: &str,
    agent_text: &str,
    session_id: &str,
    run_id: &str,
) -> Result<(), String> {
    let app_settings = load_settings_sync(db);

    if !app_settings
        .auto_training
        .auto_training_enabled
        || !app_settings.auto_training.train_from_debate
    {
        return Ok(());
    }

    let metadata = serde_json::json!({
        "session_id": session_id,
        "run_id": run_id,
    });

    insert_training_row(
        db,
        project_id,
        local_model_id,
        user_text,
        agent_text,
        TrainingSource::DebateRoom,
        Some(metadata),
    )
}

/// Ingest a ClineIDE turn (user + assistant + tool executions) into training_data.
pub fn ingest_cline_turn(
    db: &Database,
    project_id: &str,
    local_model_id: Option<&str>,
    user_text: &str,
    assistant_text: &str,
    tool_executions: &[serde_json::Value],
    browser_steps: Option<&[serde_json::Value]>,
    error_context: Option<&str>,
) -> Result<(), String> {
    let app_settings = load_settings_sync(db);

    if !app_settings
        .auto_training
        .auto_training_enabled
        || !app_settings.auto_training.train_from_coder
    {
        return Ok(());
    }

    let metadata = serde_json::json!({
        "tool_executions": tool_executions,
        "browser_steps": browser_steps,
        "error_context": error_context,
    });    insert_training_row(
        db,
        project_id,
        local_model_id,
        user_text,
        assistant_text,
        TrainingSource::ClineIDE,
        Some(metadata),
    )
}
