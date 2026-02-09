// Coder commands - unrestricted AI agent mode

use crate::db::Database;
use crate::providers::get_adapter;
use crate::provider_resolver::{complete_resolving_hybrid, resolve_provider_chain};
use crate::types::{PromptPacket, ProviderAccount, Message, NormalizedResponse};
use crate::privacy::{PiiRedactor, ContextCompactor};
use crate::commands_privacy::PrivacySettings;
use crate::commands_training::{ChatWithTrainingDataRequest, chat_with_training_data};
use crate::token_usage::record_token_usage;
use crate::tools::{ToolRequest, ToolResult, execute_tool};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{State, AppHandle, Emitter};
use uuid::Uuid;
use chrono::Utc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::time;

// Helper to load privacy settings from database
fn load_privacy_settings(db: &Database) -> Result<PrivacySettings, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let result: Option<String> = conn_guard
        .query_row(
            "SELECT settings_json FROM privacy_settings WHERE id = 'default'",
            [],
            |row| row.get(0),
        )
        .ok();
    
    match result {
        Some(json_str) => serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse privacy settings: {}", e)),
        None => Ok(PrivacySettings::default()),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoderChatRequest {
    pub provider_id: String,
    pub model_name: String,
    pub user_message: String,
    pub conversation_context: Option<Vec<Message>>,
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub apply_privacy: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoderAutoChatRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub provider_id: String,
    pub model_name: String,
    pub user_message: String,
    pub conversation_context: Option<Vec<Message>>,
    pub system_prompt: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoderAutoChatResponse {
    pub answer: String,
    pub from_training: bool,
    pub used_remote: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoderChat {
    pub id: String,
    pub title: String,
    pub messages: Vec<CoderMessage>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoderMessage {
    pub id: String,
    pub role: String,
    pub content: String,
    pub timestamp: String,
    pub model: Option<String>,
    pub provider: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentProposedChange {
    pub file_path: String,
    pub description: Option<String>,
    pub new_content: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoderAgentTaskRequest {
    pub provider_id: String,
    pub model_name: String,
    pub task_description: String,
    pub target_paths: Option<Vec<String>>,
    pub allow_file_writes: bool,
    pub allow_commands: bool,
    pub max_steps: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoderAgentTaskResponse {
    pub run_id: String,
    pub status: String,
    pub summary: String,
    pub proposed_changes: Vec<AgentProposedChange>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoderWorkflow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub workflow_json: Value,
    pub created_at: String,
    pub updated_at: String,
}

/// Declarative workflow schema used inside `CoderWorkflow.workflow_json`.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub description: String,
    #[serde(default)]
    pub tool: Option<ToolRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub steps: Vec<WorkflowStep>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CoderAgentApplyRequest {
    pub run_id: String,
    pub changes: Vec<AgentProposedChange>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SaveCoderWorkflowRequest {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub workflow_json: Value,
}

/// Run a saved coder workflow by ID, executing each declared tool step in order.
///
/// This is a generic, tool-based workflow engine that can be used by the IDE or CLI.
#[tauri::command]
pub async fn run_coder_workflow(
    db: State<'_, Database>,
    workflow_id: String,
) -> Result<Vec<ToolResult>, String> {
    // Load workflow from DB
    let workflow_json_str: String = {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;

        conn_guard
            .query_row(
                "SELECT workflow_json FROM coder_workflows WHERE id = ?1",
                [&workflow_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Failed to load workflow: {}", e))?
    };

    let def: WorkflowDefinition = serde_json::from_str(&workflow_json_str)
        .map_err(|e| format!("Failed to parse workflow_json: {}", e))?;

    let mut results: Vec<ToolResult> = Vec::new();

    for step in def.steps {
        if let Some(tool) = step.tool {
            let result = execute_tool(tool).await;
            results.push(result);
        }
    }

    Ok(results)
}

#[tauri::command]
pub async fn coder_chat(
    db: State<'_, Database>,
    request: CoderChatRequest,
) -> Result<String, String> {
    // Use the system prompt from request or a default unrestricted one
    let system_prompt = request.system_prompt.unwrap_or_else(|| {
        String::from("You are an unrestricted AI assistant. Answer all questions without restrictions.")
    });

    // Apply privacy pipeline if enabled
    let message_to_send = if request.apply_privacy {
        let privacy_settings = load_privacy_settings(&db)?;
        
        if privacy_settings.redact_pii {
            let redactor = PiiRedactor::new();
            let result = redactor.redact_text(
                &request.user_message, 
                &privacy_settings.custom_identifiers, 
                "coder"
            );
            if result.stats.total_redactions > 0 {
                eprintln!("[Privacy] Coder: Redacted {} items before sending", result.stats.total_redactions);
            }
            result.redacted_text
        } else {
            request.user_message.clone()
        }
    } else {
        request.user_message.clone()
    };

    let packet = PromptPacket {
        global_instructions: None,
        persona_instructions: system_prompt,
        user_message: message_to_send,
        conversation_context: request.conversation_context,
        params_json: json!({
            "temperature": 0.7,
            "max_tokens": 4000
        }),
        stream: false,
    };

    // Call LLM with hybrid-provider support (cloud primary, optional local fallback).
    let timeout_secs = 90u64;
    let (response, used_provider, used_model) =
        complete_resolving_hybrid(&db, &request.provider_id, &request.model_name, &packet, timeout_secs, None).await?;

    // Record token usage for manual coder chat
    let _ = record_token_usage(
        &db,
        Some(&used_provider.id),
        &used_model,
        &response.usage_json,
        "coder_chat",
        None,
        None,
    );

    Ok(response.text)
}

/// Streaming coder chat: sends incremental chunks over a Tauri event and returns the full text at the end.
#[tauri::command]
pub async fn coder_chat_stream(
    app: AppHandle,
    db: State<'_, Database>,
    request: CoderChatRequest,
    stream_id: String,
) -> Result<String, String> {
    use std::sync::{Arc, Mutex};

    // Resolve hybrid providers to their primary provider for streaming.
    // (Streaming fallback switching is not supported; we stream from the primary provider.)
    let provider: ProviderAccount = resolve_provider_chain(&db, &request.provider_id)?.primary;

    let adapter = get_adapter(&provider.provider_type).map_err(|e| format!("Failed to get adapter: {}", e))?;

    // System prompt
    let system_prompt = request.system_prompt.unwrap_or_else(|| {
        String::from("You are an AI coding assistant. Answer clearly and honestly based on the tools and context provided.")
    });

    // Apply privacy pipeline if enabled
    let message_to_send = if request.apply_privacy {
        let privacy_settings = load_privacy_settings(&db)?;
        
        if privacy_settings.redact_pii {
            let redactor = PiiRedactor::new();
            let result = redactor.redact_text(
                &request.user_message, 
                &privacy_settings.custom_identifiers, 
                "coder",
            );
            if result.stats.total_redactions > 0 {
                eprintln!("[Privacy] Coder Stream: Redacted {} items before sending", result.stats.total_redactions);
            }
            result.redacted_text
        } else {
            request.user_message.clone()
        }
    } else {
        request.user_message.clone()
    };

    let packet = PromptPacket {
        global_instructions: None,
        persona_instructions: system_prompt,
        user_message: message_to_send,
        conversation_context: request.conversation_context,
        params_json: json!({
            "temperature": 0.7,
            "max_tokens": 4000
        }),
        stream: true,
    };

    #[derive(Serialize, Clone)]
    struct StreamChunkPayload {
        stream_id: String,
        chunk: String,
        done: bool,
        error: Option<String>,
    }

    let full_text = Arc::new(Mutex::new(String::new()));
    let full_text_clone = full_text.clone();
    let stream_id_clone = stream_id.clone();
    // Clone AppHandle for use inside the streaming closure so we can still use `app` later.
    let app_for_stream = app.clone();
    // Track if streaming has completed
    let completed = Arc::new(AtomicBool::new(false));
    let completed_clone = completed.clone();

    let on_chunk = Box::new(move |chunk: String| {
        // Mark that we've received data
        completed_clone.store(false, Ordering::SeqCst);

        // Append to accumulated text
        if let Ok(mut buf) = full_text_clone.lock() {
            buf.push_str(&chunk);
        }
        // Fire event to frontend
        let payload = StreamChunkPayload {
            stream_id: stream_id_clone.clone(),
            chunk: chunk.clone(),
            done: false,
            error: None,
        };
        eprintln!("📡 Emitting chunk for stream {}: {} chars", stream_id_clone, chunk.len());
        let emit_result = app_for_stream.emit("panther://coder_stream", payload);
        if let Err(e) = emit_result {
            eprintln!("❌ Failed to emit chunk: {}", e);
        }
    });

    eprintln!("🚀 Starting streaming for request {}", stream_id);

    // Create a custom streaming wrapper that includes completion detection
    let app_completion = app.clone();
    let stream_id_completion = stream_id.clone();
    let completed_for_timeout = completed.clone();
    // Clone values that will be moved into the async closure
    let provider_clone = provider.clone();
    let model_name_clone = request.model_name.clone();

    let stream_with_completion = async move {
        // Start the actual streaming
        let result = adapter.stream(&packet, &provider_clone, &model_name_clone, on_chunk).await;

        // Mark as completed
        completed.store(true, Ordering::SeqCst);
        result
    };

    // Add a timeout for completion detection
    let completion_timeout = async {
        time::sleep(time::Duration::from_secs(10)).await;
        if !completed_for_timeout.load(Ordering::SeqCst) {
            eprintln!("⚠️ No completion marker received for {} after 10s, sending manual completion", stream_id_completion);
            let payload = StreamChunkPayload {
                stream_id: stream_id_completion.clone(),
                chunk: String::new(),
                done: true,
                error: None,
            };
            let _ = app_completion.emit("panther://coder_stream", payload);
            completed_for_timeout.store(true, Ordering::SeqCst);
        }
    };

    // Race the streaming with overall timeout
    let stream_future = tokio::time::timeout(std::time::Duration::from_secs(120), stream_with_completion);
    let response = tokio::select! {
        result = stream_future => {
            match result {
                Ok(Ok(resp)) => resp,
                Ok(Err(e)) => {
                    let error_msg = format!("LLM error: {}", e);
                    eprintln!("❌ Stream error for {}: {}", stream_id, error_msg);
                    let payload = StreamChunkPayload {
                        stream_id: stream_id.clone(),
                        chunk: String::new(),
                        done: true,
                        error: Some(error_msg.clone()),
                    };
                    let _ = app.emit("panther://coder_stream", payload);
                    return Err(error_msg);
                }
                Err(_) => {
                    let error_msg = "Streaming request timed out after 120 seconds".to_string();
                    eprintln!("⏰ Stream timeout for {}", stream_id);
                    let payload = StreamChunkPayload {
                        stream_id: stream_id.clone(),
                        chunk: String::new(),
                        done: true,
                        error: Some(error_msg.clone()),
                    };
                    let _ = app.emit("panther://coder_stream", payload);
                    return Err(error_msg);
                }
            }
        }
        _ = completion_timeout => {
            eprintln!("🎯 Completion timeout triggered for {}", stream_id);
            // Return a successful response with whatever text we accumulated
            NormalizedResponse {
                text: if let Ok(buf) = full_text.lock() {
                    buf.clone()
                } else {
                    String::new()
                },
                finish_reason: Some("timeout".to_string()),
                request_id: None,
                usage_json: None,
                raw_provider_payload_json: None,
            }
        }
    };

    // Ensure we have some response text, even if streaming failed
    let final_text = if response.text.is_empty() {
        if let Ok(buf) = full_text.lock() {
            buf.clone()
        } else {
            String::new()
        }
    } else {
        response.text
    };

    eprintln!("✅ Streaming completed for request {}, total chars: {}", stream_id, final_text.len());

    // Record token usage
    let _ = record_token_usage(
        &db,
        Some(&provider.id),
        &request.model_name,
        &response.usage_json,
        "coder_chat_stream",
        None,
        None,
    );

    // Return full text (use accumulated text if response.text is empty)
    Ok(final_text)
}

/// Auto mode chat: prefer local training data, fall back to remote LLM.
#[tauri::command]
pub async fn coder_auto_chat(
    db: State<'_, Database>,
    request: CoderAutoChatRequest,
) -> Result<CoderAutoChatResponse, String> {
    // First, attempt to answer using local training data for this project/model.
    let training_req = ChatWithTrainingDataRequest {
        project_id: request.project_id.clone(),
        local_model_id: request.local_model_id.clone(),
        query: request.user_message.clone(),
        profile_id: None,
        max_examples: Some(5),
        use_local: Some(true),
        local_model_name: None,
    };

    if let Ok(local_resp) = chat_with_training_data(db.clone(), training_req).await {
        return Ok(CoderAutoChatResponse {
            answer: local_resp.response,
            from_training: true,
            used_remote: false,
        });
    }

    // If local training data is unavailable or fails, fall back to remote coder_chat logic.
    // Apply privacy settings and context compaction.
    let privacy_settings = load_privacy_settings(&db)?;
    let compactor = ContextCompactor::new();

    let custom_ids = &privacy_settings.custom_identifiers;
    let code_snippets: Vec<String> = Vec::new();
    let errors: Vec<String> = Vec::new();
    let notes: Option<String> = None;

    let compacted_question = compactor.compact(
        &request.user_message,
        &code_snippets,
        &errors,
        notes.as_deref(),
        custom_ids,
    );

    let message_to_send = if privacy_settings.redact_pii {
        let redactor = PiiRedactor::new();
        let result =
            redactor.redact_text(&compacted_question, &privacy_settings.custom_identifiers, "coder_auto");
        if result.stats.total_redactions > 0 {
            eprintln!("[Privacy] Coder Auto: Redacted {} items before sending", result.stats.total_redactions);
        }
        result.redacted_text
    } else {
        compacted_question
    };

    let system_prompt = request.system_prompt.unwrap_or_else(|| {
        String::from(
            "You are Panther Coder in Auto Mode. Use the provided compact context and answer precisely.",
        )
    });

    let packet = PromptPacket {
        global_instructions: None,
        persona_instructions: system_prompt,
        user_message: message_to_send,
        conversation_context: request.conversation_context,
        params_json: json!({
            "temperature": 0.6,
            "max_tokens": 2000
        }),
        stream: false,
    };

    let timeout_secs = 90u64;
    let (response, used_provider, used_model) =
        complete_resolving_hybrid(&db, &request.provider_id, &request.model_name, &packet, timeout_secs, None).await?;

    // Record token usage for remote fallback.
    let _ = record_token_usage(
        &db,
        Some(&used_provider.id),
        &used_model,
        &response.usage_json,
        "coder_auto_remote",
        None,
        None,
    );

    Ok(CoderAutoChatResponse {
        answer: response.text,
        from_training: false,
        used_remote: true,
    })
}

/// Lightweight Agent Mode entrypoint.
///
/// This does not yet perform full tool-loop execution; instead it asks the model
/// to return a structured JSON plan and set of proposed file edits, which are
/// stored as an agent_run + agent_steps and returned to the frontend for review.
#[tauri::command]
pub async fn coder_agent_task(
    db: State<'_, Database>,
    request: CoderAgentTaskRequest,
) -> Result<CoderAgentTaskResponse, String> {
    // Create agent_run row in DB
    let run_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;

        conn_guard
            .execute(
                "INSERT INTO agent_runs (
                    id, task_description, target_paths_json,
                    allow_file_writes, allow_commands, status,
                    provider_id, model_name, created_at, started_at, updated_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    run_id,
                    request.task_description,
                    request
                        .target_paths
                        .as_ref()
                        .map(|v| serde_json::to_string(v).unwrap_or_else(|_| "[]".to_string())),
                    if request.allow_file_writes { 1 } else { 0 },
                    if request.allow_commands { 1 } else { 0 },
                    "running",
                    request.provider_id,
                    request.model_name,
                    now,
                    now,
                    now,
                ],
            )
            .map_err(|e| format!("Failed to insert agent_run: {}", e))?;
    }

    // Build system instruction describing the JSON schema we expect
    let mut system_instructions = String::from(
        "You are a powerful coding agent with FULL SYSTEM ACCESS. You can create folders, files, and run commands ANYWHERE on the system.\n\
        You have admin privileges and can write to any location the user has access to.\n\
        You MUST respond with ONLY valid JSON - no markdown, no code blocks, no explanations, no control characters.\n\
        Start your response immediately with { and end with }.\n\n\
        Required JSON schema (return exactly this structure):\n\
        {\n\
          \"summary\": \"brief description of what you will do\",\n\
          \"steps\": [ { \"description\": \"step description\" } ],\n\
          \"proposed_changes\": [\n\
            {\n\
              \"file_path\": \"path (use forward slashes / or double backslashes \\\\\\\\ for Windows)\",\n\
              \"description\": \"what this change does\",\n\
              \"new_content\": \"full file content (escape quotes and backslashes properly)\"\n\
            }\n\
          ]\n\
        }\n\n\
        CRITICAL JSON RULES:\n\
        - Use forward slashes (/) in file paths: \"Panther Code/cpu_usage.py\"\n\
        - The new_content field must contain the FULL file content as a JSON string\n\
        - You can write newlines directly in new_content - they will be automatically escaped\n\
        - Escape quotes inside strings: use \\\" for quotes\n\
        - Ensure all arrays and objects are properly closed with matching brackets/braces\n\
        - Your response must be ONLY the JSON object, nothing else. No markdown fences, no ```json, no text before or after.\n\
        - Example new_content format: \"new_content\": \"line1\\nline2\\nline3\" or just write it with actual newlines\n",
    );

    if let Some(paths) = &request.target_paths {
        system_instructions.push_str("\nTarget paths to focus on:\n");
        for p in paths {
            system_instructions.push_str(&format!("- {}\n", p));
        }
    }

    let max_steps = request.max_steps.unwrap_or(8);
    system_instructions.push_str(&format!(
        "\nYou may assume you can take up to {} high-level steps.\n",
        max_steps
    ));

    if !request.allow_file_writes {
        system_instructions
            .push_str("\nDo NOT actually apply file writes; only propose changes.\n");
    }

    if !request.allow_commands {
        system_instructions
            .push_str("\nDo NOT run shell commands; you may only describe what you would run.\n");
    }

    let packet = PromptPacket {
        global_instructions: Some(system_instructions),
        persona_instructions: String::from(
            "You are a careful, senior-level coding agent. Propose small, safe steps.",
        ),
        user_message: request.task_description.clone(),
        conversation_context: None,
        params_json: json!({
            "temperature": 0.4,
            "max_tokens": 4096  // Increased for complex agent tasks with multiple file changes
        }),
        stream: false,
    };

    let timeout_secs = 120u64;
    let (llm_response, _used_provider, _used_model) =
        complete_resolving_hybrid(&db, &request.provider_id, &request.model_name, &packet, timeout_secs, None)
            .await
            .map_err(|e| {
                let msg = format!("LLM error in agent mode: {}", e);
                // Best-effort to mark run as failed
                let conn = db.get_connection();
                if let Ok(conn_guard) = conn.lock() {
                    let _ = conn_guard.execute(
                        "UPDATE agent_runs SET status = 'failed', error_text = ?1, finished_at = ?2, updated_at = ?2 WHERE id = ?3",
                        rusqlite::params![msg, Utc::now().to_rfc3339(), run_id],
                    );
                }
                msg
            })?;

    // Clean and parse JSON - handle control characters and markdown code blocks
    let cleaned_text = {
        let mut text = llm_response.text.trim().to_string();
        
        // Remove control characters (except newlines and tabs in string values)
        text = text.chars()
            .filter(|c| !matches!(c, '\u{0000}'..='\u{001F}' if *c != '\n' && *c != '\r' && *c != '\t'))
            .collect();
        
        // Extract JSON from markdown code blocks if present
        if let Some(start) = text.find("```json") {
            if let Some(end) = text[start + 7..].find("```") {
                text = text[start + 7..start + 7 + end].trim().to_string();
            }
        } else if let Some(start) = text.find("```") {
            if let Some(end) = text[start + 3..].find("```") {
                text = text[start + 3..start + 3 + end].trim().to_string();
            }
        }
        
        // Fix common JSON issues: unescaped backslashes and newlines in string values
        // This pass fixes:
        // 1. Unescaped backslashes in Windows paths -> convert to forward slashes
        // 2. Unescaped newlines, carriage returns, tabs in strings -> escape them properly
        let mut fixed_text = String::new();
        let mut in_string = false;
        let mut escape_next = false;
        let mut chars = text.chars().peekable();
        
        while let Some(ch) = chars.next() {
            if escape_next {
                fixed_text.push(ch);
                escape_next = false;
                continue;
            }
            
            match ch {
                '\\' if in_string => {
                    // Check if this is a valid JSON escape sequence
                    if let Some(&next) = chars.peek() {
                        match next {
                            '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' | 'u' => {
                                // Valid escape sequence, keep it
                                fixed_text.push('\\');
                                fixed_text.push(chars.next().unwrap());
                            }
                            _ => {
                                // Unescaped backslash (likely in Windows path), convert to forward slash
                                fixed_text.push('/');
                            }
                        }
                    } else {
                        fixed_text.push('/');
                    }
                }
                '\n' if in_string => {
                    // Unescaped newline in string - escape it
                    fixed_text.push_str("\\n");
                }
                '\r' if in_string => {
                    // Unescaped carriage return in string - escape it
                    fixed_text.push_str("\\r");
                }
                '\t' if in_string => {
                    // Unescaped tab in string - escape it
                    fixed_text.push_str("\\t");
                }
                '"' => {
                    in_string = !in_string;
                    fixed_text.push(ch);
                }
                '\\' if !in_string => {
                    escape_next = true;
                    fixed_text.push(ch);
                }
                _ => {
                    fixed_text.push(ch);
                }
            }
        }
        
        text = fixed_text;
        
        // Find the first { and try to extract complete JSON object
        if let Some(start_idx) = text.find('{') {
            // Try to find the matching closing brace
            let mut brace_count = 0;
            let mut in_string = false;
            let mut escape_next = false;
            let mut end_idx = None;
            
            for (i, ch) in text[start_idx..].char_indices() {
                if escape_next {
                    escape_next = false;
                    continue;
                }
                
                match ch {
                    '\\' if in_string => escape_next = true,
                    '"' => in_string = !in_string,
                    '{' if !in_string => brace_count += 1,
                    '}' if !in_string => {
                        brace_count -= 1;
                        if brace_count == 0 {
                            end_idx = Some(start_idx + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            
            if let Some(end) = end_idx {
                text = text[start_idx..=end].to_string();
            } else {
                // JSON is incomplete - try to auto-complete it
                eprintln!("⚠️ JSON appears incomplete, attempting to auto-complete...");
                let mut auto_complete = text[start_idx..].to_string();
                
                // Count unmatched brackets/braces
                let open_braces = auto_complete.matches('{').count();
                let close_braces = auto_complete.matches('}').count();
                let open_brackets = auto_complete.matches('[').count();
                let close_brackets = auto_complete.matches(']').count();
                
                // Close any unclosed arrays first
                for _ in 0..(open_brackets.saturating_sub(close_brackets)) {
                    auto_complete.push(']');
                }
                
                // Close any unclosed objects
                for _ in 0..(open_braces.saturating_sub(close_braces)) {
                    auto_complete.push('}');
                }
                
                text = auto_complete;
                eprintln!("🔧 Auto-completed JSON (added {} braces, {} brackets)", 
                         open_braces.saturating_sub(close_braces),
                         open_brackets.saturating_sub(close_brackets));
            }
        }
        
        text
    };
    
    eprintln!("🔍 Attempting to parse cleaned JSON (length: {}): {}", cleaned_text.len(), 
              if cleaned_text.len() > 200 { format!("{}...", &cleaned_text[..200]) } else { cleaned_text.clone() });
    
    // Parse JSON with better error reporting
    let parsed: Value = serde_json::from_str(&cleaned_text).map_err(|e| {
        // Try to extract at least the summary and steps even if proposed_changes is incomplete
        let mut partial_json = json!({
            "summary": "Agent response incomplete",
            "steps": [],
            "proposed_changes": []
        });
        
        // Try to extract summary if present
        if let Some(summary_start) = cleaned_text.find("\"summary\"") {
            if let Some(colon) = cleaned_text[summary_start..].find(':') {
                let after_colon = &cleaned_text[summary_start + colon + 1..];
                if let Some(quote_start) = after_colon.find('"') {
                    if let Some(quote_end) = after_colon[quote_start + 1..].find('"') {
                        let summary_val = &after_colon[quote_start + 1..quote_start + 1 + quote_end];
                        partial_json["summary"] = json!(summary_val);
                    }
                }
            }
        }
        
        // Log detailed error with context
        let error_line = e.to_string();
        let error_pos = if let Some(line_pos) = error_line.find("line") {
            error_line[line_pos..].chars().take(50).collect::<String>()
        } else {
            error_line.clone()
        };
        
        let msg = format!(
            "Failed to parse agent JSON response: {}.\n\nCleaned JSON preview (first 800 chars):\n{}\n\nRaw response preview (first 1000 chars):\n{}", 
            error_pos,
            if cleaned_text.len() > 800 { 
                format!("{}...", &cleaned_text[..800]) 
            } else { 
                cleaned_text.clone() 
            },
            if llm_response.text.len() > 1000 { 
                format!("{}...", &llm_response.text[..1000]) 
            } else { 
                llm_response.text.clone() 
            }
        );
        
        eprintln!("❌ JSON parse error: {}", msg);
        eprintln!("📋 Attempting to use partial JSON with summary: {}", partial_json["summary"]);
        
        let conn = db.get_connection();
        if let Ok(conn_guard) = conn.lock() {
            let _ = conn_guard.execute(
                "UPDATE agent_runs SET status = 'failed', error_text = ?1, finished_at = ?2, updated_at = ?2 WHERE id = ?3",
                rusqlite::params![msg, Utc::now().to_rfc3339(), run_id],
            );
        }
        msg
    })?;

    let summary = parsed
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("Agent completed planning.")
        .to_string();

    // Store steps, if any
    if let Some(steps) = parsed.get("steps").and_then(|v| v.as_array()) {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;

        for (idx, step) in steps.iter().enumerate() {
            let description = step
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let step_id = Uuid::new_v4().to_string();
            conn_guard
                .execute(
                    "INSERT INTO agent_steps (
                        id, run_id, step_index, step_type,
                        description, tool_name, params_json, result_summary, created_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                    rusqlite::params![
                        step_id,
                        run_id,
                        idx as i64,
                        "plan",
                        description,
                        None::<String>,
                        None::<String>,
                        None::<String>,
                        Utc::now().to_rfc3339(),
                    ],
                )
                .map_err(|e| format!("Failed to insert agent_step: {}", e))?;
        }
    }

    // Build proposed_changes payload
    let mut proposed_changes: Vec<AgentProposedChange> = Vec::new();
    if let Some(changes) = parsed
        .get("proposed_changes")
        .and_then(|v| v.as_array())
    {
        for change in changes {
            if let (Some(file_path), Some(new_content)) =
                (change.get("file_path"), change.get("new_content"))
            {
                if let (Some(file_path_str), Some(new_content_str)) =
                    (file_path.as_str(), new_content.as_str())
                {
                    let description = change
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    proposed_changes.push(AgentProposedChange {
                        file_path: file_path_str.to_string(),
                        description,
                        new_content: new_content_str.to_string(),
                    });
                }
            }
        }
    }

    // Mark run as complete
    {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;

        conn_guard
            .execute(
                "UPDATE agent_runs SET status = 'complete', finished_at = ?1, updated_at = ?1 WHERE id = ?2",
                rusqlite::params![Utc::now().to_rfc3339(), run_id],
            )
            .map_err(|e| format!("Failed to update agent_run: {}", e))?;
    }

    Ok(CoderAgentTaskResponse {
        run_id,
        status: "complete".to_string(),
        summary,
        proposed_changes,
    })
}

#[tauri::command]
pub async fn list_coder_workflows(
    db: State<'_, Database>,
) -> Result<Vec<CoderWorkflow>, String> {
    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    let mut stmt = conn_guard
        .prepare(
            "SELECT id, name, description, workflow_json, created_at, updated_at
             FROM coder_workflows
             ORDER BY updated_at DESC",
        )
        .map_err(|e| format!("Database error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            let workflow_json_str: String = row.get(3)?;
            let workflow_json: Value =
                serde_json::from_str(&workflow_json_str).unwrap_or_else(|_| json!({}));

            Ok(CoderWorkflow {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                workflow_json,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|e| format!("Database error: {}", e))?;

    let mut workflows = Vec::new();
    for row in rows {
        workflows.push(row.map_err(|e| format!("Row error: {}", e))?);
    }

    Ok(workflows)
}

#[tauri::command]
pub async fn save_coder_workflow(
    db: State<'_, Database>,
    request: SaveCoderWorkflowRequest,
) -> Result<String, String> {
    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = Utc::now().to_rfc3339();
    let workflow_json_str =
        serde_json::to_string(&request.workflow_json).unwrap_or_else(|_| "{}".to_string());

    conn_guard
        .execute(
            "INSERT OR REPLACE INTO coder_workflows
                (id, name, description, workflow_json, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4,
                     COALESCE((SELECT created_at FROM coder_workflows WHERE id = ?1), ?5),
                     ?6)",
            rusqlite::params![
                id,
                request.name,
                request.description,
                workflow_json_str,
                now,
                now
            ],
        )
        .map_err(|e| format!("Failed to save workflow: {}", e))?;

    Ok(id)
}

#[tauri::command]
pub async fn delete_coder_workflow(
    db: State<'_, Database>,
    workflow_id: String,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    conn_guard
        .execute(
            "DELETE FROM coder_workflows WHERE id = ?1",
            rusqlite::params![workflow_id],
        )
        .map_err(|e| format!("Failed to delete workflow: {}", e))?;

    Ok(())
}

/// Record that specific proposed changes were applied for an agent run.
///
/// The actual file writes are performed on the frontend via workspace APIs;
/// this endpoint only logs an \"apply\" step into agent_steps for observability.
#[tauri::command]
pub async fn coder_agent_record_apply_steps(
    db: State<'_, Database>,
    request: CoderAgentApplyRequest,
) -> Result<(), String> {
    if request.changes.is_empty() {
        return Ok(());
    }

    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    for (idx, change) in request.changes.iter().enumerate() {
        let step_id = Uuid::new_v4().to_string();
        let description = change
            .description
            .clone()
            .unwrap_or_else(|| format!("Applied change to {}", change.file_path));

        let result_summary = format!("Applied change to {}", change.file_path);

        conn_guard
            .execute(
                "INSERT INTO agent_steps (
                    id, run_id, step_index, step_type,
                    description, tool_name, params_json, result_summary, created_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    step_id,
                    request.run_id,
                    idx as i64,
                    "apply",
                    description,
                    Some("apply_changes".to_string()),
                    None::<String>,
                    result_summary,
                    Utc::now().to_rfc3339(),
                ],
            )
            .map_err(|e| format!("Failed to insert agent_step (apply): {}", e))?;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRunSummary {
    pub id: String,
    pub task_description: String,
    pub status: String,
    pub provider_id: Option<String>,
    pub model_name: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error_text: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentStep {
    pub id: String,
    pub run_id: String,
    pub step_index: i64,
    pub step_type: String,
    pub description: Option<String>,
    pub tool_name: Option<String>,
    pub result_summary: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub async fn list_agent_runs(
    db: State<'_, Database>,
) -> Result<Vec<AgentRunSummary>, String> {
    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    let mut stmt = conn_guard
        .prepare(
            "SELECT id, task_description, status, provider_id, model_name, created_at, started_at, finished_at, error_text
             FROM agent_runs
             ORDER BY created_at DESC",
        )
        .map_err(|e| format!("Database error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            Ok(AgentRunSummary {
                id: row.get(0)?,
                task_description: row.get(1)?,
                status: row.get(2)?,
                provider_id: row.get(3)?,
                model_name: row.get(4)?,
                created_at: row.get(5)?,
                started_at: row.get(6)?,
                finished_at: row.get(7)?,
                error_text: row.get(8)?,
            })
        })
        .map_err(|e| format!("Database error: {}", e))?;

    let mut runs = Vec::new();
    for row in rows {
        runs.push(row.map_err(|e| format!("Row error: {}", e))?);
    }

    Ok(runs)
}

#[tauri::command]
pub async fn get_agent_run_steps(
    db: State<'_, Database>,
    run_id: String,
) -> Result<Vec<AgentStep>, String> {
    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| format!("Database lock error: {}", e))?;

    let mut stmt = conn_guard
        .prepare(
            "SELECT id, run_id, step_index, step_type, description, tool_name, result_summary, created_at
             FROM agent_steps
             WHERE run_id = ?1
             ORDER BY step_index ASC, created_at ASC",
        )
        .map_err(|e| format!("Database error: {}", e))?;

    let rows = stmt
        .query_map(rusqlite::params![run_id], |row| {
            Ok(AgentStep {
                id: row.get(0)?,
                run_id: row.get(1)?,
                step_index: row.get(2)?,
                step_type: row.get(3)?,
                description: row.get(4)?,
                tool_name: row.get(5)?,
                result_summary: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| format!("Database error: {}", e))?;

    let mut steps = Vec::new();
    for row in rows {
        steps.push(row.map_err(|e| format!("Row error: {}", e))?);
    }

    Ok(steps)
}

#[tauri::command]
pub async fn load_coder_chats(
    db: State<'_, Database>,
) -> Result<Vec<CoderChat>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    let mut stmt = conn_guard
        .prepare("SELECT id, title, messages_json, created_at, updated_at FROM coder_chats ORDER BY updated_at DESC")
        .map_err(|e| format!("Database error: {}", e))?;

    let rows = stmt
        .query_map([], |row| {
            let messages_json: String = row.get(2)?;
            let messages: Vec<CoderMessage> = serde_json::from_str(&messages_json).unwrap_or_default();
            
            Ok(CoderChat {
                id: row.get(0)?,
                title: row.get(1)?,
                messages,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|e| format!("Database error: {}", e))?;

    let mut chats = Vec::new();
    for row in rows {
        chats.push(row.map_err(|e| format!("Row error: {}", e))?);
    }

    Ok(chats)
}

#[tauri::command]
pub async fn save_coder_chat(
    db: State<'_, Database>,
    chat: CoderChat,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    let messages_json = serde_json::to_string(&chat.messages)
        .map_err(|e| format!("Failed to serialize messages: {}", e))?;

    conn_guard.execute(
        "INSERT OR REPLACE INTO coder_chats (id, title, messages_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![chat.id, chat.title, messages_json, chat.created_at, chat.updated_at],
    )
    .map_err(|e| format!("Failed to save chat: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn delete_coder_chat(
    db: State<'_, Database>,
    chat_id: String,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

    conn_guard.execute(
        "DELETE FROM coder_chats WHERE id = ?1",
        [&chat_id],
    )
    .map_err(|e| format!("Failed to delete chat: {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn get_system_stats() -> Result<serde_json::Value, String> {
    use std::process::Command;
    
    // Get system stats using PowerShell on Windows
    #[cfg(windows)]
    {
        let cpu_output = Command::new("powershell")
            .args(["-Command", "(Get-Counter '\\Processor(_Total)\\% Processor Time').CounterSamples.CookedValue"])
            .output()
            .ok();
        
        let cpu_usage: f64 = cpu_output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0.0);

        // Get memory info
        let mem_output = Command::new("powershell")
            .args(["-Command", "$os = Get-CimInstance Win32_OperatingSystem; @{Total=$os.TotalVisibleMemorySize*1024;Free=$os.FreePhysicalMemory*1024} | ConvertTo-Json"])
            .output()
            .ok();

        let (memory_used, memory_total) = mem_output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|v| {
                let total = v["Total"].as_u64().unwrap_or(16 * 1024 * 1024 * 1024);
                let free = v["Free"].as_u64().unwrap_or(8 * 1024 * 1024 * 1024);
                (total - free, total)
            })
            .unwrap_or((8 * 1024 * 1024 * 1024, 16 * 1024 * 1024 * 1024));

        // Get disk info
        let disk_output = Command::new("powershell")
            .args(["-Command", "$d = Get-PSDrive C; @{Used=$d.Used;Free=$d.Free;Total=$d.Used+$d.Free} | ConvertTo-Json"])
            .output()
            .ok();

        let (disk_used, disk_total) = disk_output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|v| {
                let used = v["Used"].as_u64().unwrap_or(256 * 1024 * 1024 * 1024);
                let total = v["Total"].as_u64().unwrap_or(512 * 1024 * 1024 * 1024);
                (used, total)
            })
            .unwrap_or((256 * 1024 * 1024 * 1024, 512 * 1024 * 1024 * 1024));

        // Try to get GPU info (NVIDIA)
        let gpu_output = Command::new("nvidia-smi")
            .args(["--query-gpu=utilization.gpu,memory.used,memory.total", "--format=csv,noheader,nounits"])
            .output()
            .ok();

        let gpu_stats = gpu_output
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| {
                let parts: Vec<&str> = s.trim().split(',').collect();
                if parts.len() >= 3 {
                    let usage: f64 = parts[0].trim().parse().ok()?;
                    let mem_used: u64 = parts[1].trim().parse::<u64>().ok()? * 1024 * 1024;
                    let mem_total: u64 = parts[2].trim().parse::<u64>().ok()? * 1024 * 1024;
                    Some((usage, mem_used, mem_total))
                } else {
                    None
                }
            });

        let mut result = json!({
            "cpu_usage": cpu_usage,
            "memory_used": memory_used,
            "memory_total": memory_total,
            "disk_used": disk_used,
            "disk_total": disk_total
        });

        if let Some((gpu_usage, gpu_mem_used, gpu_mem_total)) = gpu_stats {
            result["gpu_usage"] = json!(gpu_usage);
            result["gpu_memory_used"] = json!(gpu_mem_used);
            result["gpu_memory_total"] = json!(gpu_mem_total);
        }

        Ok(result)
    }

    #[cfg(not(windows))]
    {
        Ok(json!({
            "cpu_usage": 25.0,
            "memory_used": 8 * 1024 * 1024 * 1024u64,
            "memory_total": 16 * 1024 * 1024 * 1024u64,
            "disk_used": 256 * 1024 * 1024 * 1024u64,
            "disk_total": 512 * 1024 * 1024 * 1024u64
        }))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExportCoderChatsRequest {
    pub chat_ids: Vec<String>,
    pub message_ids: Option<Vec<String>>, // If None, export all messages from chats
    pub project_id: String,
    pub local_model_id: Option<String>,
}

#[tauri::command]
pub async fn export_coder_chats_to_training(
    db: State<'_, Database>,
    request: ExportCoderChatsRequest,
) -> Result<usize, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    if request.chat_ids.is_empty() {
        return Err("No chats selected for export".to_string());
    }
    
    let mut exported_count = 0;
    
    // Load all selected chats
    let placeholders: String = request.chat_ids.iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(",");
    
    let query = format!(
        "SELECT id, messages_json FROM coder_chats WHERE id IN ({})",
        placeholders
    );
    
    let mut stmt = conn_guard
        .prepare(&query)
        .map_err(|e| format!("Database error: {}", e))?;
    
    let params: Vec<&dyn rusqlite::ToSql> = request.chat_ids.iter()
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
        let (chat_id, messages_json) = row.map_err(|e| format!("Row error: {}", e))?;
        
        let messages: Vec<CoderMessage> = serde_json::from_str(&messages_json)
            .map_err(|e| format!("Failed to parse messages: {}", e))?;
        
        // Filter messages if specific IDs provided
        let messages_to_export: Vec<&CoderMessage> = if let Some(selected_ids) = &request.message_ids {
            messages.iter()
                .filter(|msg| selected_ids.contains(&msg.id))
                .collect()
        } else {
            messages.iter().collect()
        };
        
        // Pair user messages with assistant responses
        let mut current_user_message: Option<&str> = None;
        for msg in messages_to_export {
            match msg.role.as_str() {
                "user" => {
                    current_user_message = Some(&msg.content);
                }
                "assistant" => {
                    if let Some(user_msg) = current_user_message.take() {
                        // Create training data entry
                        let training_id = uuid::Uuid::new_v4().to_string();
                        let now = chrono::Utc::now().to_rfc3339();
                        
                        let metadata = serde_json::json!({
                            "source": "coder_chat_export",
                            "chat_id": chat_id,
                            "message_id": msg.id,
                            "model": msg.model,
                            "provider": msg.provider,
                            "exported_at": now
                        });
                        
                        conn_guard.execute(
                            "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                            rusqlite::params![
                                training_id,
                                request.project_id,
                                request.local_model_id,
                                user_msg,
                                msg.content,
                                serde_json::to_string(&metadata).unwrap_or_default(),
                                now
                            ],
                        )
                        .map_err(|e| format!("Failed to create training data: {}", e))?;
                        
                        exported_count += 1;
                    }
                }
                _ => {}
            }
        }
    }
    
    Ok(exported_count)
}
