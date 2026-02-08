// Orchestrator for running parallel brainstorming sessions

use crate::db::Database;
use crate::provider_resolver::complete_resolving_hybrid;
use crate::types::PromptPacket;
use crate::rag;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Semaphore, RwLock};
use uuid::Uuid;

#[derive(Clone)]
pub struct Orchestrator {
    db: Database,
    cancelled_runs: Arc<RwLock<std::collections::HashSet<String>>>,
    cancelled_results: Arc<RwLock<std::collections::HashSet<String>>>,
}

impl Orchestrator {
    pub fn new(db: Database) -> Self {
        Orchestrator { 
            db,
            cancelled_runs: Arc::new(RwLock::new(std::collections::HashSet::new())),
            cancelled_results: Arc::new(RwLock::new(std::collections::HashSet::new())),
        }
    }

    pub async fn cancel_run(&self, run_id: &str) {
        let mut cancelled = self.cancelled_runs.write().await;
        cancelled.insert(run_id.to_string());
    }

    pub async fn cancel_result(&self, result_id: &str) {
        let mut cancelled = self.cancelled_results.write().await;
        cancelled.insert(result_id.to_string());
    }

    #[allow(dead_code)]
    async fn is_run_cancelled(&self, run_id: &str) -> bool {
        let cancelled = self.cancelled_runs.read().await;
        cancelled.contains(run_id)
    }

    #[allow(dead_code)]
    async fn is_result_cancelled(&self, result_id: &str) -> bool {
        let cancelled = self.cancelled_results.read().await;
        cancelled.contains(result_id)
    }

    pub async fn run_parallel_brainstorm(
        &self,
        run_id: String,
    ) -> Result<()> {
        // Load run data
        let (_session_id, profile_ids, user_question, run_settings): (String, Vec<String>, String, Value) = {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            
            let run_data: Result<(String, String, String), _> = conn_guard.query_row(
                "SELECT session_id, selected_profile_ids_json, run_settings_json FROM runs WHERE id = ?1",
                [&run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            );
            
            let (session_id, profile_ids_json, run_settings_json) = run_data
                .map_err(|e| anyhow::anyhow!("Failed to load run: {}", e))?;
            
            let session_data: Result<(String, Option<String>), _> = conn_guard.query_row(
                "SELECT user_question, local_model_id FROM sessions WHERE id = ?1",
                [&session_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );
            
            let (user_question, _session_local_model_id) = session_data
                .map_err(|e| anyhow::anyhow!("Failed to load session: {}", e))?;
            
            let profile_ids: Vec<String> = serde_json::from_str(&profile_ids_json)
                .map_err(|e| anyhow::anyhow!("Failed to parse profile IDs: {}", e))?;
            
            let run_settings: Value = serde_json::from_str(&run_settings_json)
                .map_err(|e| anyhow::anyhow!("Failed to parse run settings: {}", e))?;
            
            (session_id, profile_ids, user_question, run_settings)
        };

        // Update run status to running
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            conn_guard.execute(
                "UPDATE runs SET status = 'running' WHERE id = ?1",
                [&run_id],
            )
            .map_err(|e| anyhow::anyhow!("Failed to update run status: {}", e))?;
        }

        // Load profiles
        let profiles = self.load_profiles(&profile_ids)?;
        
        // Get concurrency limit
        let concurrency = run_settings
            .get("concurrency")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;
        
        let semaphore = Arc::new(Semaphore::new(concurrency));
        let mut handles = Vec::new();

        // Create tasks for each profile
        let cancelled_runs_clone = Arc::clone(&self.cancelled_runs);
        let cancelled_results_clone = Arc::clone(&self.cancelled_results);
        
        for profile in profiles {
            let run_id_clone = run_id.clone();
            let db_clone = self.db.clone();
            let semaphore_clone = Arc::clone(&semaphore);
            let user_question_clone = user_question.clone();
            let cancelled_runs_task = Arc::clone(&cancelled_runs_clone);
            let cancelled_results_task = Arc::clone(&cancelled_results_clone);
            
            let handle = tokio::spawn(async move {
                // Check if run is cancelled before starting
                {
                    let cancelled = cancelled_runs_task.read().await;
                    if cancelled.contains(&run_id_clone) {
                        return Err(anyhow::anyhow!("Run was cancelled"));
                    }
                }
                
                let _permit = semaphore_clone.acquire().await
                    .map_err(|e| anyhow::anyhow!("Failed to acquire semaphore: {}", e))?;
                
                // Check again after acquiring permit
                {
                    let cancelled = cancelled_runs_task.read().await;
                    if cancelled.contains(&run_id_clone) {
                        return Err(anyhow::anyhow!("Run was cancelled"));
                    }
                }
                
                let result_id = Self::execute_profile(
                    &db_clone,
                    &run_id_clone,
                    &profile,
                    &user_question_clone,
                    &cancelled_runs_task,
                    &cancelled_results_task,
                ).await?;
                
                Ok(result_id)
            });
            
            handles.push(handle);
        }

        // Wait for all tasks to complete
        let mut completed = 0;
        let mut failed = 0;
        
        for handle in handles {
            match handle.await {
                Ok(Ok(_)) => completed += 1,
                Ok(Err(e)) => {
                    eprintln!("Profile execution error: {}", e);
                    failed += 1;
                }
                Err(e) => {
                    eprintln!("Task join error: {}", e);
                    failed += 1;
                }
            }
        }

        // Update run status
        let final_status = if failed == 0 {
            "complete"
        } else if completed > 0 {
            "partial"
        } else {
            "failed"
        };
        
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let finished_at = chrono::Utc::now().to_rfc3339();
            conn_guard.execute(
                "UPDATE runs SET status = ?1, finished_at = ?2 WHERE id = ?3",
                rusqlite::params![final_status, finished_at, run_id],
            )
            .map_err(|e| anyhow::anyhow!("Failed to update run status: {}", e))?;
        }

        Ok(())
    }

