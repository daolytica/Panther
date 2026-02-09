// Tauri commands for frontend-backend communication
// userId is camelCase to match Tauri invoke from frontend
#![allow(non_snake_case)]

use crate::db::Database;
use crate::keychain::Keychain;
use crate::providers::get_adapter;
use crate::types::ProviderAccount;
use crate::orchestrator::Orchestrator;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use chrono::Utc;
use serde_json;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateProviderRequest {
    pub provider_type: String,
    pub display_name: String,
    pub base_url: Option<String>,
    pub region: Option<String>,
    pub api_key: Option<String>,
    pub provider_metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateSessionRequest {
    pub project_id: String,
    pub title: String,
    pub user_question: String,
    pub mode: String, // "parallel" or "debate"
    pub selected_profile_ids: Vec<String>,
    pub run_settings: Option<serde_json::Value>,
    pub local_model_id: Option<String>, // Optional trained local model to use
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateProfileRequest {
    pub name: String,
    pub provider_account_id: String,
    pub model_name: String,
    pub persona_prompt: String,
    pub character_definition_json: Option<serde_json::Value>,
    pub model_features_json: Option<serde_json::Value>,
    pub params_json: serde_json::Value,
    pub photo_url: Option<String>,
    /// Voice gender preference for TTS: "male" | "female" | "neutral" | "any"
    pub voice_gender: Option<String>,
    /// Specific voice URI from speechSynthesis.getVoices()
    pub voice_uri: Option<String>,
}

// Provider commands - impl versions for HTTP server
pub async fn create_provider_impl(db: &Database, request: CreateProviderRequest, user_id: Option<String>) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    let auth_ref = if let Some(api_key) = &request.api_key {
        // Only store if API key is not empty
        if !api_key.trim().is_empty() {
            let keychain = Keychain::new();
            let auth_ref = format!("provider_{}", id);
            keychain.store("panther", &auth_ref, api_key.trim())
                .map_err(|e| format!("Failed to store API key in keychain: {}", e))?;
            Some(auth_ref)
        } else {
            None
        }
    } else {
        None
    };

    let metadata_json = request
        .provider_metadata_json
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());

    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "INSERT INTO provider_accounts (id, provider_type, display_name, base_url, region, auth_ref, provider_metadata_json, created_at, updated_at, user_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            rusqlite::params![
                id,
                request.provider_type,
                request.display_name,
                request.base_url,
                request.region,
                auth_ref,
                metadata_json,
                now,
                now,
                user_id
            ],
        )
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(id)
}

#[tauri::command]
pub async fn create_provider(db: State<'_, Database>, request: CreateProviderRequest, userId: Option<String>) -> Result<String, String> {
    create_provider_impl(&db, request, userId).await
}

fn map_provider_row(row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    Ok(serde_json::json!({
        "id": row.get::<_, String>(0)?,
        "provider_type": row.get::<_, String>(1)?,
        "display_name": row.get::<_, String>(2)?,
        "base_url": row.get::<_, Option<String>>(3)?,
        "region": row.get::<_, Option<String>>(4)?,
        "auth_ref": row.get::<_, Option<String>>(5)?,
        "created_at": row.get::<_, String>(6)?,
        "updated_at": row.get::<_, String>(7)?,
        "provider_metadata_json": row.get::<_, Option<String>>(8)?
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
    }))
}

