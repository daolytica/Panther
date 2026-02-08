// Debate Room orchestrator with state machine

use crate::db::Database;
use crate::provider_resolver::complete_resolving_hybrid;
use crate::types::{PromptPacket, Message};
use crate::token_usage::record_token_usage;
use crate::training_ingest;
use anyhow::Result;
use serde_json::{Value, json};
use uuid::Uuid;
use rand::seq::SliceRandom;
use rand::thread_rng;

#[derive(Debug, Clone, PartialEq)]
pub enum DebateState {
    Idle,
    Starting,
    RoundActive,
    TurnActive,
    Paused,
    Cancelled,
    Complete,
}

pub struct DebateOrchestrator {
    db: Database,
    state: DebateState,
}

impl DebateOrchestrator {
    pub fn new(db: Database) -> Self {
        DebateOrchestrator {
            db,
            state: DebateState::Idle,
        }
    }

    pub async fn run_debate(
        &mut self,
        run_id: String,
        rounds: i32,
        speaking_order: Vec<String>,
        max_words: Option<i32>,
        language: Option<String>,
        tone: Option<String>,
        web_search_results: Option<Vec<crate::web_search::NewsResult>>,
    ) -> Result<()> {
        self.state = DebateState::Starting;
        eprintln!("[Debate] Starting run_debate for run_id={}, speaking_order={:?}", run_id, speaking_order);

        if speaking_order.is_empty() {
            anyhow::bail!("speaking_order is empty - no profiles provided for debate");
        }

        // Load run and session data
        let (session_id, project_id, session_local_model_id, user_question): (String, String, Option<String>, String) = {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            
            let run_data: Result<(String,), _> = conn_guard.query_row(
                "SELECT session_id FROM runs WHERE id = ?1",
                [&run_id],
                |row| Ok((row.get(0)?,)),
            );
            
            let (session_id,) = run_data
                .map_err(|e| anyhow::anyhow!("Failed to load run: {}", e))?;

            let session_data: Result<(String, Option<String>, String), _> = conn_guard.query_row(
                "SELECT project_id, local_model_id, user_question FROM sessions WHERE id = ?1",
                [&session_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            );

            let (project_id, local_model_id, user_question) = session_data
                .map_err(|e| anyhow::anyhow!("Failed to load session: {}", e))?;

            (session_id, project_id, local_model_id, user_question)
        };

        // Create debate config
        let config_id = Uuid::new_v4().to_string();
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let speaking_order_json = serde_json::to_string(&speaking_order)
                .map_err(|e| anyhow::anyhow!("Failed to serialize speaking order: {}", e))?;
            
            conn_guard.execute(
                "INSERT INTO debate_configs (id, run_id, mode, rounds, speaking_order_json, context_policy, last_k, concurrency, max_words) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    config_id,
                    run_id,
                    "sequential",
                    rounds,
                    speaking_order_json,
                    "last_k_messages",
                    6,
                    1,
                    max_words
                ],
            )
            .map_err(|e| anyhow::anyhow!("Failed to create debate config: {}", e))?;
        }

