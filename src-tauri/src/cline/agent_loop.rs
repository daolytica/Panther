// Cline agent execution loop with tool approval

use crate::db::Database;
use crate::providers::get_adapter;
use crate::ProviderAccount;
use crate::types::{PromptPacket, Message};
use crate::cline::tools::ClineToolRequest;
use crate::cline::checkpoints::create_checkpoint;
use crate::cline::context_builder::ContextBuilder;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClineTaskResult {
    pub run_id: String,
    pub status: String,
    pub summary: String,
    pub tool_executions: Vec<ToolExecution>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolExecution {
    pub id: String,
    pub step_index: i32,
    pub tool_type: String,
    pub tool_params: Value,
    pub approval_status: String,
    pub result: Option<Value>,
}

pub struct ClineAgentLoop {
    db: Database,
    workspace_path: PathBuf,
}

impl ClineAgentLoop {
    pub fn new(db: Database, workspace_path: PathBuf) -> Self {
        ClineAgentLoop {
            db,
            workspace_path,
        }
    }
    
    /// Execute a Cline agent task with tool loop
    pub async fn execute_task(
        &self,
        task: String,
        provider: ProviderAccount,
        model_name: String,
        conversation_context: Option<Vec<serde_json::Value>>,
    ) -> Result<ClineTaskResult, String> {
        let run_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        
        // Create run record
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
            conn_guard.execute(
                "INSERT INTO cline_runs (id, task_description, provider_id, model_name, status, workspace_path, created_at, started_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    run_id,
                    task,
                    provider.id,
                    model_name,
                    "running",
                    self.workspace_path.to_string_lossy(),
                    now,
                    now
                ],
            )
            .map_err(|e| format!("Failed to create run: {}", e))?;
        }
        
        eprintln!("📋 Building context for workspace: {:?}", self.workspace_path);
        // Build context (don't block on failures)
        let context = match ContextBuilder::build_context(&self.workspace_path, None).await {
            Ok(ctx) => {
                eprintln!("✅ Context built successfully ({} chars)", ctx.len());
                ctx
            }
            Err(e) => {
                eprintln!("⚠️ Context build failed (non-fatal): {}", e);
                format!("Workspace: {:?}\n(Detailed context unavailable)", self.workspace_path)
            }
        };
        
        // Create initial checkpoint in background (don't block on it)
        eprintln!("💾 Spawning checkpoint creation in background (non-blocking)...");
        let db_for_checkpoint = self.db.clone();
        let run_id_for_checkpoint = run_id.clone();
        let workspace_path_for_checkpoint = self.workspace_path.clone();
        tokio::spawn(async move {
            eprintln!("💾 Background checkpoint task started");
            if let Err(e) = create_checkpoint(
                &db_for_checkpoint,
                &run_id_for_checkpoint,
                0,
                &workspace_path_for_checkpoint,
            ).await {
                eprintln!("⚠️ Checkpoint creation failed (non-fatal): {}", e);
            } else {
                eprintln!("✅ Checkpoint created successfully");
            }
        });
        eprintln!("💾 Checkpoint task spawned, continuing with LLM request...");
        
        // Build system prompt for Cline agent - request structured JSON with tool calls
        let system_prompt = format!(
            "You are Cline, an advanced AI coding assistant with FULL SYSTEM ACCESS and powerful tools.\n\
            CRITICAL: You MUST respond with ONLY valid JSON. NO markdown, NO code blocks, NO explanations, NO text before or after.\n\
            Your ENTIRE response must be a single valid JSON object starting with {{ and ending with }}.\n\
            Example of CORRECT format:\n\
            {{\"summary\":\"Create Python script\",\"steps\":[{{\"description\":\"Step 1\"}}],\"tool_requests\":[{{\"type\":\"workspace_write\",\"path\":\"script.py\",\"content\":\"print(\\\"hello\\\")\"}}]}}\n\n\
            Required JSON schema:\n\
            {{\n\
              \"summary\": \"brief description of what you will do\",\n\
              \"steps\": [ {{ \"description\": \"step description\" }} ],\n\
              \"tool_requests\": [\n\
                {{\n\
                  \"type\": \"workspace_write\",\n\
                  \"path\": \"file path (use forward slashes)\",\n\
                  \"content\": \"full file content\"\n\
                }},\n\
                {{\n\
                  \"type\": \"terminal\",\n\
                  \"command\": \"command to execute\",\n\
                  \"cwd\": \"optional working directory\"\n\
                }},\n\
                {{\n\
                  \"type\": \"directory_create\",\n\
                  \"path\": \"directory path\"\n\
                }}\n\
              ]\n\
            }}\n\n\
            Available tool types:\n\
            - workspace_write: Create/edit a file (path, content)\n\
            - workspace_read: Read a file (path)\n\
            - terminal: Run a command (command, cwd optional)\n\
            - directory_create: Create directory (path)\n\
            - file_delete: Delete file (path)\n\
            - analyze_ast: Analyze code structure (path)\n\
            - search_files: Search for files (pattern, regex)\n\
            - search_code: Search code (pattern, language)\n\
            - browser_launch: Launch browser (url)\n\
            - browser_click: Click element (selector)\n\
            - browser_type: Type text (selector, text)\n\
            - browser_screenshot: Take screenshot\n\n\
            CRITICAL JSON RULES:\n\
            - Use forward slashes (/) in file paths\n\
            - Use RELATIVE paths only - just the filename or relative path from current directory\n\
            - DO NOT use absolute paths like C:\\Users\\... - use just the filename (e.g., \"cpu_usage.ps1\")\n\
            - The content field must contain FULL file content as a JSON string\n\
            - Escape quotes: use \\\" for quotes inside strings\n\
            - Your response must be ONLY the JSON object, nothing else\n\
            - Each tool request will require user approval before execution\n\n\
            Current workspace directory: {:?}\n\
            Workspace context:\n{}",
            self.workspace_path, context
        );
        
        eprintln!("📤 Sending request to LLM (provider: {}, model: {})", provider.provider_type, model_name);
        // Convert conversation context to Message format if provided
        let conversation_messages: Option<Vec<Message>> = conversation_context.as_ref().map(|ctx| {
            ctx.iter().enumerate().map(|(idx, msg)| {
                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                Message {
                    id: format!("cline-msg-{}", idx),
                    run_id: "".to_string(), // Not needed for conversation context
                    author_type: if role == "user" { "user" } else { "assistant" }.to_string(),
                    profile_id: None,
                    round_index: Some(idx as i32),
                    turn_index: Some(idx as i32),
                    text: content.to_string(),
                    created_at: Utc::now().to_rfc3339(),
                    provider_metadata_json: None,
                }
            }).collect()
        });
        
        let packet = PromptPacket {
            global_instructions: Some(system_prompt),
            persona_instructions: "You are Cline, a helpful and careful coding assistant.".to_string(),
            user_message: task.clone(),
            conversation_context: conversation_messages,
            params_json: json!({
                "temperature": 0.4,
                "max_tokens": 4096
            }),
            stream: false,
        };
        
        let adapter = get_adapter(&provider.provider_type)
            .map_err(|e| {
                eprintln!("❌ Failed to get adapter for {}: {}", provider.provider_type, e);
                format!("Failed to get adapter: {}", e)
            })?;
        
        eprintln!("⏳ Waiting for LLM response...");
        // Get initial response from LLM
        let response = adapter.complete(&packet, &provider, &model_name).await
            .map_err(|e| {
                eprintln!("❌ LLM error: {}", e);
                format!("LLM error: {}", e)
            })?;
        
        eprintln!("✅ LLM response received ({} chars)", response.text.len());
        eprintln!("📄 Full LLM response:\n{}", response.text);
        
        // Parse tool requests from LLM response
        eprintln!("🔍 Parsing tool requests from LLM response...");
        let tool_executions = match Self::parse_tool_requests_from_response(
            &self.db,
            &run_id,
            &response.text,
            &self.workspace_path,
        ).await {
            Ok(executions) => {
                eprintln!("✅ Parsed {} tool execution(s)", executions.len());
                executions
            }
            Err(e) => {
                eprintln!("⚠️ Failed to parse tool requests (non-fatal): {}", e);
                eprintln!("ℹ️ Treating as text-only response");
                Vec::new()
            }
        };
        
        // Update run status
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
            conn_guard.execute(
                "UPDATE cline_runs SET status = ?1, finished_at = ?2 WHERE id = ?3",
                rusqlite::params!["complete", Utc::now().to_rfc3339(), run_id],
            )
            .map_err(|e| format!("Failed to update run: {}", e))?;
        }
        
        // Extract summary from parsed JSON if available
        let summary = Self::extract_summary_from_response(&response.text)
            .unwrap_or_else(|| response.text.clone());
        
        Ok(ClineTaskResult {
            run_id,
            status: "complete".to_string(),
            summary,
            tool_executions,
        })
    }
    
    /// Parse tool requests from LLM JSON response
    async fn parse_tool_requests_from_response(
        db: &Database,
        run_id: &str,
        response_text: &str,
        workspace_path: &PathBuf,
    ) -> Result<Vec<ToolExecution>, String> {
            // Clean and parse JSON (similar to Panther Coder)
            let cleaned_text = {
                let mut text = response_text.trim().to_string();
                
                // Remove control characters
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
                
                // Remove any leading text before the first {
                if let Some(start_idx) = text.find('{') {
                    if start_idx > 0 {
                        eprintln!("⚠️ Found text before JSON, removing {} chars", start_idx);
                        text = text[start_idx..].to_string();
                    }
                }
                
                // Find the first { and extract complete JSON object
                if let Some(start_idx) = text.find('{') {
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
                    }
                }
                
                text
            };
            
            eprintln!("🔍 Parsing JSON ({} chars)", cleaned_text.len());
            eprintln!("📄 JSON preview (first 500 chars): {}", 
                cleaned_text.chars().take(500).collect::<String>());
            
            // Try to parse JSON with better error recovery
            let parsed: Value = match serde_json::from_str(&cleaned_text) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("❌ JSON parse error: {} at line {} column {}", e, 
                        e.line(), e.column());
                    eprintln!("📄 Full JSON text ({} chars):\n{}", cleaned_text.len(), cleaned_text);
                    
                    // Try to fix common JSON issues
                    let mut fixed_text = cleaned_text.clone();
                    
                    // Fix unescaped newlines in strings
                    fixed_text = fixed_text.replace("\n", "\\n").replace("\r", "\\r");
                    
                    // Try parsing again
                    match serde_json::from_str(&fixed_text) {
                        Ok(v) => {
                            eprintln!("✅ Fixed JSON and parsed successfully");
                            v
                        }
                        Err(e2) => {
                            eprintln!("❌ Still failed after fix attempt: {}", e2);
                            // Try to extract just the tool_requests array if it exists
                            if let Some(start) = fixed_text.find("\"tool_requests\"") {
                                if let Some(bracket_start) = fixed_text[start..].find('[') {
                                    let array_start = start + bracket_start;
                                    let mut bracket_count = 0;
                                    let mut in_string = false;
                                    let mut escape_next = false;
                                    let mut array_end = None;
                                    
                                    for (i, ch) in fixed_text[array_start..].char_indices() {
                                        if escape_next {
                                            escape_next = false;
                                            continue;
                                        }
                                        match ch {
                                            '\\' if in_string => escape_next = true,
                                            '"' => in_string = !in_string,
                                            '[' if !in_string => bracket_count += 1,
                                            ']' if !in_string => {
                                                bracket_count -= 1;
                                                if bracket_count == 0 {
                                                    array_end = Some(array_start + i + 1);
                                                    break;
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                    
                                    if let Some(end) = array_end {
                                        let array_text = &fixed_text[array_start..end];
                                        eprintln!("🔧 Attempting to parse extracted array: {}", 
                                            array_text.chars().take(200).collect::<String>());
                                        // Try to parse as array directly
                                        if let Ok(arr) = serde_json::from_str::<Vec<Value>>(array_text) {
                                            eprintln!("✅ Successfully parsed extracted array with {} items", arr.len());
                                            return Err(format!("JSON parse error, but extracted {} tool requests. Please fix JSON format.", arr.len()));
                                        }
                                    }
                                }
                            }
                            return Err(format!("Failed to parse tool requests JSON: {}. Original error: {}", e2, e));
                        }
                    }
                }
            };
            
            // Extract tool_requests array (optional - LLM might just respond with text)
            let empty_vec: Vec<serde_json::Value> = Vec::new();
            let tool_requests = parsed.get("tool_requests")
                .and_then(|v| v.as_array())
                .unwrap_or(&empty_vec);
            
            eprintln!("📋 Found {} tool request(s)", tool_requests.len());
            
            if tool_requests.is_empty() {
                eprintln!("ℹ️ No tool requests in response - agent provided text response only");
                return Ok(Vec::new());
            }
            
            // Convert each tool request to ToolExecution
            let mut tool_executions = Vec::new();
            for (idx, tool_req) in tool_requests.iter().enumerate() {
                let tool_type = tool_req.get("type")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| format!("Tool request {} missing 'type'", idx))?;
                
                // Convert JSON tool request to ClineToolRequest
                let cline_tool = Self::json_to_cline_tool(tool_req, tool_type, workspace_path)?;
                
                // Create tool execution record
                let tool_id = Uuid::new_v4().to_string();
                let tool_params = serde_json::to_value(&cline_tool)
                    .map_err(|e| format!("Failed to serialize tool: {}", e))?;
                
                // Store in database
                {
                    let conn = db.get_connection();
                    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
                    conn_guard.execute(
                        "INSERT INTO cline_tool_executions (id, run_id, step_index, tool_type, tool_params_json, approval_status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        rusqlite::params![
                            tool_id,
                            run_id,
                            idx as i32,
                            tool_type,
                            serde_json::to_string(&tool_params).map_err(|e| format!("JSON error: {}", e))?,
                            "pending"
                        ],
                    )
                    .map_err(|e| format!("Failed to store tool execution: {}", e))?;
                }
                
                eprintln!("✅ Created tool execution {}: {}", tool_id, tool_type);
                
                tool_executions.push(ToolExecution {
                    id: tool_id,
                    step_index: idx as i32,
                    tool_type: tool_type.to_string(),
                    tool_params,
                    approval_status: "pending".to_string(),
                    result: None,
                });
            }
            
            Ok(tool_executions)
    }
    
    /// Convert JSON tool request to ClineToolRequest
    fn json_to_cline_tool(tool_req: &Value, tool_type: &str, workspace_path: &PathBuf) -> Result<ClineToolRequest, String> {
        // Helper to normalize paths - convert absolute to relative if they're under workspace
        let normalize_path = |path: &str| -> String {
            // If path contains absolute Windows path pattern, extract just the filename
            // This handles cases like "C:\Users\username\Documents\file.ps1"
            if path.contains(":\\") || path.starts_with("C:\\") || path.starts_with("D:\\") || 
               path.contains("Users\\") || path.contains("Users/") {
                // Extract just the filename
                if let Some(filename) = std::path::Path::new(path).file_name() {
                    let normalized = filename.to_string_lossy().to_string();
                    eprintln!("📝 Normalized absolute path '{}' to '{}'", path, normalized);
                    return normalized;
                }
            }
            // If path is absolute and under workspace, make it relative
            if let Ok(absolute_path) = std::fs::canonicalize(path) {
                if let Ok(relative) = absolute_path.strip_prefix(workspace_path) {
                    let normalized = relative.to_string_lossy().replace('\\', "/");
                    eprintln!("📝 Normalized workspace path '{}' to '{}'", path, normalized);
                    return normalized;
                }
            }
            // Use forward slashes and return as-is
            path.replace('\\', "/")
        };
        
        match tool_type {
            "workspace_write" => {
                let path = tool_req.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'path' in workspace_write")?;
                let content = tool_req.get("content")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'content' in workspace_write")?;
                Ok(ClineToolRequest::WorkspaceWrite {
                    path: normalize_path(path),
                    content: content.to_string(),
                    create_dirs: tool_req.get("create_dirs").and_then(|v| v.as_bool()),
                    permissions: tool_req.get("permissions").and_then(|v| v.as_str()).map(|s| s.to_string()),
                })
            }
            "workspace_read" => {
                let path = tool_req.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'path' in workspace_read")?;
                Ok(ClineToolRequest::WorkspaceRead {
                    path: normalize_path(path),
                })
            }
            "terminal" => {
                let command = tool_req.get("command")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'command' in terminal")?;
                let cwd = tool_req.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());
                Ok(ClineToolRequest::Terminal {
                    command: command.to_string(),
                    cwd,
                    elevated: tool_req.get("elevated").and_then(|v| v.as_bool()),
                })
            }
            "directory_create" => {
                let path = tool_req.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'path' in directory_create")?;
                Ok(ClineToolRequest::DirectoryCreate {
                    path: normalize_path(path),
                    recursive: tool_req.get("recursive").and_then(|v| v.as_bool()),
                    permissions: tool_req.get("permissions").and_then(|v| v.as_str()).map(|s| s.to_string()),
                })
            }
            "file_delete" => {
                let path = tool_req.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'path' in file_delete")?;
                Ok(ClineToolRequest::FileDelete {
                    path: normalize_path(path),
                    recursive: tool_req.get("recursive").and_then(|v| v.as_bool()),
                })
            }
            "analyze_ast" => {
                let path = tool_req.get("path")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'path' in analyze_ast")?;
                Ok(ClineToolRequest::AnalyzeAST {
                    path: normalize_path(path),
                })
            }
            "search_files" => {
                let pattern = tool_req.get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'pattern' in search_files")?;
                Ok(ClineToolRequest::SearchFiles {
                    pattern: pattern.to_string(),
                    regex: tool_req.get("regex").and_then(|v| v.as_bool()).unwrap_or(false),
                    include_system: tool_req.get("include_system").and_then(|v| v.as_bool()),
                })
            }
            "search_code" => {
                let pattern = tool_req.get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'pattern' in search_code")?;
                Ok(ClineToolRequest::SearchCode {
                    pattern: pattern.to_string(),
                    language: tool_req.get("language").and_then(|v| v.as_str()).map(|s| s.to_string()),
                })
            }
            "browser_launch" => {
                let url = tool_req.get("url")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'url' in browser_launch")?;
                Ok(ClineToolRequest::BrowserLaunch {
                    url: url.to_string(),
                    headless: tool_req.get("headless").and_then(|v| v.as_bool()),
                    user_agent: tool_req.get("user_agent").and_then(|v| v.as_str()).map(|s| s.to_string()),
                })
            }
            "browser_click" => {
                let selector = tool_req.get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'selector' in browser_click")?;
                Ok(ClineToolRequest::BrowserClick {
                    selector: selector.to_string(),
                })
            }
            "browser_type" => {
                let selector = tool_req.get("selector")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'selector' in browser_type")?;
                let text = tool_req.get("text")
                    .and_then(|v| v.as_str())
                    .ok_or("Missing 'text' in browser_type")?;
                Ok(ClineToolRequest::BrowserType {
                    selector: selector.to_string(),
                    text: text.to_string(),
                })
            }
            "browser_screenshot" => {
                Ok(ClineToolRequest::BrowserScreenshot {
                    full_page: tool_req.get("full_page").and_then(|v| v.as_bool()),
                })
            }
            // Common model “alias” / mistake: directory_read
            // We treat it as a terminal command (content/command) executed in the provided path/cwd.
            "directory_read" => {
                let command = tool_req
                    .get("command")
                    .and_then(|v| v.as_str())
                    .or_else(|| tool_req.get("content").and_then(|v| v.as_str()))
                    .ok_or("Missing 'command' or 'content' in directory_read")?;

                let cwd = tool_req
                    .get("cwd")
                    .and_then(|v| v.as_str())
                    .or_else(|| tool_req.get("path").and_then(|v| v.as_str()))
                    .map(|s| s.to_string());

                Ok(ClineToolRequest::Terminal {
                    command: command.to_string(),
                    cwd,
                    elevated: tool_req.get("elevated").and_then(|v| v.as_bool()),
                })
            }
            // Fallback: if an unknown tool type includes a command/content, treat it as terminal.
            _ => {
                let command = tool_req
                    .get("command")
                    .and_then(|v| v.as_str())
                    .or_else(|| tool_req.get("content").and_then(|v| v.as_str()));

                if let Some(command) = command {
                    let cwd = tool_req
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .or_else(|| tool_req.get("path").and_then(|v| v.as_str()))
                        .map(|s| s.to_string());

                    eprintln!(
                        "⚠️ Unknown tool type '{}' – falling back to terminal execution",
                        tool_type
                    );

                    return Ok(ClineToolRequest::Terminal {
                        command: command.to_string(),
                        cwd,
                        elevated: tool_req.get("elevated").and_then(|v| v.as_bool()),
                    });
                }

                Err(format!("Unknown tool type: {}", tool_type))
            }
        }
    }
    
    /// Extract summary from LLM response
    fn extract_summary_from_response(response_text: &str) -> Option<String> {
        // Try to parse JSON and extract summary
        let cleaned = {
            let mut text = response_text.trim().to_string();
            if let Some(start) = text.find("```json") {
                if let Some(end) = text[start + 7..].find("```") {
                    text = text[start + 7..start + 7 + end].trim().to_string();
                }
            } else if let Some(start) = text.find("```") {
                if let Some(end) = text[start + 3..].find("```") {
                    text = text[start + 3..start + 3 + end].trim().to_string();
                }
            }
            if let Some(start_idx) = text.find('{') {
                if let Some(end_idx) = text.rfind('}') {
                    text = text[start_idx..=end_idx].to_string();
                }
            }
            text
        };
        
        if let Ok(parsed) = serde_json::from_str::<Value>(&cleaned) {
            if let Some(summary) = parsed.get("summary").and_then(|v| v.as_str()) {
                return Some(summary.to_string());
            }
        }
        
        None
    }
    
    /// Execute a tool with approval tracking
    #[allow(dead_code)]
    pub async fn execute_tool_with_approval(
        &self,
        run_id: &str,
        step_index: i32,
        tool_request: ClineToolRequest,
    ) -> Result<ToolExecution, String> {
        let tool_id = Uuid::new_v4().to_string();
        let tool_type = match &tool_request {
            ClineToolRequest::Terminal { .. } => "terminal",
            ClineToolRequest::SystemCommand { .. } => "system_command",
            ClineToolRequest::WorkspaceRead { .. } => "workspace_read",
            ClineToolRequest::WorkspaceWrite { .. } => "workspace_write",
            ClineToolRequest::SystemFileRead { .. } => "system_file_read",
            ClineToolRequest::SystemFileWrite { .. } => "system_file_write",
            ClineToolRequest::FileDelete { .. } => "file_delete",
            ClineToolRequest::DirectoryCreate { .. } => "directory_create",
            ClineToolRequest::HttpRequest { .. } => "http_request",
            ClineToolRequest::NetworkScan { .. } => "network_scan",
            ClineToolRequest::ProcessList => "process_list",
            ClineToolRequest::ProcessKill { .. } => "process_kill",
            ClineToolRequest::ProcessStart { .. } => "process_start",
            ClineToolRequest::SystemInfo => "system_info",
            ClineToolRequest::EnvironmentVariables => "environment_variables",
            ClineToolRequest::RegistryRead { .. } => "registry_read",
            ClineToolRequest::RegistryWrite { .. } => "registry_write",
            ClineToolRequest::BrowserLaunch { .. } => "browser_launch",
            ClineToolRequest::BrowserClick { .. } => "browser_click",
            ClineToolRequest::BrowserType { .. } => "browser_type",
            ClineToolRequest::BrowserScreenshot { .. } => "browser_screenshot",
            ClineToolRequest::BrowserScroll { .. } => "browser_scroll",
            ClineToolRequest::BrowserExecuteJS { .. } => "browser_execute_js",
            ClineToolRequest::BrowserCookies { .. } => "browser_cookies",
            ClineToolRequest::AnalyzeAST { .. } => "analyze_ast",
            ClineToolRequest::SearchFiles { .. } => "search_files",
            ClineToolRequest::SearchCode { .. } => "search_code",
            ClineToolRequest::MCPCall { .. } => "mcp_call",
            ClineToolRequest::DatabaseQuery { .. } => "database_query",
            ClineToolRequest::ArchiveExtract { .. } => "archive_extract",
            ClineToolRequest::ArchiveCreate { .. } => "archive_create",
        };
        
        let tool_params = serde_json::to_value(&tool_request)
            .map_err(|e| format!("Failed to serialize tool: {}", e))?;
        
        // Store tool execution as pending
        {
            let conn = self.db.get_connection();
            let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
            conn_guard.execute(
                "INSERT INTO cline_tool_executions (id, run_id, step_index, tool_type, tool_params_json, approval_status) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    tool_id,
                    run_id,
                    step_index,
                    tool_type,
                    serde_json::to_string(&tool_params).map_err(|e| format!("JSON error: {}", e))?,
                    "pending"
                ],
            )
            .map_err(|e| format!("Failed to store tool execution: {}", e))?;
        }
        
        Ok(ToolExecution {
            id: tool_id,
            step_index,
            tool_type: tool_type.to_string(),
            tool_params,
            approval_status: "pending".to_string(),
            result: None,
        })
    }
}