pub async fn list_providers_impl(db: &Database, user_id: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let mut providers = Vec::new();
    if let Some(uid) = &user_id {
        let mut stmt = conn_guard
            .prepare("SELECT id, provider_type, display_name, base_url, region, auth_ref, created_at, updated_at, provider_metadata_json FROM provider_accounts WHERE user_id = ?1 OR user_id IS NULL ORDER BY created_at DESC")
            .map_err(|e| format!("Database error: {}", e))?;
        let rows = stmt.query_map(rusqlite::params![uid], map_provider_row).map_err(|e| format!("Database error: {}", e))?;
        for row in rows {
            providers.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
    } else {
        let mut stmt = conn_guard
            .prepare("SELECT id, provider_type, display_name, base_url, region, auth_ref, created_at, updated_at, provider_metadata_json FROM provider_accounts WHERE user_id IS NULL ORDER BY created_at DESC")
            .map_err(|e| format!("Database error: {}", e))?;
        let rows = stmt.query_map([], map_provider_row).map_err(|e| format!("Database error: {}", e))?;
        for row in rows {
            providers.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
    }
    Ok(providers)
}


#[tauri::command]
pub async fn list_providers(db: State<'_, Database>, userId: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    list_providers_impl(&db, userId).await
}

pub async fn update_provider_impl(db: &Database, id: String, request: CreateProviderRequest, _user_id: Option<String>) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    
    // Handle API key update - only update if a new non-empty key is provided
    if let Some(api_key) = &request.api_key {
        if !api_key.trim().is_empty() {
            let keychain = Keychain::new();
            let auth_ref = format!("provider_{}", id);
            
            // Store or update API key under the "panther" service. keyring::Entry::set_password
            // will create or overwrite as needed, so we don't need an existence check here.
            keychain.store("panther", &auth_ref, api_key.trim())
                .map_err(|e| format!("Failed to store API key in keychain: {}", e))?;

            // Ensure database has the correct auth_ref
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
            conn_guard.execute(
                "UPDATE provider_accounts SET auth_ref = ?1 WHERE id = ?2",
                rusqlite::params![auth_ref, id],
            )
            .map_err(|e| format!("Database error: {}", e))?;
        }
    }
    // If no API key provided, preserve the existing one (don't touch keychain or auth_ref)

    let metadata_json = request.provider_metadata_json
        .map(|v| serde_json::to_string(&v).ok())
        .flatten();
    
    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "UPDATE provider_accounts SET provider_type = ?1, display_name = ?2, base_url = ?3, region = ?4, provider_metadata_json = ?5, updated_at = ?6 WHERE id = ?7",
            rusqlite::params![
                request.provider_type,
                request.display_name,
                request.base_url,
                request.region,
                metadata_json,
                now,
                id
            ],
        )
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn update_provider(db: State<'_, Database>, id: String, request: CreateProviderRequest, userId: Option<String>) -> Result<(), String> {
    update_provider_impl(&db, id, request, userId).await
}

pub async fn delete_provider_impl(db: &Database, id: String, _user_id: Option<String>) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let auth_ref: Option<String> = conn_guard
        .query_row(
            "SELECT auth_ref FROM provider_accounts WHERE id = ?1",
            [&id],
            |row| row.get(0),
        )
        .ok();

    if let Some(auth_ref) = auth_ref {
        let keychain = Keychain::new();
        keychain.delete("panther", &auth_ref).ok();
    }

    conn_guard
        .execute("DELETE FROM provider_accounts WHERE id = ?1", [&id])
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_provider(db: State<'_, Database>, id: String, userId: Option<String>) -> Result<(), String> {
    delete_provider_impl(&db, id, userId).await
}

pub async fn test_provider_connection_impl(db: &Database, provider_id: String) -> Result<bool, String> {
    let chain = crate::provider_resolver::resolve_provider_chain(db, &provider_id)?;

    let primary_adapter: Box<dyn crate::providers::ProviderAdapter> =
        get_adapter(&chain.primary.provider_type).map_err(|e| {
            format!(
                "Failed to get adapter for provider type '{}': {}",
                chain.primary.provider_type, e
            )
        })?;

    let primary_ok = primary_adapter
        .validate(&chain.primary)
        .await
        .map_err(|e| format!("Validation error (primary): {}", e))?;

    if let Some((fallback_provider, _fallback_model)) = chain.fallback {
        let fallback_adapter: Box<dyn crate::providers::ProviderAdapter> =
            get_adapter(&fallback_provider.provider_type).map_err(|e| {
                format!(
                    "Failed to get adapter for provider type '{}': {}",
                    fallback_provider.provider_type, e
                )
            })?;
        let fallback_ok = fallback_adapter
            .validate(&fallback_provider)
            .await
            .map_err(|e| format!("Validation error (fallback): {}", e))?;
        return Ok(primary_ok && fallback_ok);
    }

    Ok(primary_ok)
}

#[tauri::command]
pub async fn test_provider_connection(db: State<'_, Database>, provider_id: String) -> Result<bool, String> {
    test_provider_connection_impl(&db, provider_id).await
}

pub async fn list_provider_models_impl(db: &Database, provider_id: String) -> Result<Vec<String>, String> {
    let chain = crate::provider_resolver::resolve_provider_chain(db, &provider_id)?;

    let adapter: Box<dyn crate::providers::ProviderAdapter> = get_adapter(&chain.primary.provider_type)
        .map_err(|e| format!("Failed to get adapter: {}", e))?;

    adapter
        .list_models(&chain.primary)
        .await
        .map_err(|e| format!("Failed to list models: {}", e))
}

#[tauri::command]
pub async fn list_provider_models(db: State<'_, Database>, provider_id: String) -> Result<Vec<String>, String> {
    list_provider_models_impl(&db, provider_id).await
}

// Profile commands
pub async fn create_profile_impl(db: &Database, request: CreateProfileRequest, user_id: Option<String>) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let params_json_str = serde_json::to_string(&request.params_json)
        .map_err(|e| format!("Failed to serialize params: {}", e))?;

    let character_json_str = request.character_definition_json
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());
    let features_json_str = request.model_features_json
        .as_ref()
        .and_then(|v| serde_json::to_string(v).ok());

    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "INSERT INTO prompt_profiles (id, name, provider_account_id, model_name, persona_prompt, character_definition_json, model_features_json, params_json, photo_url, voice_gender, voice_uri, created_at, updated_at, user_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                id,
                request.name,
                request.provider_account_id,
                request.model_name,
                request.persona_prompt,
                character_json_str,
                features_json_str,
                params_json_str,
                request.photo_url,
                request.voice_gender,
                request.voice_uri,
                now,
                now,
                user_id
            ],
        )
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(id)
}

