// Debate and export commands

use crate::db::Database;
use crate::debate_orchestrator::DebateOrchestrator;
use serde_json;
use tauri::State;

// Panic guard to log panics in spawned tasks
struct PanicGuard<'a> {
    run_id: &'a str,
}

impl<'a> Drop for PanicGuard<'a> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("[Debate] PANIC in background task for run_id={}", self.run_id);
        }
    }
}

// Set up panic hook for debugging
fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.to_string();
        eprintln!("[Panic] {}", msg);
        // Also print backtrace if available
        if let Some(location) = info.location() {
            eprintln!("[Panic] occurred at {}:{}", location.file(), location.line());
        }
    }));
}

#[tauri::command]
pub async fn start_debate(
    db: State<'_, Database>,
    run_id: String,
    rounds: i32,
    speaking_order: Vec<String>,
    max_words: Option<i32>,
    language: Option<String>,
    tone: Option<String>,
    web_search_results: Option<Vec<crate::web_search::NewsResult>>,
) -> Result<(), String> {
    eprintln!("[Debate] start_debate called: run_id={}, rounds={}, speaking_order.len()={}", run_id, rounds, speaking_order.len());
    
    // Immediately update status to running so UI updates
    {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        conn_guard.execute(
            "UPDATE runs SET status = 'running' WHERE id = ?1",
            rusqlite::params![run_id],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    }
    
    if speaking_order.is_empty() {
        return Err("speaking_order is empty - no profiles selected".to_string());
    }
    
    let db_clone = db.inner().clone();
    let run_id_clone = run_id.clone();
    let mut orchestrator = DebateOrchestrator::new(db_clone.clone());
    
    // Run in background with panic handling
    tokio::spawn(async move {
        let _guard = PanicGuard { run_id: &run_id_clone };
        
        eprintln!("[Debate] Background task starting for run_id={}", run_id_clone);
        
        // Add a small delay to let the UI update first
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        
        let result = orchestrator.run_debate(run_id_clone.clone(), rounds, speaking_order, max_words, language, tone, web_search_results).await;
        if let Err(e) = result {
            let error_msg = format!("{}", e);
            eprintln!("[Debate] Background task failed: {}", error_msg);
            orchestrator.handle_error(&run_id_clone, &error_msg).await;
        } else {
            eprintln!("[Debate] Background task completed successfully for run_id={}", run_id_clone);
        }
        
        drop(_guard);
    });
    
    // Give a small delay to ensure the task starts
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    
    Ok(())
}

#[tauri::command]
pub async fn get_debate_messages(
    db: State<'_, Database>,
    run_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Orchestrator writes usage to provider_metadata_json
    let mut stmt = conn_guard
        .prepare("SELECT id, author_type, profile_id, round_index, turn_index, text, created_at, provider_metadata_json FROM messages WHERE run_id = ?1 ORDER BY round_index, turn_index, created_at")
        .map_err(|e| format!("Database error: {}", e))?;
    
    let rows = stmt
        .query_map([&run_id], |row| {
            let usage_json_str: Option<String> = row.get(7)?;
            let usage: Option<serde_json::Value> = usage_json_str.and_then(|s| serde_json::from_str(&s).ok());
            
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "author_type": row.get::<_, String>(1)?,
                "profile_id": row.get::<_, Option<String>>(2)?,
                "round_index": row.get::<_, Option<i32>>(3)?,
                "turn_index": row.get::<_, Option<i32>>(4)?,
                "text": row.get::<_, String>(5)?,
                "created_at": row.get::<_, String>(6)?,
                "usage": usage,
            }))
        })
        .map_err(|e| format!("Database error: {}", e))?;
    
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(|e| format!("Row error: {}", e))?);
    }
    
    Ok(messages)
}

