// RAG-related commands for fetching citations and groundedness scores

use crate::db::Database;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct Citation {
    pub id: String,
    pub run_result_id: String,
    pub source_id: String,
    pub chunk_index: i32,
    pub citation_text: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GroundednessScore {
    pub id: String,
    pub run_result_id: String,
    pub score: f64,
    pub is_grounded: bool,
    pub ungrounded_claims: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub async fn get_citations_for_result(
    db: State<'_, Database>,
    run_result_id: String,
) -> Result<Vec<Citation>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Schema: id, run_result_id, source_id, chunk_id, raw_citation_text, created_at
    let mut stmt = conn_guard
        .prepare("SELECT id, run_result_id, source_id, chunk_id, raw_citation_text, created_at FROM citations WHERE run_result_id = ?1 ORDER BY created_at")
        .map_err(|e| format!("Database error: {}", e))?;
    
    let rows = stmt
        .query_map([&run_result_id], |row| {
            let chunk_id: Option<String> = row.get(3)?;
            let chunk_index = chunk_id
                .as_ref()
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(0);
            Ok(Citation {
                id: row.get(0)?,
                run_result_id: row.get(1)?,
                source_id: row.get(2)?,
                chunk_index,
                citation_text: row.get(4)?,
                created_at: row.get(5)?,
            })
        })
        .map_err(|e| format!("Database error: {}", e))?;
    
    let mut citations = Vec::new();
    for row in rows {
        citations.push(row.map_err(|e| format!("Row error: {}", e))?);
    }
    
    Ok(citations)
}

#[tauri::command]
pub async fn get_groundedness_for_result(
    db: State<'_, Database>,
    run_result_id: String,
) -> Result<Option<GroundednessScore>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // Schema: id, run_result_id, score, details_json, created_at
    let result: Result<Option<GroundednessScore>, _> = conn_guard.query_row(
        "SELECT id, run_result_id, score, details_json, created_at FROM groundedness_scores WHERE run_result_id = ?1 ORDER BY created_at DESC LIMIT 1",
        [&run_result_id],
        |row| {
            let score: f64 = row.get(2)?;
            let details_json_str: Option<String> = row.get(3)?;
            let (is_grounded, ungrounded_claims) = details_json_str
                .as_ref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                .map(|j| {
                    let is_g = j.get("is_grounded").and_then(|v| v.as_bool()).unwrap_or(score >= 0.7);
                    let ung = j.get("ungrounded_claims").and_then(|v| v.as_str()).map(String::from);
                    (is_g, ung)
                })
                .unwrap_or((score >= 0.7, None));
            Ok(Some(GroundednessScore {
                id: row.get(0)?,
                run_result_id: row.get(1)?,
                score,
                is_grounded,
                ungrounded_claims,
                created_at: row.get(4)?,
            }))
        },
    );
    
    match result {
        Ok(score) => Ok(score),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Database error: {}", e)),
    }
}

#[tauri::command]
pub async fn get_document_chunk(
    db: State<'_, Database>,
    source_id: String,
    chunk_index: i32,
) -> Result<Option<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let result: Result<Option<serde_json::Value>, _> = conn_guard.query_row(
        "SELECT id, project_id, source_id, chunk_index, chunk_text, metadata_json, created_at FROM document_chunks WHERE source_id = ?1 AND chunk_index = ?2",
        rusqlite::params![&source_id, chunk_index],
        |row| {
            let metadata_json_str: Option<String> = row.get(5)?;
            let metadata_json: Option<serde_json::Value> = metadata_json_str.and_then(|s| serde_json::from_str(&s).ok());
            Ok(Some(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "project_id": row.get::<_, String>(1)?,
                "source_id": row.get::<_, String>(2)?,
                "chunk_index": row.get::<_, i32>(3)?,
                "chunk_text": row.get::<_, String>(4)?,
                "metadata_json": metadata_json,
                "created_at": row.get::<_, String>(6)?,
            })))
        },
    );
    
    match result {
        Ok(chunk) => Ok(chunk),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Database error: {}", e)),
    }
}