#[tauri::command]
pub async fn create_profile(db: State<'_, Database>, request: CreateProfileRequest, userId: Option<String>) -> Result<String, String> {
    create_profile_impl(&db, request, userId).await
}

pub async fn update_profile_impl(db: &Database, id: String, request: CreateProfileRequest) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let character_json_str = request.character_definition_json
        .as_ref()
        .map(|v| serde_json::to_string(v))
        .transpose()
        .map_err(|e| format!("Failed to serialize character definition: {}", e))?;
    
    let features_json_str = request.model_features_json
        .as_ref()
        .map(|v| serde_json::to_string(v))
        .transpose()
        .map_err(|e| format!("Failed to serialize model features: {}", e))?;
    
    let params_json_str = serde_json::to_string(&request.params_json)
        .map_err(|e| format!("Failed to serialize params: {}", e))?;
    
    let now = Utc::now().to_rfc3339();
    
    conn_guard.execute(
        "UPDATE prompt_profiles SET name = ?1, provider_account_id = ?2, model_name = ?3, persona_prompt = ?4, character_definition_json = ?5, model_features_json = ?6, params_json = ?7, photo_url = ?8, voice_gender = ?9, voice_uri = ?10, updated_at = ?11 WHERE id = ?12",
        rusqlite::params![
            request.name,
            request.provider_account_id,
            request.model_name,
            request.persona_prompt,
            character_json_str,
            features_json_str,
            params_json_str,
            request.photo_url,
            request.voice_gender,
            request.voice_uri,
            now,
            id
        ],
    )
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn update_profile(db: State<'_, Database>, id: String, request: CreateProfileRequest, _userId: Option<String>) -> Result<(), String> {
    update_profile_impl(&db, id, request).await
}

fn map_profile_row(row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    let voice_gender = row.get::<_, Option<String>>(12).ok().flatten();
    let voice_uri = row.get::<_, Option<String>>(13).ok().flatten();
    Ok(serde_json::json!({
        "id": row.get::<_, String>(0)?,
        "name": row.get::<_, String>(1)?,
        "provider_account_id": row.get::<_, String>(2)?,
        "model_name": row.get::<_, String>(3)?,
        "persona_prompt": row.get::<_, String>(4)?,
        "character_definition": row.get::<_, Option<String>>(5)?
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
        "model_features": row.get::<_, Option<String>>(6)?
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()),
        "params_json": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(7)?).unwrap_or(serde_json::json!({})),
        "output_preset_id": row.get::<_, Option<String>>(8)?,
        "photo_url": row.get::<_, Option<String>>(9)?,
        "created_at": row.get::<_, String>(10)?,
        "updated_at": row.get::<_, String>(11)?,
        "voice_gender": voice_gender,
        "voice_uri": voice_uri,
    }))
}

