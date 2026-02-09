// Workspace checkpoint system for Cline

use crate::db::Database;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use uuid::Uuid;
use chrono::Utc;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Checkpoint {
    pub id: String,
    pub run_id: String,
    pub step_index: i32,
    pub snapshot_json: Value,
    pub created_at: String,
}

/// Create a workspace checkpoint
pub async fn create_checkpoint(
    db: &Database,
    run_id: &str,
    step_index: i32,
    workspace_path: &Path,
) -> Result<String, String> {
    // Snapshot all files in workspace
    let snapshot = snapshot_workspace(workspace_path).await?;
    
    let checkpoint_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    conn_guard.execute(
        "INSERT INTO cline_checkpoints (id, run_id, step_index, snapshot_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            checkpoint_id,
            run_id,
            step_index,
            serde_json::to_string(&snapshot).map_err(|e| format!("JSON error: {}", e))?,
            now
        ],
    )
    .map_err(|e| format!("Failed to create checkpoint: {}", e))?;
    
    Ok(checkpoint_id)
}

/// Restore workspace to a checkpoint
pub async fn restore_checkpoint(
    db: &Database,
    checkpoint_id: &str,
    workspace_path: &Path,
) -> Result<(), String> {
    let snapshot_json: String = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT snapshot_json FROM cline_checkpoints WHERE id = ?1",
                [checkpoint_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Checkpoint not found: {}", e))?
    };
    
    let snapshot: Value = serde_json::from_str(&snapshot_json)
        .map_err(|e| format!("Failed to parse snapshot: {}", e))?;
    
    // Restore files from snapshot
    restore_workspace_from_snapshot(workspace_path, &snapshot).await?;
    
    Ok(())
}

/// Compare current workspace with a checkpoint
pub async fn compare_checkpoint(
    db: &Database,
    checkpoint_id: &str,
    workspace_path: &Path,
) -> Result<Value, String> {
    let snapshot_json: String = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT snapshot_json FROM cline_checkpoints WHERE id = ?1",
                [checkpoint_id],
                |row| row.get(0),
            )
            .map_err(|e| format!("Checkpoint not found: {}", e))?
    };
    
    let checkpoint_snapshot: Value = serde_json::from_str(&snapshot_json)
        .map_err(|e| format!("Failed to parse snapshot: {}", e))?;
    
    let current_snapshot = snapshot_workspace(workspace_path).await?;
    
    // Generate diff
    let diff = generate_diff(&checkpoint_snapshot, &current_snapshot);
    
    Ok(diff)
}

async fn snapshot_workspace(workspace_path: &Path) -> Result<Value, String> {
    use crate::commands_workspace;
    
    eprintln!("📸 Starting workspace snapshot for: {:?}", workspace_path);
    
    // Limit snapshot to avoid hanging on large directories
    // Only snapshot top-level files and directories, not recursive
    let max_files = 1000; // Limit to prevent hanging
    let mut files = Vec::new();
    let mut file_count = 0;
    
    // Get top-level files only (not recursive to avoid hanging on large dirs like C:\Users\mirfa)
    match commands_workspace::list_workspace_files(
        workspace_path.to_str().map(|s| s.to_string())
    ).await {
        Ok(entries) => {
            eprintln!("📁 Found {} entries in workspace", entries.len());
            for entry in entries {
                if file_count >= max_files {
                    eprintln!("⚠️ Reached file limit ({}), stopping snapshot", max_files);
                    break;
                }
                
                if !entry.is_dir {
                    // Only snapshot files, not directories (to avoid recursive traversal)
                    match commands_workspace::read_workspace_file(entry.path.clone()).await {
                        Ok(content) => {
                            // Use content hash for comparison (simple approach)
                            let mut hasher = DefaultHasher::new();
                            content.hash(&mut hasher);
                            let hash = hasher.finish();
                            
                            files.push(json!({
                                "path": entry.path,
                                "content": content,
                                "hash": format!("{:x}", hash)
                            }));
                            file_count += 1;
                        }
                        Err(_) => {
                            // Skip files that can't be read
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("⚠️ Failed to list workspace files: {}", e);
            return Err(format!("Failed to list files: {}", e));
        }
    }
    
    eprintln!("✅ Snapshot complete: {} files", files.len());
    
    Ok(json!({
        "files": files,
        "timestamp": Utc::now().to_rfc3339(),
        "limited": file_count >= max_files
    }))
}

async fn restore_workspace_from_snapshot(_workspace_path: &Path, snapshot: &Value) -> Result<(), String> {
    use crate::commands_workspace;
    
    let files = snapshot.get("files")
        .and_then(|f| f.as_array())
        .ok_or("Invalid snapshot format")?;
    
    for file in files {
        let path = file.get("path")
            .and_then(|p| p.as_str())
            .ok_or("Invalid file path in snapshot")?;
        let content = file.get("content")
            .and_then(|c| c.as_str())
            .ok_or("Invalid file content in snapshot")?;
        
        // Call the Tauri command function directly (it's async and returns Result)
        commands_workspace::write_workspace_file(path.to_string(), content.to_string()).await
            .map_err(|e| format!("Failed to restore file {}: {}", path, e))?;
    }
    
    Ok(())
}

fn generate_diff(checkpoint: &Value, current: &Value) -> Value {
    let empty_vec: Vec<serde_json::Value> = Vec::new();
    let checkpoint_files = checkpoint.get("files")
        .and_then(|f| f.as_array())
        .unwrap_or(&empty_vec);
    let current_files = current.get("files")
        .and_then(|f| f.as_array())
        .unwrap_or(&empty_vec);
    
    let mut added = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();
    
    // Create maps for easier lookup
    let checkpoint_map: std::collections::HashMap<&str, &Value> = checkpoint_files
        .iter()
        .filter_map(|f| f.get("path").and_then(|p| p.as_str()).map(|p| (p, f)))
        .collect();
    
    let current_map: std::collections::HashMap<&str, &Value> = current_files
        .iter()
        .filter_map(|f| f.get("path").and_then(|p| p.as_str()).map(|p| (p, f)))
        .collect();
    
    // Find added and modified files
    for file in current_files {
        if let Some(path) = file.get("path").and_then(|p| p.as_str()) {
            if let Some(checkpoint_file) = checkpoint_map.get(path) {
                let checkpoint_hash = checkpoint_file.get("hash").and_then(|h| h.as_str());
                let current_hash = file.get("hash").and_then(|h| h.as_str());
                if checkpoint_hash != current_hash {
                    modified.push(path);
                }
            } else {
                added.push(path);
            }
        }
    }
    
    // Find deleted files
    for file in checkpoint_files {
        if let Some(path) = file.get("path").and_then(|p| p.as_str()) {
            if !current_map.contains_key(path) {
                deleted.push(path);
            }
        }
    }
    
    json!({
        "added": added,
        "modified": modified,
        "deleted": deleted
    })
}
