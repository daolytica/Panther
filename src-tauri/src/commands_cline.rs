// Cline IDE backend commands

use crate::db::Database;
use crate::ProviderAccount;
use crate::cline::ClineAgentLoop;
use crate::training_ingest;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::time::Instant;
use tauri::{State, AppHandle};
use chrono::Utc;
use tokio::time::{timeout, Duration};

// Import LinterError only when needed
#[allow(unused_imports)]
use crate::cline::error_monitor::LinterError;

#[derive(Debug, Serialize, Deserialize)]
pub struct ClineAgentTaskRequest {
    pub provider_id: String,
    pub model_name: String,
    pub task_description: String,
    pub workspace_path: String,
    pub target_paths: Option<Vec<String>>,
    pub conversation_context: Option<Vec<serde_json::Value>>, // Previous messages for continuous chat
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClineAgentTaskResponse {
    pub run_id: String,
    pub status: String,
    pub summary: String,
    pub tool_executions: Vec<ToolExecutionResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolExecutionResponse {
    pub id: String,
    pub step_index: i32,
    pub tool_type: String,
    pub tool_params: Value,
    pub approval_status: String,
    pub result: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApproveToolRequest {
    pub tool_id: String,
    pub approved: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateCheckpointRequest {
    pub run_id: String,
    pub step_index: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestoreCheckpointRequest {
    pub checkpoint_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IngestClineTurnRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub user_text: String,
    pub assistant_text: String,
    pub tool_executions: Vec<Value>,
    pub browser_steps: Option<Vec<Value>>,
    pub error_context: Option<String>,
}

/// Execute a Cline agent task
#[tauri::command]
pub async fn cline_agent_task(
    db: State<'_, Database>,
    _app: AppHandle,
    request: ClineAgentTaskRequest,
) -> Result<ClineAgentTaskResponse, String> {
    eprintln!("🚀 Cline agent task started: {}", request.task_description);
    let workspace_path = PathBuf::from(&request.workspace_path);
    eprintln!("📁 Workspace path: {:?}", workspace_path);
    
    // Load provider
    let provider: ProviderAccount = {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| {
                eprintln!("❌ Database lock error: {}", e);
                format!("Database lock error: {}", e)
            })?;
        
        conn_guard
            .query_row(
                "SELECT id, provider_type, display_name, base_url, region, auth_ref, created_at, updated_at, provider_metadata_json FROM provider_accounts WHERE id = ?1",
                [&request.provider_id],
                |row| {
                    Ok(ProviderAccount {
                        id: row.get(0)?,
                        provider_type: row.get(1)?,
                        display_name: row.get(2)?,
                        base_url: row.get(3)?,
                        region: row.get(4)?,
                        auth_ref: row.get(5)?,
                        created_at: row.get(6)?,
                        updated_at: row.get(7)?,
                        provider_metadata_json: row
                            .get::<_, Option<String>>(8)?
                            .and_then(|s| serde_json::from_str(&s).ok()),
                    })
                },
            )
            .map_err(|e| {
                eprintln!("❌ Failed to load provider {}: {}", request.provider_id, e);
                format!("Failed to load provider: {}", e)
            })?
    };
    
    eprintln!("✅ Provider loaded: {} ({})", provider.display_name, provider.provider_type);
    eprintln!("🤖 Model: {}", request.model_name);
    
    // Clone database for agent loop (Database is Clone)
    let agent_loop = ClineAgentLoop::new(db.inner().clone(), workspace_path);
    
    eprintln!("🔄 Executing agent task...");
    let result = agent_loop
        .execute_task(
            request.task_description.clone(),
            provider,
            request.model_name.clone(),
            request.conversation_context.clone(),
        )
        .await
        .map_err(|e| {
            eprintln!("❌ Agent task failed: {}", e);
            e
        })?;
    
    eprintln!("✅ Agent task completed: {}", result.run_id);
    eprintln!("📊 Response summary length: {} chars", result.summary.len());
    eprintln!("🔧 Tool executions: {}", result.tool_executions.len());
    
    // Convert to response format
    let tool_executions: Vec<ToolExecutionResponse> = result.tool_executions
        .into_iter()
        .map(|te| ToolExecutionResponse {
            id: te.id,
            step_index: te.step_index,
            tool_type: te.tool_type,
            tool_params: te.tool_params,
            approval_status: te.approval_status,
            result: te.result,
        })
        .collect();
    
    let response = ClineAgentTaskResponse {
        run_id: result.run_id.clone(),
        status: result.status.clone(),
        summary: result.summary.clone(),
        tool_executions: tool_executions.clone(),
    };
    
    eprintln!("📤 Returning response to frontend: run_id={}, status={}, summary_len={}, tools={}", 
        response.run_id, response.status, response.summary.len(), response.tool_executions.len());
    
    Ok(response)
}

/// Approve or reject a tool execution
#[tauri::command]
pub async fn cline_approve_tool(
    db: State<'_, Database>,
    request: ApproveToolRequest,
) -> Result<Value, String> {
    eprintln!("🔧 Approving tool: {} (approved: {})", request.tool_id, request.approved);
    
    // Get tool execution details
    let (tool_type, tool_params_json, _run_id, workspace_path_str): (String, String, String, String) = {
        let conn = db.get_connection();
        // Don't hang forever waiting for the DB mutex.
        let start = Instant::now();
        let conn_guard = loop {
            match conn.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::Poisoned(poison)) => {
                    eprintln!("⚠️ Database mutex poisoned; continuing");
                    break poison.into_inner();
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    // keep looping
                }
            }

            if start.elapsed() > Duration::from_secs(3) {
                return Err("Database is busy (lock contention). Try again in a moment.".to_string());
            }

            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        
        let mut stmt = conn_guard.prepare(
            "SELECT tool_type, tool_params_json, run_id FROM cline_tool_executions WHERE id = ?1"
        )
        .map_err(|e| {
            eprintln!("❌ Failed to prepare statement: {}", e);
            format!("Failed to prepare statement: {}", e)
        })?;
        
        let tool_info: (String, String, String) = stmt.query_row(
            rusqlite::params![request.tool_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        )
        .map_err(|e| {
            eprintln!("❌ Tool execution not found: {}", e);
            format!("Tool execution not found: {}", e)
        })?;
        
        eprintln!("📋 Tool info: type={}, run_id={}", tool_info.0, tool_info.2);
        
        // Get workspace path from run
        let mut run_stmt = conn_guard.prepare(
            "SELECT workspace_path FROM cline_runs WHERE id = ?1"
        )
        .map_err(|e| {
            eprintln!("❌ Failed to prepare run statement: {}", e);
            format!("Failed to prepare run statement: {}", e)
        })?;
        
        let workspace_path_raw: String = run_stmt.query_row(
            rusqlite::params![tool_info.2.clone()],
            |row| row.get(0)
        )
        .map_err(|e| {
            eprintln!("❌ Run not found: {}", e);
            format!("Run not found: {}", e)
        })?;
        
        // Resolve workspace path to absolute path
        let workspace_path = if std::path::Path::new(&workspace_path_raw).is_absolute() {
            workspace_path_raw
        } else {
            // Resolve relative path from user home
            let home = if cfg!(windows) {
                std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users".to_string())
            } else {
                std::env::var("HOME").unwrap_or_else(|_| "/home".to_string())
            };
            std::path::Path::new(&home)
                .join(&workspace_path_raw)
                .to_string_lossy()
                .to_string()
        };
        
        eprintln!("📁 Workspace path (resolved): {}", workspace_path);
        
        (tool_info.0, tool_info.1, tool_info.2, workspace_path)
    };
    
    let status = if request.approved { "approved" } else { "rejected" };
    
    // If approved, execute the tool
    let execution_result: Option<Value> = if request.approved {
        eprintln!("✅ Executing approved tool: {} (workspace: {})", tool_type, workspace_path_str);
        
        // Parse tool params and convert to ClineToolRequest
        let tool_params: Value = serde_json::from_str(&tool_params_json)
            .map_err(|e| {
                eprintln!("❌ Failed to parse tool params JSON: {} | JSON: {}", e, tool_params_json);
                format!("Failed to parse tool params: {}", e)
            })?;
        
        eprintln!("📦 Tool params parsed: {:?}", tool_params);
        
        let cline_tool: crate::cline::tools::ClineToolRequest = serde_json::from_value(tool_params.clone())
            .map_err(|e| {
                eprintln!("❌ Failed to deserialize tool: {} | Value: {:?}", e, tool_params);
                format!("Failed to deserialize tool: {}", e)
            })?;
        
        eprintln!("🔧 Calling execute_cline_tool...");

        // Execute the tool with a hard timeout so the UI doesn't hang forever.
        // Many terminal scans can take a long time (or hang); this ensures we always return.
        let timeout_secs: u64 = tool_params
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(90);

        let result = match timeout(
            Duration::from_secs(timeout_secs),
            crate::cline::tools::execute_cline_tool(cline_tool, &workspace_path_str),
        )
        .await
        {
            Ok(tool_result) => tool_result,
            Err(_) => {
                eprintln!("⏱️ Tool execution timed out after {}s", timeout_secs);
                crate::tools::ToolResult {
                    success: false,
                    output: String::new(),
                    error: Some(format!("Tool execution timed out after {} seconds", timeout_secs)),
                    extra_json: Some(json!({
                        "timeout_seconds": timeout_secs,
                        "status": "timeout"
                    })),
                }
            }
        };
        
        eprintln!("✅ Tool execution returned: success={}, error={:?}", 
            result.success, 
            result.error.as_ref().map(|e| e.as_str()).unwrap_or("None"));
        
        let result_json = serde_json::to_value(&result)
            .map_err(|e| {
                eprintln!("❌ Failed to serialize result: {}", e);
                format!("Failed to serialize result: {}", e)
            })?;
        
        eprintln!("✅ Tool execution completed: success={}", result.success);
        
        Some(result_json)
    } else {
        eprintln!("❌ Tool rejected, not executing");
        None
    };
    
    // Update database with status and result
    {
        let conn = db.get_connection();
        // Don't hang forever waiting for the DB mutex.
        let start = Instant::now();
        let conn_guard = loop {
            match conn.try_lock() {
                Ok(guard) => break guard,
                Err(std::sync::TryLockError::Poisoned(poison)) => {
                    eprintln!("⚠️ Database mutex poisoned during update; continuing");
                    break poison.into_inner();
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    // keep looping
                }
            }

            if start.elapsed() > Duration::from_secs(3) {
                return Err("Database is busy (lock contention) while saving tool result. Try again in a moment.".to_string());
            }

            tokio::time::sleep(Duration::from_millis(25)).await;
        };
        
        if let Some(ref result) = execution_result {
            let executed_at = Utc::now().to_rfc3339();
            let result_json_str = serde_json::to_string(result)
                .map_err(|e| {
                    eprintln!("❌ Failed to serialize result: {}", e);
                    format!("JSON error: {}", e)
                })?;
            
            eprintln!("💾 Updating database with result ({} chars)", result_json_str.len());
            
            conn_guard.execute(
                "UPDATE cline_tool_executions SET approval_status = ?1, result_json = ?2, executed_at = ?3 WHERE id = ?4",
                rusqlite::params![
                    status,
                    result_json_str,
                    executed_at,
                    request.tool_id
                ],
            )
            .map_err(|e| {
                eprintln!("❌ Failed to update tool execution: {}", e);
                format!("Failed to update tool approval: {}", e)
            })?;
            
            eprintln!("✅ Database updated successfully");
        } else {
            eprintln!("💾 Updating database status only (rejected)");
            conn_guard.execute(
                "UPDATE cline_tool_executions SET approval_status = ?1 WHERE id = ?2",
                rusqlite::params![status, request.tool_id],
            )
            .map_err(|e| {
                eprintln!("❌ Failed to update tool approval: {}", e);
                format!("Failed to update tool approval: {}", e)
            })?;
        }
    }
    
    eprintln!("✅ Approve tool function completed successfully");
    
    Ok(json!({ 
        "status": "updated", 
        "tool_id": request.tool_id, 
        "approval_status": status,
        "executed": request.approved,
        "result": execution_result
    }))
}

/// Create a workspace checkpoint
#[tauri::command]
pub async fn cline_create_checkpoint(
    db: State<'_, Database>,
    request: CreateCheckpointRequest,
) -> Result<String, String> {
    // Get workspace path from run
    let workspace_path: String = {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT workspace_path FROM cline_runs WHERE id = ?1",
                [&request.run_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Run not found: {}", e))?
    };
    
    let checkpoint_id = crate::cline::checkpoints::create_checkpoint(
        db.inner(),
        &request.run_id,
        request.step_index,
        &PathBuf::from(workspace_path),
    ).await?;
    
    Ok(checkpoint_id)
}

/// Restore workspace to a checkpoint
#[tauri::command]
pub async fn cline_restore_checkpoint(
    db: State<'_, Database>,
    request: RestoreCheckpointRequest,
) -> Result<(), String> {
    // Get workspace path from checkpoint's run
    let workspace_path: String = {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT cr.workspace_path FROM cline_checkpoints cp
                 JOIN cline_runs cr ON cp.run_id = cr.id
                 WHERE cp.id = ?1",
                [&request.checkpoint_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Checkpoint not found: {}", e))?
    };
    
    crate::cline::checkpoints::restore_checkpoint(
        db.inner(),
        &request.checkpoint_id,
        &PathBuf::from(workspace_path),
    ).await?;
    
    Ok(())
}

/// Compare current workspace with a checkpoint
#[tauri::command]
pub async fn cline_compare_checkpoint(
    db: State<'_, Database>,
    checkpoint_id: String,
) -> Result<Value, String> {
    // Get workspace path from checkpoint's run
    let workspace_path: String = {
        let conn = db.get_connection();
        let conn_guard = conn
            .lock()
            .map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT cr.workspace_path FROM cline_checkpoints cp
                 JOIN cline_runs cr ON cp.run_id = cr.id
                 WHERE cp.id = ?1",
                [&checkpoint_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Checkpoint not found: {}", e))?
    };
    
    let diff = crate::cline::checkpoints::compare_checkpoint(
        db.inner(),
        &checkpoint_id,
        &PathBuf::from(workspace_path),
    ).await?;
    
    Ok(diff)
}

/// Get linter/compiler errors
#[tauri::command]
pub async fn cline_get_errors(
    _workspace_path: String,
) -> Result<Vec<LinterError>, String> {
    // TODO: Implement actual error detection
    // For now, return empty list
    Ok(Vec::new())
}

/// Analyze AST of a file
#[tauri::command]
pub async fn cline_analyze_ast(
    path: String,
) -> Result<Value, String> {
    let result = crate::cline::tools::ast_tool::analyze_ast(&path).await;
    
    if result.success {
        Ok(result.extra_json.unwrap_or(json!({})))
    } else {
        Err(result.error.unwrap_or("AST analysis failed".to_string()))
    }
}

/// Ingest ClineIDE conversation turn into training data
#[tauri::command]
pub async fn ingest_cline_turn(
    db: State<'_, Database>,
    request: IngestClineTurnRequest,
) -> Result<(), String> {
    training_ingest::ingest_cline_turn(
        &db,
        &request.project_id,
        request.local_model_id.as_deref(),
        &request.user_text,
        &request.assistant_text,
        &request.tool_executions,
        request.browser_steps.as_deref(),
        request.error_context.as_deref(),
    )
}