#[tauri::command]
pub async fn pause_debate(
    db: State<'_, Database>,
    run_id: String,
) -> Result<(), String> {
    // Mark debate as paused in database
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    conn_guard.execute(
        "UPDATE runs SET status = 'paused' WHERE id = ?1",
        rusqlite::params![run_id],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn resume_debate(
    db: State<'_, Database>,
    run_id: String,
) -> Result<(), String> {
    // Mark debate as running
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    conn_guard.execute(
        "UPDATE runs SET status = 'running' WHERE id = ?1",
        rusqlite::params![run_id],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn cancel_debate(
    db: State<'_, Database>,
    run_id: String,
) -> Result<(), String> {
    // Mark debate as cancelled
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
pub async fn delete_debate_message(
    db: State<'_, Database>,
    message_id: String,
) -> Result<(), String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    conn_guard.execute(
        "DELETE FROM messages WHERE id = ?1",
        rusqlite::params![message_id],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn add_user_message(
    db: State<'_, Database>,
    run_id: String,
    text: String,
    insert_after_message_id: Option<String>,
) -> Result<String, String> {
    let message_id = uuid::Uuid::new_v4().to_string();
    let created_at = chrono::Utc::now().to_rfc3339();
    
    // Determine round_index and turn_index
    let (round_index, turn_index) = if let Some(after_id) = insert_after_message_id {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        let result: Result<(Option<i32>, Option<i32>), _> = conn_guard.query_row(
            "SELECT round_index, turn_index FROM messages WHERE id = ?1",
            [&after_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        
        if let Ok((Some(r), Some(t))) = result {
            (Some(r), Some(t + 1)) // Insert after this turn
        } else {
            // Get max round/turn
            let max_result: Result<(Option<i32>, Option<i32>), _> = conn_guard.query_row(
                "SELECT MAX(round_index), MAX(turn_index) FROM messages WHERE run_id = ?1",
                [&run_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );
            if let Ok((Some(r), Some(t))) = max_result {
                (Some(r), Some(t + 1))
            } else {
                (Some(0), Some(0))
            }
        }
    } else {
        // Append to end
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        let max_result: Result<(Option<i32>, Option<i32>), _> = conn_guard.query_row(
            "SELECT MAX(round_index), MAX(turn_index) FROM messages WHERE run_id = ?1",
            [&run_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );
        if let Ok((Some(r), Some(t))) = max_result {
            (Some(r), Some(t + 1))
        } else {
            (Some(0), Some(0))
        }
    };
    
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    conn_guard.execute(
        "INSERT INTO messages (id, run_id, author_type, profile_id, round_index, turn_index, text, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            message_id,
            run_id,
            "user",
            None::<String>,
            round_index,
            turn_index,
            text,
            created_at
        ],
    )
    .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(message_id)
}

#[tauri::command]
pub   async fn continue_debate(
    db: State<'_, Database>,
    run_id: String,
    rounds: i32,
) -> Result<(), String> {
    // Get current debate config and continue
    let (speaking_order_json, max_words, language, tone): (String, Option<i32>, Option<String>, Option<String>) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        let result: Result<(String, Option<i32>, Option<String>, Option<String>), _> = conn_guard.query_row(
            "SELECT speaking_order_json, max_words, language, tone FROM debate_configs WHERE run_id = ?1",
            [&run_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        );
        result.map_err(|e| format!("Failed to get debate config: {}", e))?
    };
    
    let speaking_order: Vec<String> = serde_json::from_str(&speaking_order_json)
        .map_err(|e| format!("Failed to parse speaking order: {}", e))?;
    
    // Mark as running and restart debate
    {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        conn_guard.execute(
            "UPDATE runs SET status = 'running' WHERE id = ?1",
            rusqlite::params![run_id],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    }
    
    // Start new debate continuation
    let db_clone = db.inner().clone();
    let mut orchestrator = DebateOrchestrator::new(db_clone);
    let language_clone = language.clone();
    let tone_clone = tone.clone();
    
    tokio::spawn(async move {
        if let Err(e) = orchestrator.run_debate(run_id, rounds, speaking_order, max_words, language_clone, tone_clone, None).await {
            eprintln!("Debate continuation error: {}", e);
        }
    });
    
    Ok(())
}

#[tauri::command]
pub async fn export_session_markdown(
    db: State<'_, Database>,
    session_id: String,
) -> Result<String, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Get session info
    let session_data: Result<(String, String, String), _> = conn_guard.query_row(
        "SELECT title, user_question, mode FROM sessions WHERE id = ?1",
        [&session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    );
    
    let (title, user_question, mode) = session_data
        .map_err(|e| format!("Failed to load session: {}", e))?;
    
    // Get run
    let run_data: Result<(String, String), _> = conn_guard.query_row(
        "SELECT id, status FROM runs WHERE session_id = ?1 ORDER BY started_at DESC LIMIT 1",
        [&session_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    );
    
    let (run_id, run_status) = run_data
        .map_err(|e| format!("Failed to load run: {}", e))?;
    
    let mut markdown = String::new();
    markdown.push_str(&format!("# {}\n\n", title));
    markdown.push_str(&format!("**Mode:** {}\n", mode));
    markdown.push_str(&format!("**Status:** {}\n\n", run_status));
    markdown.push_str(&format!("## Question\n\n{}\n\n", user_question));
    
    if mode == "parallel" {
        // Export parallel results
        let mut stmt = conn_guard
            .prepare("SELECT profile_id, raw_output_text, error_message_safe FROM run_results WHERE run_id = ?1 ORDER BY started_at")
            .map_err(|e| format!("Database error: {}", e))?;
        
        let rows = stmt
            .query_map([&run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|e| format!("Database error: {}", e))?;
        
        markdown.push_str("## Agent Responses\n\n");
        for row in rows {
            let (profile_id, output, error) = row.map_err(|e| format!("Row error: {}", e))?;
            markdown.push_str(&format!("### Agent: {}\n\n", profile_id));
            if let Some(text) = output {
                markdown.push_str(&format!("{}\n\n", text));
            } else if let Some(err) = error {
                markdown.push_str(&format!("**Error:** {}\n\n", err));
            }
        }
    } else {
        // Export debate messages
        let mut stmt = conn_guard
            .prepare("SELECT author_type, profile_id, round_index, turn_index, text FROM messages WHERE run_id = ?1 ORDER BY round_index, turn_index, created_at")
            .map_err(|e| format!("Database error: {}", e))?;
        
        let rows = stmt
            .query_map([&run_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<i32>>(2)?,
                    row.get::<_, Option<i32>>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })
            .map_err(|e| format!("Database error: {}", e))?;
        
        markdown.push_str("## Debate Transcript\n\n");
        let mut current_round = -1;
        for row in rows {
            let (author_type, profile_id, round_index, _turn_index, text) = row.map_err(|e| format!("Row error: {}", e))?;
            
            if let Some(round) = round_index {
                if round != current_round {
                    markdown.push_str(&format!("### Round {}\n\n", round));
                    current_round = round;
                }
            }
            
            if author_type == "agent" {
                markdown.push_str(&format!("**Agent {}:** {}\n\n", profile_id.unwrap_or_default(), text));
            } else {
                markdown.push_str(&format!("**{}:** {}\n\n", author_type, text));
            }
        }
    }
    
    Ok(markdown)
}

#[tauri::command]
pub async fn export_session_json(
    db: State<'_, Database>,
    session_id: String,
) -> Result<serde_json::Value, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Get session info
    let session_data: Result<(String, String, String, String), _> = conn_guard.query_row(
        "SELECT id, title, user_question, mode FROM sessions WHERE id = ?1",
        [&session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    );
    
    let (id, title, user_question, mode) = session_data
        .map_err(|e| format!("Failed to load session: {}", e))?;
    
    // Get run
    let run_data: Result<(String, String, Option<String>, Option<String>), _> = conn_guard.query_row(
        "SELECT id, status, started_at, finished_at FROM runs WHERE session_id = ?1 ORDER BY started_at DESC LIMIT 1",
        [&session_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    );
    
    let (run_id, run_status, started_at, finished_at) = run_data
        .map_err(|e| format!("Failed to load run: {}", e))?;
    
    let mut export_data = serde_json::json!({
        "session": {
            "id": id,
            "title": title,
            "user_question": user_question,
            "mode": mode,
        },
        "run": {
            "id": run_id,
            "status": run_status,
            "started_at": started_at,
            "finished_at": finished_at,
        },
    });
    
    if mode == "parallel" {
        // Export parallel results
        let mut stmt = conn_guard
            .prepare("SELECT profile_id, status, raw_output_text, error_message_safe, started_at, finished_at FROM run_results WHERE run_id = ?1 ORDER BY started_at")
            .map_err(|e| format!("Database error: {}", e))?;
        
        let rows = stmt
            .query_map([&run_id], |row| {
                Ok(serde_json::json!({
                    "profile_id": row.get::<_, String>(0)?,
                    "status": row.get::<_, String>(1)?,
                    "raw_output_text": row.get::<_, Option<String>>(2)?,
                    "error_message_safe": row.get::<_, Option<String>>(3)?,
                    "started_at": row.get::<_, String>(4)?,
                    "finished_at": row.get::<_, Option<String>>(5)?,
                }))
            })
            .map_err(|e| format!("Database error: {}", e))?;
        
        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        export_data["results"] = serde_json::json!(results);
    } else {
        // Export debate messages
        let mut stmt = conn_guard
            .prepare("SELECT id, author_type, profile_id, round_index, turn_index, text, created_at FROM messages WHERE run_id = ?1 ORDER BY round_index, turn_index, created_at")
            .map_err(|e| format!("Database error: {}", e))?;
        
        let rows = stmt
            .query_map([&run_id], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "author_type": row.get::<_, String>(1)?,
                    "profile_id": row.get::<_, Option<String>>(2)?,
                    "round_index": row.get::<_, Option<i32>>(3)?,
                    "turn_index": row.get::<_, Option<i32>>(4)?,
                    "text": row.get::<_, String>(5)?,
                    "created_at": row.get::<_, String>(6)?,
                }))
            })
            .map_err(|e| format!("Database error: {}", e))?;
        
        let mut messages = Vec::new();
        for row in rows {
            messages.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
        export_data["messages"] = serde_json::json!(messages);
    }
    
    Ok(export_data)
}