        // Status is already set to running by start_debate command, so we don't need to update it here
        // But we'll verify it's running
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let status: Result<String, _> = conn_guard.query_row(
                "SELECT status FROM runs WHERE id = ?1",
                [&run_id],
                |row| row.get(0),
            );
            if let Ok(s) = status {
                if s != "running" {
                    eprintln!("Warning: Run status is {} but expected running", s);
                }
            }
        }

        // Insert user question as first message so UI shows something immediately
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let msg_id = Uuid::new_v4().to_string();
            let created_at = chrono::Utc::now().to_rfc3339();
            conn_guard.execute(
                "INSERT INTO messages (id, run_id, author_type, profile_id, round_index, turn_index, text, created_at, provider_metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    msg_id,
                    run_id,
                    "user",
                    None::<String>,
                    -1,
                    -1,
                    user_question,
                    created_at,
                    None::<String>,
                ],
            )
            .map_err(|e| anyhow::anyhow!("Failed to insert user question: {}", e))?;
            eprintln!("[Debate] Inserted user question for run_id={}", run_id);
        }

        // Load profiles
        let profiles = self.load_profiles(&speaking_order)?;
        eprintln!("[Debate] Loaded {} profiles for run_id={}", profiles.len(), run_id);

        if profiles.is_empty() {
            anyhow::bail!("No profiles found for IDs: {:?}. Check that selected profiles exist in prompt_profiles.", speaking_order);
        }

        // Execute debate rounds (0 = opening, 1.. = rebuttals; rounds=2 means 2 rounds total)
        self.state = DebateState::RoundActive;
        
        for round_index in 0..rounds {
            // Check if cancelled or paused
            {
                let status: String = {
                    let conn = self.db.get_connection();
                    let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
                    let status_result: Result<String, _> = conn_guard.query_row(
                        "SELECT status FROM runs WHERE id = ?1",
                        [&run_id],
                        |row| row.get(0),
                    );
                    drop(conn_guard); // Drop before await
                    status_result.map_err(|e| anyhow::anyhow!("Failed to get status: {}", e))?
                };
                
                if status == "cancelled" {
                    self.state = DebateState::Cancelled;
                    break;
                } else if status == "paused" {
                    self.state = DebateState::Paused;
                    // Wait for resume
                    while self.state == DebateState::Paused {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        let status_check: String = {
                            let conn = self.db.get_connection();
                            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
                            let status_result: Result<String, _> = conn_guard.query_row(
                                "SELECT status FROM runs WHERE id = ?1",
                                [&run_id],
                                |row| row.get(0),
                            );
                            drop(conn_guard); // Drop before await
                            status_result.map_err(|e| anyhow::anyhow!("Failed to get status: {}", e))?
                        };
                        if status_check == "running" {
                            self.state = DebateState::RoundActive;
                            break;
                        } else if status_check == "cancelled" {
                            self.state = DebateState::Cancelled;
                            break;
                        }
                    }
                    if self.state == DebateState::Cancelled {
                        break;
                    }
                }
            }
            
            if self.state == DebateState::Cancelled {
                break;
            }

            let mut messages: Vec<Message> = Vec::new();
            
            // Generate random order for this round (before any async operations)
            // Each round gets a fresh random shuffle of all agents
            let mut round_order: Vec<String> = speaking_order.clone();
            {
                let mut rng = thread_rng();
                round_order.shuffle(&mut rng);
            }
            
            // Round 0: Opening statements
            // Round 1+: Rebuttals
            // Use the randomly shuffled order for this round
            for (turn_index, profile_id) in round_order.iter().enumerate() {
                // Check DB status at start of each turn (user may have clicked Pause/Stop)
                {
                    let status: String = {
                        let conn = self.db.get_connection();
                        let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
                        let status_result: Result<String, _> = conn_guard.query_row(
                            "SELECT status FROM runs WHERE id = ?1",
                            [&run_id],
                            |row| row.get(0),
                        );
                        drop(conn_guard);
                        status_result.map_err(|e| anyhow::anyhow!("Failed to get status: {}", e))?
                    };
                    if status == "cancelled" {
                        self.state = DebateState::Cancelled;
                        break;
                    }
                    if status == "paused" {
                        self.state = DebateState::Paused;
                    }
                }

                if self.state == DebateState::Cancelled || self.state == DebateState::Paused {
                    // Wait for resume or break
                    while self.state == DebateState::Paused {
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        let status_check: String = {
                            let conn = self.db.get_connection();
                            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
                            let status_result: Result<String, _> = conn_guard.query_row(
                                "SELECT status FROM runs WHERE id = ?1",
                                [&run_id],
                                |row| row.get(0),
                            );
                            drop(conn_guard);
                            status_result.map_err(|e| anyhow::anyhow!("Failed to get status: {}", e))?
                        };
                        if status_check == "running" {
                            self.state = DebateState::RoundActive;
                            break;
                        } else if status_check == "cancelled" {
                            self.state = DebateState::Cancelled;
                            break;
                        }
                    }
                    if self.state == DebateState::Cancelled {
                        break;
                    }
                }

                self.state = DebateState::TurnActive;

                let profile = profiles.iter()
                    .find(|p| p.id == *profile_id)
                    .ok_or_else(|| anyhow::anyhow!("Profile not found: {}", profile_id))?;

                // Build context: load ALL previous messages from database
                let context_messages: Vec<Message> = {
                    let conn = self.db.get_connection();
                    let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
                    
                    let mut stmt = conn_guard
                        .prepare("SELECT id, author_type, profile_id, round_index, turn_index, text, created_at FROM messages WHERE run_id = ?1 AND (round_index < ?2 OR (round_index = ?2 AND turn_index < ?3)) ORDER BY round_index, turn_index, created_at")
                        .map_err(|e| anyhow::anyhow!("Failed to prepare query: {}", e))?;
                    
                    let rows = stmt
                        .query_map(rusqlite::params![run_id, round_index, turn_index], |row| {
                            Ok(Message {
                                id: row.get(0)?,
                                run_id: run_id.clone(),
                                author_type: row.get(1)?,
                                profile_id: row.get(2)?,
                                round_index: row.get(3)?,
                                turn_index: row.get(4)?,
                                text: row.get(5)?,
                                created_at: row.get(6)?,
                                provider_metadata_json: None,
                            })
                        })
                        .map_err(|e| anyhow::anyhow!("Failed to query messages: {}", e))?;
                    
                    let mut context = Vec::new();
                    for row in rows {
                        context.push(row.map_err(|e| anyhow::anyhow!("Row error: {}", e))?);
                    }
                    context
                };

                // Execute turn
                eprintln!("[Debate] Executing turn round={} turn={} profile={} run_id={}", round_index, turn_index, profile_id, run_id);
                let turn_result = self.execute_turn(
                    &run_id,
                    round_index,
                    turn_index as i32,
                    profile,
                    &user_question,
                    &context_messages,
                    max_words,
                    language.clone(),
                    tone.clone(),
                    if round_index == 0 { web_search_results.clone() } else { None },
                ).await;

                match turn_result {
                    Ok((response_text, usage_json)) => {
                        eprintln!("[Debate] Turn completed SUCCESS: run_id={} round={} turn={} profile={} text_len={}", run_id, round_index, turn_index, profile.id, response_text.len());
                        // Create message
                        let message_id = Uuid::new_v4().to_string();
                        let created_at = chrono::Utc::now().to_rfc3339();
                        
                        // Save message with usage data
                        {
                            let conn = self.db.get_connection();
                            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
                            
                            let usage_json_str = usage_json.as_ref()
                                .and_then(|u| serde_json::to_string(u).ok());
                            
                            match conn_guard.execute(
                                "INSERT INTO messages (id, run_id, author_type, profile_id, round_index, turn_index, text, created_at, provider_metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                                rusqlite::params![
                                    message_id,
                                    run_id,
                                    "agent",
                                    profile_id,
                                    round_index,
                                    turn_index,
                                    response_text,
                                    created_at,
                                    usage_json_str
                                ],
                            ) {
                                Ok(_) => eprintln!("[Debate] Message saved to DB: id={}", message_id),
                                Err(e) => eprintln!("[Debate] FAILED to save message: {}", e),
                            }
                        }

                        // Record token usage for this debate turn (if usage info is available)
                        if let Some(usage) = &usage_json {
                            eprintln!("[Debate] Recording token usage...");
                            // We don't currently track provider_id/model_name per turn here,
                            // so we record without provider_id and with a generic model name.
                            let _ = record_token_usage(
                                &self.db,
                                None,
                                "debate_model",
                                &Some(usage.clone()),
                                "debate",
                                None,
                                None,
                            );
                            eprintln!("[Debate] Token usage recorded");
                        }

                        // Auto-training ingest: debate turn → training_data (best-effort)
                        eprintln!("[Debate] Starting training ingest...");
                        let _ = training_ingest::ingest_debate_turn(
                            &self.db,
                            &project_id,
                            session_local_model_id.as_deref(),
                            &user_question,
                            &response_text,
                            &session_id,
                            &run_id,
                        );
                        eprintln!("[Debate] Training ingest complete");
                        
                        let message = Message {
                            id: message_id.clone(),
                            run_id: run_id.clone(),
                            author_type: "agent".to_string(),
                            profile_id: Some(profile_id.clone()),
                            round_index: Some(round_index),
                            turn_index: Some(turn_index as i32),
                            text: response_text.clone(),
                            created_at,
                            provider_metadata_json: usage_json,
                        };

                        messages.push(message);
                    }
                    Err(e) => {
                        eprintln!("[Debate] Turn execution error run_id={} round={} turn={} profile={}: {}", run_id, round_index, turn_index, profile.id, e);
                        // Continue to next turn even on error
                    }
                }
                
                eprintln!("[Debate] Finished processing turn, messages.len()={}", messages.len());
            }

            if self.state == DebateState::Cancelled {
                eprintln!("[Debate] Debate was cancelled, exiting round loop");
                break;
            }
            
            eprintln!("[Debate] Round {} complete, messages collected: {}", round_index, messages.len());
        }
        
        eprintln!("[Debate] All rounds complete, about to update run status");

        // Update run status
        self.state = DebateState::Complete;
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let finished_at = chrono::Utc::now().to_rfc3339();
            conn_guard.execute(
                "UPDATE runs SET status = 'complete', finished_at = ?1 WHERE id = ?2",
                rusqlite::params![finished_at, run_id],
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

    fn resolve_timeout_secs(&self, provider_account_id: &str) -> u64 {
        let conn = self.db.get_connection();
        let conn_guard = match conn.lock() {
            Ok(g) => g,
            Err(_) => return 120,
        };

        let provider_type: Result<String, _> = conn_guard.query_row(
            "SELECT provider_type FROM provider_accounts WHERE id = ?1",
            [provider_account_id],
            |row| row.get(0),
        );

        match provider_type.as_deref() {
            Ok("ollama") | Ok("local_http") => 240,
            Ok("hybrid") => 180,
            Ok(_) => 90,
            Err(_) => 120,
        }
    }

    async fn execute_turn(
        &self,
        run_id: &str,
        round_index: i32,
        turn_index: i32,
        profile: &ProfileData,
        user_question: &str,
        context_messages: &[Message],
        max_words: Option<i32>,
        language: Option<String>,
        tone: Option<String>,
        web_search_results: Option<Vec<crate::web_search::NewsResult>>,
    ) -> Result<(String, Option<serde_json::Value>)> {
        eprintln!("[Debate] execute_turn start: run_id={} round={} turn={} profile={} provider={} model={}", run_id, round_index, turn_index, profile.id, profile.provider_account_id, profile.model_name);
        let turn_id = Uuid::new_v4().to_string();
        let started_at = chrono::Utc::now().to_rfc3339();
        
        // Create debate turn entry
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            let input_snapshot = json!({
                "user_question": user_question,
                "context_messages": context_messages.len(),
            });
            let input_snapshot_json = serde_json::to_string(&input_snapshot)?;
            
            conn_guard.execute(
                "INSERT INTO debate_turns (id, run_id, round_index, turn_index, speaker_profile_id, input_snapshot_json, status, started_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    turn_id,
                    run_id,
                    round_index,
                    turn_index,
                    profile.id,
                    input_snapshot_json,
                    "running",
                    started_at
                ],
            )
            .map_err(|e| anyhow::anyhow!("Failed to create debate turn: {}", e))?;
        }

        // Provider selection (supports provider_type = "hybrid") happens at call time.

        // Build prompt based on round - use soft limit to avoid LLM failures; skip if too restrictive
        let word_limit_instruction = if let Some(max) = max_words {
            if max >= 50 {
                format!("\n\nKeep your response to approximately {} words or fewer. Be concise but complete.", max)
            } else {
                String::new() // Skip very strict limits that can cause LLM issues
            }
        } else {
            String::new()
        };
        
        // Language instruction
        let language_instruction = if let Some(lang) = &language {
            format!("\n\nIMPORTANT: Respond in {} language. All your messages must be in this language.", lang)
        } else {
            String::new()
        };
        
        // Tone instruction
        let tone_instruction = if let Some(t) = &tone {
            format!("\n\nIMPORTANT: Maintain a {} tone throughout your response. The overall debate should be {}.", t, t)
        } else {
            String::new()
        };
        
        // Add web search results if provided (only for first round to avoid repetition)
        let mut web_context = String::new();
        if let Some(news_results) = &web_search_results {
            if !news_results.is_empty() {
                web_context = "\n\nRECENT NEWS AND INFORMATION:\n".to_string();
                for (i, result) in news_results.iter().enumerate() {
                    web_context.push_str(&format!(
                        "{}. {}\n   Source: {}\n   Summary: {}\n\n",
                        i + 1,
                        result.title,
                        result.url,
                        if result.snippet.len() > 200 {
                            &result.snippet[..200]
                        } else {
                            &result.snippet
                        }
                    ));
                }
                web_context.push_str("Use this recent information to provide up-to-date, relevant responses. Reference these sources naturally in your conversation.\n");
            }
        }
        
        let persona_instruction = if round_index == 0 {
            format!("{}\n\nAnswer the following question with your perspective. Be conversational, natural, and human-like. Avoid overly formal or robotic language. Use contractions, natural pauses, and speak as if you're having a real discussion. Engage naturally with the topic.{}{}{}{}", 
                profile.persona_prompt, 
                word_limit_instruction,
                language_instruction,
                tone_instruction,
                web_context)
        } else {
            format!("{}\n\nConsider the previous discussion and provide your response. You may agree, disagree, or add new perspectives. Be conversational, natural, and human-like. Avoid overly formal or robotic language. Use contractions, natural pauses, and speak as if you're having a real discussion. Engage naturally with what others have said.{}{}{}", 
                profile.persona_prompt, 
                word_limit_instruction,
                language_instruction,
                tone_instruction)
        };

        // Build conversation context
        let conversation_context: Option<Vec<Message>> = if context_messages.is_empty() {
            None
        } else {
            Some(context_messages.to_vec())
        };

        // Adjust temperature based on debate style: keep slightly creative but bounded
        let mut params = profile.params_json.clone();
        let default_temp = 0.7;
        let temp = params
            .get("temperature")
            .and_then(|v| v.as_f64())
            .unwrap_or(default_temp);
        let clamped = temp.max(0.4).min(1.0);
        params["temperature"] = json!(clamped);
        
        // Global instructions for citations & groundedness in debate
        let mut global_instructions = String::from(
            "You are participating in a multi-agent debate. When you present factual claims:\n\
            - Ground them in the provided context or widely-accepted knowledge.\n\
            - Where possible, include inline citations using [source:SOURCE_ID chunk:INDEX].\n\
            - If you are uncertain, say so explicitly instead of guessing.",
        );
        if !web_context.is_empty() {
            global_instructions.push_str(
                "\n\nYou also have access to RECENT NEWS AND INFORMATION above. Prefer these sources when relevant.",
            );
        }

        // Build prompt packet
        let packet = PromptPacket {
            global_instructions: Some(global_instructions),
            persona_instructions: persona_instruction,
            user_message: user_question.to_string(),
            conversation_context,
            params_json: params,
            stream: false,
        };

        // Execute the request (supports provider_type = "hybrid").
        // Local models (e.g., Ollama/gemma2:9b) can be slower, especially first turn.
        let timeout_secs = self.resolve_timeout_secs(&profile.provider_account_id);
        eprintln!("[Debate] Calling LLM: provider={} model={} (timeout={}s)", profile.provider_account_id, profile.model_name, timeout_secs);
        let result = complete_resolving_hybrid(
            &self.db,
            &profile.provider_account_id,
            &profile.model_name,
            &packet,
            timeout_secs,
            None,
        )
        .await
        .map(|(resp, _used_provider, _used_model)| resp)
        .map_err(|e| anyhow::anyhow!(e));

        // Save result and track usage
        let (status, response_text, error_code, error_message, usage_json) = match result {
            Ok(response) => {
                let usage = response.usage_json.clone();
                (
                    "complete",
                    response.text,
                    None::<String>,
                    None::<String>,
                    usage,
                )
            },
            Err(e) => {
                let error_msg = format!("{}", e);
                (
                    "failed",
                    String::new(),
                    Some("provider_error".to_string()),
                    Some(error_msg),
                    None,
                )
            }
        };

        let finished_at = chrono::Utc::now().to_rfc3339();
        
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;
            
            conn_guard.execute(
                "UPDATE debate_turns SET status = ?1, finished_at = ?2, error_code = ?3, error_message = ?4 WHERE id = ?5",
                rusqlite::params![
                    status,
                    finished_at,
                    error_code,
                    error_message,
                    turn_id
                ],
            )
            .map_err(|e| anyhow::anyhow!("Failed to update debate turn: {}", e))?;
        }

        if status == "failed" {
            anyhow::bail!("Turn execution failed: {}", error_message.unwrap_or_default());
        }

        // Return response text and usage
        Ok((response_text, usage_json))
    }

    #[allow(dead_code)]
    pub fn pause(&mut self) {
        if self.state == DebateState::TurnActive || self.state == DebateState::RoundActive {
            self.state = DebateState::Paused;
        }
    }

    #[allow(dead_code)]
    pub fn resume(&mut self) {
        if self.state == DebateState::Paused {
            self.state = DebateState::RoundActive;
        }
    }

    #[allow(dead_code)]
    pub fn cancel(&mut self) {
        self.state = DebateState::Cancelled;
    }
    
    pub async fn handle_error(&self, run_id: &str, error: &str) {
        let conn = self.db.get_connection();
        if let Ok(conn_guard) = conn.lock() {
            let finished_at = chrono::Utc::now().to_rfc3339();
            let _ = conn_guard.execute(
                "UPDATE runs SET status = 'failed', finished_at = ?1 WHERE id = ?2",
                rusqlite::params![finished_at, run_id],
            );
            // Store error for UI retrieval (runs.error_message_safe if column exists)
            let _ = conn_guard.execute(
                "UPDATE runs SET error_message_safe = ?1 WHERE id = ?2",
                rusqlite::params![error, run_id],
            );
        }
        eprintln!("[Debate] Error for run {}: {}", run_id, error);
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