pub async fn list_profiles_impl(db: &Database, user_id: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let mut profiles = Vec::new();
    if let Some(uid) = &user_id {
        let mut stmt = conn_guard
            .prepare("SELECT id, name, provider_account_id, model_name, persona_prompt, character_definition_json, model_features_json, params_json, output_preset_id, photo_url, created_at, updated_at, voice_gender, voice_uri FROM prompt_profiles WHERE user_id = ?1 OR user_id IS NULL ORDER BY created_at DESC")
            .map_err(|e| format!("Database error: {}", e))?;
        let rows = stmt.query_map(rusqlite::params![uid], map_profile_row).map_err(|e| format!("Database error: {}", e))?;
        for row in rows {
            profiles.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
    } else {
        let mut stmt = conn_guard
            .prepare("SELECT id, name, provider_account_id, model_name, persona_prompt, character_definition_json, model_features_json, params_json, output_preset_id, photo_url, created_at, updated_at, voice_gender, voice_uri FROM prompt_profiles WHERE user_id IS NULL ORDER BY created_at DESC")
            .map_err(|e| format!("Database error: {}", e))?;
        let rows = stmt.query_map([], map_profile_row).map_err(|e| format!("Database error: {}", e))?;
        for row in rows {
            profiles.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
    }
    Ok(profiles)
}

#[tauri::command]
pub async fn list_profiles(db: State<'_, Database>, userId: Option<String>) -> Result<Vec<serde_json::Value>, String> {
    list_profiles_impl(&db, userId).await
}

// Project commands
#[tauri::command]
pub async fn create_project(
    db: State<'_, Database>,
    request: CreateProjectRequest,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "INSERT INTO projects (id, name, description, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            rusqlite::params![id, request.name, request.description, now, now],
        )
        .map_err(|e| format!("Database error: {}", e))?;

    Ok(id)
}

pub async fn list_projects_impl(
    db: &Database,
    user_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let (query, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = match &user_id {
        Some(uid) => (
            "SELECT id, name, description, created_at, updated_at FROM projects WHERE user_id = ?1 OR user_id IS NULL ORDER BY created_at DESC",
            vec![Box::new(uid.clone()) as Box<dyn rusqlite::ToSql>],
        ),
        None => (
            "SELECT id, name, description, created_at, updated_at FROM projects WHERE user_id IS NULL ORDER BY created_at DESC",
            vec![],
        ),
    };
    
    let mut stmt = conn_guard
        .prepare(query)
        .map_err(|e| format!("Database error: {}", e))?;

    let rows = stmt
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "name": row.get::<_, String>(1)?,
                "description": row.get::<_, Option<String>>(2)?,
                "created_at": row.get::<_, String>(3)?,
                "updated_at": row.get::<_, String>(4)?,
            }))
        })
        .map_err(|e| format!("Database error: {}", e))?;

    let mut projects = Vec::new();
    for row in rows {
        projects.push(row.map_err(|e| format!("Row error: {}", e))?);
    }

    Ok(projects)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_projects(
    db: State<'_, Database>,
    userId: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    list_projects_impl(&*db, userId).await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateProjectRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[tauri::command]
pub async fn update_project(
    db: State<'_, Database>,
    request: UpdateProjectRequest,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "UPDATE projects SET name = ?1, description = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![request.name, request.description, now, request.id],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn delete_project(
    db: State<'_, Database>,
    project_id: String,
) -> Result<(), String> {
    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "DELETE FROM projects WHERE id = ?1",
            rusqlite::params![project_id],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn move_session_to_project(
    db: State<'_, Database>,
    session_id: String,
    project_id: String,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "UPDATE sessions SET project_id = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![project_id, now, session_id],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    Ok(())
}

// Session commands
pub async fn create_session_impl(db: &Database, request: CreateSessionRequest) -> Result<String, String> {
    let session_id = Uuid::new_v4().to_string();
    let run_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    let profile_ids_json = serde_json::to_string(&request.selected_profile_ids)
        .map_err(|e| format!("Failed to serialize profile IDs: {}", e))?;
    
    let run_settings = request.run_settings
        .unwrap_or_else(|| serde_json::json!({"concurrency": 3, "streaming": true}));
    let run_settings_json = serde_json::to_string(&run_settings)
        .map_err(|e| format!("Failed to serialize run settings: {}", e))?;

    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Create session
    let local_model_id = request.local_model_id.as_ref();
    conn_guard.execute(
        "INSERT INTO sessions (id, project_id, title, user_question, mode, local_model_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            session_id,
            request.project_id,
            request.title,
            request.user_question,
            request.mode,
            local_model_id,
            now,
            now
        ],
    )
    .map_err(|e| format!("Database error: {}", e))?;

    // Create run
    conn_guard.execute(
        "INSERT INTO runs (id, session_id, selected_profile_ids_json, status, run_settings_json, started_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![
            run_id,
            session_id,
            profile_ids_json,
            "queued",
            run_settings_json,
            now
        ],
    )
    .map_err(|e| format!("Database error: {}", e))?;

    Ok(run_id)
}