    fn load_profiles(&self, profile_ids: &[String]) -> Result<Vec<ProfileData>> {
        let conn = self.db.get_connection();
        let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
        
        let placeholders = profile_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query = format!(
            "SELECT id, name, provider_account_id, model_name, persona_prompt, params_json FROM prompt_profiles WHERE id IN ({})",
            placeholders
        );
        
        let mut stmt = conn_guard.prepare(&query)
            .map_err(|e| anyhow::anyhow!("Failed to prepare query: {}", e))?;
        
        let rows = stmt.query_map(rusqlite::params_from_iter(profile_ids.iter()), |row| {
            Ok(ProfileData {
                id: row.get(0)?,
                name: row.get(1)?,
                provider_account_id: row.get(2)?,
                model_name: row.get(3)?,
                persona_prompt: row.get(4)?,
                params_json: serde_json::from_str::<Value>(&row.get::<_, String>(5)?)
                    .unwrap_or_else(|_| serde_json::json!({})),
            })
        })
        .map_err(|e| anyhow::anyhow!("Failed to query profiles: {}", e))?;
        
        let mut profiles = Vec::new();
        for row in rows {
            profiles.push(row.map_err(|e| anyhow::anyhow!("Row error: {}", e))?);
        }
        
        Ok(profiles)
    }

