// RAG (Retrieval-Augmented Generation) module
//
// This module provides a thin abstraction around document chunks stored in the
// local SQLite database. For now, it focuses on:
// - Storing text chunks associated with a project/source
// - Retrieving simple keyword-based context for a question
//
// The design intentionally leaves room to plug in real vector embeddings later.

use crate::db::Database;
use anyhow::Result;
use rusqlite::params;

#[derive(Debug, Clone)]
pub struct RetrievedChunk {
    #[allow(dead_code)]
    pub id: String,
    pub source_id: String,
    pub chunk_index: i32,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct RagContext {
    pub combined_text: String,
    #[allow(dead_code)]
    pub chunks: Vec<RetrievedChunk>,
}

/// Store a document chunk for later retrieval.
/// This is a simple helper that can be called from future ingestion commands.
#[allow(dead_code)]
pub fn insert_document_chunk(
    db: &Database,
    project_id: &str,
    source_id: &str,
    chunk_index: i32,
    text: &str,
    metadata_json: Option<&str>,
) -> Result<()> {
    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;

    conn_guard.execute(
        "INSERT INTO document_chunks (id, project_id, source_id, chunk_index, text, embedding_json, metadata_json, created_at)
         VALUES (lower(hex(randomblob(16))), ?1, ?2, ?3, ?4, NULL, ?5, datetime('now'))",
        params![project_id, source_id, chunk_index, text, metadata_json],
    )?;

    Ok(())
}

/// Very simple retrieval: fetch the most recent N chunks for a project.
/// This is intentionally conservative until a full vector search is wired up.
pub fn retrieve_simple_context_for_project(
    db: &Database,
    project_id: Option<&str>,
    limit: usize,
) -> Result<RagContext> {
    if project_id.is_none() {
        return Ok(RagContext {
            combined_text: String::new(),
            chunks: Vec::new(),
        });
    }

    let project_id = project_id.unwrap();

    let conn = db.get_connection();
    let conn_guard = conn
        .lock()
        .map_err(|e| anyhow::anyhow!("Database lock error: {}", e))?;

    let mut stmt = conn_guard.prepare(
        "SELECT id, source_id, chunk_index, text
         FROM document_chunks
         WHERE project_id = ?1
         ORDER BY created_at DESC, chunk_index ASC
         LIMIT ?2",
    )?;

    let rows = stmt.query_map(params![project_id, limit as i64], |row| {
        Ok(RetrievedChunk {
            id: row.get(0)?,
            source_id: row.get(1)?,
            chunk_index: row.get(2)?,
            text: row.get(3)?,
        })
    })?;

    let mut chunks = Vec::new();
    let mut combined = String::new();

    for row in rows {
        let chunk = row?;
        combined.push_str(&format!(
            "[source:{} chunk:{}]\n{}\n\n",
            chunk.source_id, chunk.chunk_index, chunk.text
        ));
        chunks.push(chunk);
    }

    Ok(RagContext {
        combined_text: combined,
        chunks,
    })
}