#[tauri::command]
pub async fn create_session(
    db: State<'_, Database>,
    request: CreateSessionRequest,
) -> Result<String, String> {
    create_session_impl(&*db, request).await
}

pub async fn list_sessions_impl(
    db: &Database,
    _user_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    // Note: sessions table doesn't have user_id column yet, so we return all sessions
    // In the future, sessions should be linked to users through projects
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let mut stmt = conn_guard
        .prepare("SELECT id, project_id, title, user_question, mode, global_prompt_template_id, created_at, updated_at FROM sessions ORDER BY created_at DESC")
        .map_err(|e| format!("Database error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "project_id": row.get::<_, String>(1)?,
                "title": row.get::<_, String>(2)?,
                "user_question": row.get::<_, String>(3)?,
                "mode": row.get::<_, String>(4)?,
                "global_prompt_template_id": row.get::<_, Option<String>>(5)?,
                "created_at": row.get::<_, String>(6)?,
                "updated_at": row.get::<_, String>(7)?,
            }))
        })
        .map_err(|e| format!("Database error: {}", e))?;

    let mut sessions = Vec::new();
    for row in rows {
        sessions.push(row.map_err(|e| format!("Row error: {}", e))?);
    }

    Ok(sessions)
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_sessions(
    db: State<'_, Database>,
    userId: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    list_sessions_impl(&*db, userId).await
}