    async fn execute_profile(
        db: &Database,
        run_id: &str,
        profile: &ProfileData,
        user_question: &str,
        cancelled_runs: &Arc<RwLock<std::collections::HashSet<String>>>,
        cancelled_results: &Arc<RwLock<std::collections::HashSet<String>>>,
    ) -> Result<String> {
        // Check if result already exists for this profile in this run
        let existing_result: Option<String> = {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let result: Result<Option<String>, _> = conn_guard.query_row(
                "SELECT id FROM run_results WHERE run_id = ?1 AND profile_id = ?2 AND status != 'cancelled' LIMIT 1",
                rusqlite::params![run_id, profile.id],
                |row| Ok(Some(row.get(0)?)),
            );
            result.unwrap_or(None)
        };
        
        let result_id = existing_result.clone().unwrap_or_else(|| Uuid::new_v4().to_string());
        let started_at = chrono::Utc::now().to_rfc3339();
        
        // Create run result entry only if it doesn't exist
        if existing_result.is_none() {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            conn_guard.execute(
                "INSERT INTO run_results (id, run_id, profile_id, status, started_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![result_id, run_id, profile.id, "running", started_at],
            )
            .map_err(|e| anyhow::anyhow!("Failed to create run result: {}", e))?;
        } else {
            // Update existing result to running
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            conn_guard.execute(
                "UPDATE run_results SET status = 'running', started_at = ?1 WHERE id = ?2",
                rusqlite::params![started_at, result_id],
            )
            .map_err(|e| anyhow::anyhow!("Failed to update run result: {}", e))?;
        }

        // Load provider account
        // (Hybrid providers are resolved at call time; we don't need to load the provider here.)

        // Load local model context if available (from session)
        let local_model_context = {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            // Get session's local_model_id
            let session_local_model: Result<Option<String>, _> = conn_guard.query_row(
                "SELECT local_model_id FROM sessions WHERE id = (SELECT session_id FROM runs WHERE id = ?1)",
                [run_id],
                |row| Ok(row.get(0)?),
            );
            if let Ok(Some(model_id)) = session_local_model {
                // Get training data summary for context
                let training_summary: Result<String, _> = conn_guard.query_row(
                    "SELECT GROUP_CONCAT(input_text || ' -> ' || output_text, '\n') FROM training_data WHERE local_model_id = ?1 LIMIT 10",
                    [&model_id],
                    |row| Ok(row.get(0)?),
                );
                training_summary.ok()
            } else {
                None
            }
        };
        
        // Build enhanced persona with local model context
        let mut enhanced_persona = profile.persona_prompt.clone();
        if let Some(context) = local_model_context {
            enhanced_persona = format!(
                "{}\n\n[Trained Model Context]\nThis model has been trained on the following examples:\n{}\n\nUse this context to inform your responses while maintaining your persona.",
                profile.persona_prompt,
                context
            );
        }
        
        // Optionally retrieve simple RAG context (project-aware retrieval can be added later)
        let rag_context = rag::retrieve_simple_context_for_project(&db, None, 8)
            .unwrap_or_else(|_| rag::RagContext {
                combined_text: String::new(),
                chunks: Vec::new(),
            });

        // Global instructions for citations & groundedness
        let mut global_instructions = String::from(
            "You are an expert assistant. Your job is to provide answers that are strictly grounded in the provided context and your own reasoning.\n\
            - When you make a factual claim, cite the supporting source using the format [source:SOURCE_ID chunk:INDEX].\n\
            - If the context does not support a claim, explicitly say that the information is not available.\n\
            - Do not invent citations.",
        );

        if !rag_context.combined_text.is_empty() {
            global_instructions.push_str(
                "\n\nCONTEXT (from retrieved documents):\n====================================\n",
            );
            global_instructions.push_str(&rag_context.combined_text);
        }

        // Build prompt packet
        let packet = PromptPacket {
            global_instructions: Some(global_instructions),
            persona_instructions: enhanced_persona,
            user_message: user_question.to_string(),
            conversation_context: None,
            params_json: {
                // Lower temperature for higher factual accuracy if not explicitly set
                let mut params = profile.params_json.clone();
                let default_temp = 0.4;
                let temp = params
                    .get("temperature")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(default_temp);
                let clamped = temp.max(0.1).min(0.7);
                params["temperature"] = serde_json::json!(clamped);
                params
            },
            stream: false, // For now, non-streaming
        };

        // Check if cancelled before executing
        {
            let cancelled = cancelled_runs.read().await;
            if cancelled.contains(run_id) {
                // Mark as cancelled
                let conn = db.get_connection();
                let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
                let finished_at = chrono::Utc::now().to_rfc3339();
                conn_guard.execute(
                    "UPDATE run_results SET status = 'cancelled', finished_at = ?1 WHERE id = ?2",
                    rusqlite::params![finished_at, result_id],
                )
                .map_err(|e| anyhow::anyhow!("Failed to update result: {}", e))?;
                return Err(anyhow::anyhow!("Run was cancelled"));
            }
        }
        
        {
            let cancelled = cancelled_results.read().await;
            if cancelled.contains(&result_id) {
                // Mark as cancelled
                let conn = db.get_connection();
                let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
                let finished_at = chrono::Utc::now().to_rfc3339();
                conn_guard.execute(
                    "UPDATE run_results SET status = 'cancelled', finished_at = ?1 WHERE id = ?2",
                    rusqlite::params![finished_at, result_id],
                )
                .map_err(|e| anyhow::anyhow!("Failed to update result: {}", e))?;
                return Err(anyhow::anyhow!("Result was cancelled"));
            }
        }

        // Execute the request with cancellation support
        // We use tokio::select! to race between the API call and periodic cancellation checks
        let timeout_secs = 90u64;
        let api_future = async {
            let (resp, _used_provider, _used_model) =
                complete_resolving_hybrid(db, &profile.provider_account_id, &profile.model_name, &packet, timeout_secs, None)
                    .await
                    .map_err(|e| anyhow::anyhow!(e))?;
            Ok::<crate::types::NormalizedResponse, anyhow::Error>(resp)
        };
        
        // Create a cancellation check loop
        let cancelled_runs_clone = Arc::clone(cancelled_runs);
        let cancelled_results_clone = Arc::clone(cancelled_results);
        let run_id_string = run_id.to_string();
        let result_id_clone = result_id.clone();
        
        let cancellation_check = async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let runs_cancelled = cancelled_runs_clone.read().await;
                let results_cancelled = cancelled_results_clone.read().await;
                if runs_cancelled.contains(&run_id_string) || results_cancelled.contains(&result_id_clone) {
                    return true;
                }
            }
        };
        
        let result = tokio::select! {
            api_result = api_future => {
                // API call completed
                Some(api_result)
            }
            _ = cancellation_check => {
                // Cancellation requested
                None
            }
        };
        
        // Handle cancellation
        if result.is_none() {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let finished_at = chrono::Utc::now().to_rfc3339();
            conn_guard.execute(
                "UPDATE run_results SET status = 'cancelled', finished_at = ?1 WHERE id = ?2",
                rusqlite::params![finished_at, result_id],
            )
            .map_err(|e| anyhow::anyhow!("Failed to update result: {}", e))?;
            return Err(anyhow::anyhow!("Result was cancelled"));
        }
        
        let result = result.unwrap();

        // Save result
        let (status, raw_output, normalized_output, usage, error_code, error_message) = match result {
            Ok(response) => (
                "complete",
                Some(response.text),
                Some(serde_json::to_string(&response.raw_provider_payload_json.unwrap_or(serde_json::json!({})))?),
                response.usage_json.and_then(|u| serde_json::to_string(&u).ok()),
                None::<String>,
                None::<String>,
            ),
            Err(e) => {
                let error_msg = format!("{}", e);
                (
                    "failed",
                    None::<String>,
                    None::<String>,
                    None::<String>,
                    Some("provider_error".to_string()),
                    Some(error_msg),
                )
            }
        };

        let finished_at = chrono::Utc::now().to_rfc3339();
        
        {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            conn_guard.execute(
                "UPDATE run_results SET status = ?1, raw_output_text = ?2, normalized_output_json = ?3, usage_json = ?4, error_code = ?5, error_message_safe = ?6, finished_at = ?7 WHERE id = ?8",
                rusqlite::params![
                    status,
                    raw_output,
                    normalized_output,
                    usage,
                    error_code,
                    error_message,
                    finished_at,
                    result_id
                ],
            )
            .map_err(|e| anyhow::anyhow!("Failed to update run result: {}", e))?;
        }

        Ok(result_id)
    }

    /// Run a single agent (for rerun functionality)
    pub async fn run_single_agent(
        &self,
        db: &Database,
        _run_id: &str,
        profile_id: &str,
        user_question: &str,
        result_id: &str,
    ) -> Result<()> {
        // Load profile
        let profile = {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let profile_data: Result<(String, String, String, String, String, String), _> = conn_guard.query_row(
                "SELECT id, name, provider_account_id, model_name, persona_prompt, params_json FROM prompt_profiles WHERE id = ?1",
                [profile_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            );
            let (id, name, provider_account_id, model_name, persona_prompt, params_json_str) = profile_data
                .map_err(|e| anyhow::anyhow!("Failed to load profile: {}", e))?;
            ProfileData {
                id,
                name,
                provider_account_id,
                model_name,
                persona_prompt,
                params_json: serde_json::from_str(&params_json_str).unwrap_or(serde_json::json!({})),
            }
        };

        // Build prompt packet
        let packet = PromptPacket {
            global_instructions: None,
            persona_instructions: profile.persona_prompt.clone(),
            user_message: user_question.to_string(),
            conversation_context: None,
            params_json: profile.params_json.clone(),
            stream: false,
        };

        // Execute the request
        let timeout_secs = 90u64;
        let result = complete_resolving_hybrid(db, &profile.provider_account_id, &profile.model_name, &packet, timeout_secs, None)
            .await
            .map(|(resp, _used_provider, _used_model)| resp)
            .map_err(|e| anyhow::anyhow!(e));

        // Save result
        let (status, raw_output, normalized_output, usage, error_code, error_message) = match result {
            Ok(response) => (
                "complete",
                Some(response.text),
                Some(serde_json::to_string(&response.raw_provider_payload_json.unwrap_or(serde_json::json!({})))?),
                response.usage_json.and_then(|u| serde_json::to_string(&u).ok()),
                None::<String>,
                None::<String>,
            ),
            Err(e) => {
                let error_msg = e.to_string();
                (
                    "failed",
                    None::<String>,
                    None::<String>,
                    None::<String>,
                    Some("provider_error".to_string()),
                    Some(error_msg),
                )
            }
        };

        let finished_at = chrono::Utc::now().to_rfc3339();
        
        {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            conn_guard.execute(
                "UPDATE run_results SET status = ?1, raw_output_text = ?2, normalized_output_json = ?3, usage_json = ?4, error_code = ?5, error_message_safe = ?6, finished_at = ?7 WHERE id = ?8",
                rusqlite::params![
                    status,
                    raw_output,
                    normalized_output,
                    usage,
                    error_code,
                    error_message,
                    finished_at,
                    result_id
                ],
            )
            .map_err(|e| anyhow::anyhow!("Failed to update run result: {}", e))?;
        }

        Ok(())
    }

    /// Continue an agent with a follow-up message
    pub async fn continue_agent(
        &self,
        db: &Database,
        run_id: &str,
        profile_id: &str,
        original_question: &str,
        previous_output: Option<&str>,
        follow_up_message: &str,
        result_id: &str,
    ) -> Result<()> {
        // Load profile
        let profile = {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let profile_data: Result<(String, String, String, String, String, String), _> = conn_guard.query_row(
                "SELECT id, name, provider_account_id, model_name, persona_prompt, params_json FROM prompt_profiles WHERE id = ?1",
                [profile_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            );
            let (id, name, provider_account_id, model_name, persona_prompt, params_json_str) = profile_data
                .map_err(|e| anyhow::anyhow!("Failed to load profile: {}", e))?;
            ProfileData {
                id,
                name,
                provider_account_id,
                model_name,
                persona_prompt,
                params_json: serde_json::from_str(&params_json_str).unwrap_or(serde_json::json!({})),
            }
        };

        // Build conversation context with previous exchange
        let mut context = Vec::new();
        context.push(crate::types::Message {
            id: "original".to_string(),
            run_id: run_id.to_string(),
            author_type: "user".to_string(),
            profile_id: None,
            round_index: None,
            turn_index: None,
            text: original_question.to_string(),
            created_at: String::new(),
            provider_metadata_json: None,
        });
        if let Some(prev) = previous_output {
            context.push(crate::types::Message {
                id: "previous".to_string(),
                run_id: run_id.to_string(),
                author_type: "agent".to_string(),
                profile_id: Some(profile_id.to_string()),
                round_index: None,
                turn_index: None,
                text: prev.to_string(),
                created_at: String::new(),
                provider_metadata_json: None,
            });
        }

        // Build prompt packet with context
        let packet = PromptPacket {
            global_instructions: None,
            persona_instructions: profile.persona_prompt.clone(),
            user_message: follow_up_message.to_string(),
            conversation_context: Some(context),
            params_json: profile.params_json.clone(),
            stream: false,
        };

        // Execute the request
        let timeout_secs = 90u64;
        let result = complete_resolving_hybrid(db, &profile.provider_account_id, &profile.model_name, &packet, timeout_secs, None)
            .await
            .map(|(resp, _used_provider, _used_model)| resp)
            .map_err(|e| anyhow::anyhow!(e));

        // Save result
        let (status, raw_output, normalized_output, usage, error_code, error_message) = match result {
            Ok(response) => (
                "complete",
                Some(response.text),
                Some(serde_json::to_string(&response.raw_provider_payload_json.unwrap_or(serde_json::json!({})))?),
                response.usage_json.and_then(|u| serde_json::to_string(&u).ok()),
                None::<String>,
                None::<String>,
            ),
            Err(e) => {
                let error_msg = e.to_string();
                (
                    "failed",
                    None::<String>,
                    None::<String>,
                    None::<String>,
                    Some("provider_error".to_string()),
                    Some(error_msg),
                )
            }
        };

        let finished_at = chrono::Utc::now().to_rfc3339();
        
        {
            let conn = db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            conn_guard.execute(
                "UPDATE run_results SET status = ?1, raw_output_text = ?2, normalized_output_json = ?3, usage_json = ?4, error_code = ?5, error_message_safe = ?6, finished_at = ?7 WHERE id = ?8",
                rusqlite::params![
                    status,
                    raw_output,
                    normalized_output,
                    usage,
                    error_code,
                    error_message,
                    finished_at,
                    result_id
                ],
            )
            .map_err(|e| anyhow::anyhow!("Failed to update run result: {}", e))?;
        }

        Ok(())
    }
}

struct ProfileData {
    id: String,
    #[allow(dead_code)]
    name: String,
    provider_account_id: String,
    model_name: String,
    persona_prompt: String,
    params_json: Value,
}