pub async fn get_session_run_impl(db: &Database, session_id: &str) -> Result<Option<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let result: Result<(String, String), _> = conn_guard.query_row(
        "SELECT id, status FROM runs WHERE session_id = ?1 ORDER BY started_at DESC LIMIT 1",
        [session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    
    match result {
        Ok((run_id, status)) => Ok(Some(serde_json::json!({
            "run_id": run_id,
            "status": status,
        }))),
        Err(_) => Ok(None),
    }
}

#[tauri::command]
pub async fn get_session_run(
    db: State<'_, Database>,
    session_id: String,
) -> Result<Option<serde_json::Value>, String> {
    get_session_run_impl(&*db, &session_id).await
}

#[tauri::command]
pub async fn get_session(
    db: State<'_, Database>,
    session_id: String,
) -> Result<Option<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let result: Result<(String, String, String, String, String), _> = conn_guard.query_row(
        "SELECT id, title, user_question, mode, created_at FROM sessions WHERE id = ?1",
        [&session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
    );
    
    match result {
        Ok((id, title, user_question, mode, created_at)) => Ok(Some(serde_json::json!({
            "id": id,
            "title": title,
            "user_question": user_question,
            "mode": mode,
            "created_at": created_at,
        }))),
        Err(_) => Ok(None),
    }
}

pub async fn delete_session_impl(db: &Database, session_id: &str) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    conn_guard.execute(
        "DELETE FROM sessions WHERE id = ?1",
        rusqlite::params![session_id],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn delete_session(
    db: State<'_, Database>,
    session_id: String,
) -> Result<(), String> {
    delete_session_impl(&*db, &session_id).await
}

// Keychain commands
#[tauri::command]
pub async fn store_api_key(
    service: String,
    username: String,
    password: String,
) -> Result<(), String> {
    let keychain = Keychain::new();
    keychain.store(&service, &username, &password)
        .map_err(|e| format!("Failed to store: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn retrieve_api_key(
    service: String,
    username: String,
) -> Result<String, String> {
    let keychain = Keychain::new();
    keychain.retrieve(&service, &username)
        .map_err(|e| format!("Failed to retrieve: {}", e))
}

#[tauri::command]
pub async fn delete_api_key(
    service: String,
    username: String,
) -> Result<(), String> {
    let keychain = Keychain::new();
    keychain.delete(&service, &username)
        .map_err(|e| format!("Failed to delete: {}", e))?;
    Ok(())
}

// Run execution commands
#[tauri::command]
pub async fn start_run(
    db: State<'_, Database>,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
) -> Result<(), String> {
    // Check if run is already running or complete
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let status: Result<String, _> = conn_guard.query_row(
        "SELECT status FROM runs WHERE id = ?1",
        [&run_id],
        |row| row.get(0),
    );
    
    if let Ok(current_status) = status {
        if current_status == "running" || current_status == "complete" || current_status == "partial" {
            // Run already started or completed, don't start again
            return Ok(());
        }
    }
    
    // Use the shared orchestrator instance
    let orchestrator_inner = orchestrator.inner();
    
    // Run in background - clone the orchestrator for the task
    let orchestrator_clone = orchestrator_inner.clone();
    tokio::spawn(async move {
        if let Err(e) = orchestrator_clone.run_parallel_brainstorm(run_id).await {
            eprintln!("Run execution error: {}", e);
        }
    });
    
    Ok(())
}

#[tauri::command]
pub async fn get_run_status(
    db: State<'_, Database>,
    run_id: String,
) -> Result<serde_json::Value, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let run_data: Result<(String, String, Option<String>, String, String, Option<String>), _> = conn_guard.query_row(
        "SELECT status, started_at, finished_at, session_id, selected_profile_ids_json, error_message_safe FROM runs WHERE id = ?1",
        [&run_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5).ok().flatten())),
    );
    
    let (status, started_at, finished_at, session_id, profile_ids_json, error_message) = run_data
        .map_err(|e| format!("Failed to load run: {}", e))?;
    
    let profile_ids: Vec<String> = serde_json::from_str(&profile_ids_json)
        .unwrap_or_default();
    
    let mut result = serde_json::json!({
        "status": status,
        "started_at": started_at,
        "finished_at": finished_at,
        "session_id": session_id,
        "selected_profile_ids": profile_ids,
    });
    if let Some(err) = error_message {
        result["error_message"] = serde_json::json!(err);
    }
    Ok(result)
}

#[tauri::command]
pub async fn get_run_results(
    db: State<'_, Database>,
    run_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let mut stmt = conn_guard
        .prepare("SELECT id, profile_id, status, raw_output_text, error_message_safe, started_at, finished_at FROM run_results WHERE run_id = ?1 ORDER BY started_at")
        .map_err(|e| format!("Database error: {}", e))?;
    
    let rows = stmt
        .query_map([&run_id], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "profile_id": row.get::<_, String>(1)?,
                "status": row.get::<_, String>(2)?,
                "raw_output_text": row.get::<_, Option<String>>(3)?,
                "error_message_safe": row.get::<_, Option<String>>(4)?,
                "started_at": row.get::<_, String>(5)?,
                "finished_at": row.get::<_, Option<String>>(6)?,
            }))
        })
        .map_err(|e| format!("Database error: {}", e))?;
    
    let mut results = Vec::new();
    for row in rows {
        results.push(row.map_err(|e| format!("Row error: {}", e))?);
    }
    
    Ok(results)
}

#[tauri::command]
pub async fn cancel_run(
    db: State<'_, Database>,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
) -> Result<(), String> {
    // Mark run as cancelled in orchestrator
    orchestrator.cancel_run(&run_id).await;
    
    // Update run status in database
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let finished_at = chrono::Utc::now().to_rfc3339();
    conn_guard.execute(
        "UPDATE runs SET status = 'cancelled', finished_at = ?1 WHERE id = ?2",
        rusqlite::params![finished_at, run_id],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn cancel_run_result(
    db: State<'_, Database>,
    orchestrator: State<'_, Orchestrator>,
    result_id: String,
) -> Result<(), String> {
    // Mark result as cancelled in orchestrator
    orchestrator.cancel_result(&result_id).await;
    
    // Update result status in database
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    let finished_at = chrono::Utc::now().to_rfc3339();
    conn_guard.execute(
        "UPDATE run_results SET status = 'cancelled', finished_at = ?1 WHERE id = ?2",
        rusqlite::params![finished_at, result_id],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn delete_run_result(
    db: State<'_, Database>,
    result_id: String,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    conn_guard.execute(
        "DELETE FROM run_results WHERE id = ?1",
        rusqlite::params![result_id],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn rerun_single_agent(
    db: State<'_, Database>,
    orchestrator: State<'_, Orchestrator>,
    run_id: String,
    profile_id: String,
) -> Result<String, String> {
    // Get the user question from the session
    let user_question: String = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        conn_guard.query_row(
            "SELECT s.user_question FROM sessions s 
             JOIN runs r ON r.session_id = s.id 
             WHERE r.id = ?1",
            [&run_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to get session: {}", e))?
    };
    
    // Create a new result entry for this profile
    let result_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    
    {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        conn_guard.execute(
            "INSERT INTO run_results (id, run_id, profile_id, status, started_at) VALUES (?1, ?2, ?3, 'running', ?4)",
            rusqlite::params![result_id, run_id, profile_id, started_at],
        )
        .map_err(|e| format!("Failed to create result: {}", e))?;
    }
    
    // Run the agent in background
    let orchestrator_clone = orchestrator.inner().clone();
    let db_clone = db.inner().clone();
    let result_id_clone = result_id.clone();
    
    tokio::spawn(async move {
        if let Err(e) = orchestrator_clone.run_single_agent(
            &db_clone,
            &run_id,
            &profile_id,
            &user_question,
            &result_id_clone,
        ).await {
            eprintln!("Rerun agent error: {}", e);
        }
    });
    
    Ok(result_id)
}

#[tauri::command]
pub async fn continue_agent(
    db: State<'_, Database>,
    orchestrator: State<'_, Orchestrator>,
    result_id: String,
    follow_up_message: String,
) -> Result<String, String> {
    // Get the existing result info
    let (run_id, profile_id, previous_output): (String, String, Option<String>) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        conn_guard.query_row(
            "SELECT run_id, profile_id, raw_output_text FROM run_results WHERE id = ?1",
            [&result_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .map_err(|e| format!("Failed to get result: {}", e))?
    };
    
    // Get the original user question
    let original_question: String = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        conn_guard.query_row(
            "SELECT s.user_question FROM sessions s 
             JOIN runs r ON r.session_id = s.id 
             WHERE r.id = ?1",
            [&run_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to get session: {}", e))?
    };
    
    // Create a new result entry for the continuation
    let new_result_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    
    {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        conn_guard.execute(
            "INSERT INTO run_results (id, run_id, profile_id, status, started_at) VALUES (?1, ?2, ?3, 'running', ?4)",
            rusqlite::params![new_result_id, run_id, profile_id, started_at],
        )
        .map_err(|e| format!("Failed to create result: {}", e))?;
    }
    
    // Run the continuation in background
    let orchestrator_clone = orchestrator.inner().clone();
    let db_clone = db.inner().clone();
    let new_result_id_clone = new_result_id.clone();
    
    tokio::spawn(async move {
        if let Err(e) = orchestrator_clone.continue_agent(
            &db_clone,
            &run_id,
            &profile_id,
            &original_question,
            previous_output.as_deref(),
            &follow_up_message,
            &new_result_id_clone,
        ).await {
            eprintln!("Continue agent error: {}", e);
        }
    });
    
    Ok(new_result_id)
}

#[tauri::command]
pub async fn generate_comparison_table(
    db: State<'_, Database>,
    run_id: String,
) -> Result<String, String> {
    // Get run results and session question
    let (user_question, results_data): (String, Vec<(String, String, Option<String>)>) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        // Get session question
        let question: String = conn_guard.query_row(
            "SELECT s.user_question FROM sessions s 
             JOIN runs r ON r.session_id = s.id 
             WHERE r.id = ?1",
            [&run_id],
            |row| row.get(0),
        )
        .map_err(|e| format!("Failed to get session: {}", e))?;
        
        // Get all results with profile names
        let mut stmt = conn_guard
            .prepare("SELECT rr.profile_id, pp.name, rr.raw_output_text 
                      FROM run_results rr 
                      JOIN prompt_profiles pp ON pp.id = rr.profile_id 
                      WHERE rr.run_id = ?1 AND rr.status = 'complete' AND rr.raw_output_text IS NOT NULL
                      ORDER BY rr.started_at")
            .map_err(|e| format!("Database error: {}", e))?;
        
        let rows = stmt
            .query_map([&run_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?))
            })
            .map_err(|e| format!("Database error: {}", e))?;
        
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        
        (question, results)
    };
    
    if results_data.is_empty() {
        return Err("No completed results found for comparison".to_string());
    }
    
    // Build the prompt for comparison
    let mut comparison_prompt = format!(
        "You are an expert analyst. Please analyze the following responses to the question: \"{}\"\n\n",
        user_question
    );
    
    comparison_prompt.push_str("## Agent Responses:\n\n");
    for (_profile_id, profile_name, output) in &results_data {
        if let Some(text) = output {
            comparison_prompt.push_str(&format!("### {}\n{}\n\n", profile_name, text));
        }
    }
    
    comparison_prompt.push_str(&format!(
        "## Task:\n\n\
        Please create a comprehensive comparison table in Markdown format that includes:\n\
        1. Key points/arguments from each agent\n\
        2. Areas of agreement\n\
        3. Areas of disagreement\n\
        4. Unique insights from each agent\n\
        5. Overall assessment and recommendations\n\n\
        Format the response as a well-structured Markdown document with clear sections and a comparison table.\n\
        Use markdown table syntax for the comparison.\n"
    ));
    
    // Find an OpenAI-compatible provider to use for comparison
    let (provider_account, model_name, params_json): (ProviderAccount, String, serde_json::Value) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        // Try to find an OpenAI-compatible provider
        let provider_data: Result<(String, String, String, Option<String>, Option<String>, Option<String>), _> = conn_guard.query_row(
            "SELECT id, provider_type, display_name, base_url, region, auth_ref 
             FROM provider_accounts 
             WHERE provider_type = 'openai_compatible' 
             LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        );
        
        let (provider_id, provider_type, display_name, base_url, region, auth_ref) = provider_data
            .map_err(|_| "No OpenAI-compatible provider found. Please add a provider first.")?;
        
        // Get a profile that uses this provider to get model and params
        let profile_data: Result<(String, String), _> = conn_guard.query_row(
            "SELECT model_name, params_json 
             FROM prompt_profiles 
             WHERE provider_account_id = ?1 
             LIMIT 1",
            [&provider_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        
        let (model, params_json_str) = profile_data
            .map_err(|_| "No profile found for the provider. Please create a profile first.")?;
        
        let provider_account = ProviderAccount {
            id: provider_id,
            provider_type,
            display_name,
            base_url,
            region,
            auth_ref,
            created_at: String::new(),
            updated_at: String::new(),
            provider_metadata_json: None,
        };
        
        let params: serde_json::Value = serde_json::from_str(&params_json_str)
            .unwrap_or(serde_json::json!({}));
        
        (provider_account, model, params)
    };
    
    // Get adapter and make the API call
    let adapter: Box<dyn crate::providers::ProviderAdapter> = crate::providers::get_adapter(&provider_account.provider_type)
        .map_err(|e| format!("Failed to get adapter: {}", e))?;
    
    let packet = crate::types::PromptPacket {
        global_instructions: None,
        persona_instructions: "You are an expert analyst who creates clear, structured comparisons and analysis. Format your response in Markdown with tables where appropriate.".to_string(),
        user_message: comparison_prompt,
        conversation_context: None,
        params_json,
        stream: false,
    };
    
    let response = adapter.complete(&packet, &provider_account, &model_name).await
        .map_err(|e| format!("Failed to generate comparison: {}", e))?;
    
    Ok(response.text)
}
