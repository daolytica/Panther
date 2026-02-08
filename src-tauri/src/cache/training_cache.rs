// Training data cache implementation

use crate::db::Database;
use crate::cache::get_file_size;
use crate::commands_settings::load_settings_sync;
use rusqlite::params;
use std::path::PathBuf;
use std::fs;
use std::io::{BufWriter, Write};
use flate2::Compression;
use flate2::write::GzEncoder;
use uuid::Uuid;
use chrono::Utc;

pub struct TrainingCache {
    db: Database,
}

impl TrainingCache {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    /// Check if cache exists for given project, model, and data hash
    pub fn get_cached_file(
        &self,
        project_id: &str,
        model_id: &str,
        data_hash: &str,
    ) -> Result<Option<PathBuf>, String> {
        let conn = self.db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

        let result: Option<String> = match conn_guard
            .query_row(
                "SELECT file_path FROM training_data_cache 
                 WHERE project_id = ?1 AND model_id = ?2 AND data_hash = ?3",
                params![project_id, model_id, data_hash],
                |row| Ok(row.get::<_, String>(0)?),
            ) {
                Ok(path) => Some(path),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(format!("Database error: {}", e)),
            };

        if let Some(file_path) = result {
            let path = PathBuf::from(&file_path);
            // Verify file still exists
            if path.exists() {
                // Update access time and count
                let now = Utc::now().to_rfc3339();
                conn_guard.execute(
                    "UPDATE training_data_cache 
                     SET last_accessed_at = ?1, access_count = access_count + 1 
                     WHERE project_id = ?2 AND model_id = ?3 AND data_hash = ?4",
                    params![now, project_id, model_id, data_hash],
                )
                .map_err(|e| format!("Failed to update cache access: {}", e))?;
                
                Ok(Some(path))
            } else {
                // File doesn't exist, remove from cache
                self.remove_cache_entry(project_id, model_id, data_hash)?;
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }

    /// Store cache entry in database
    pub fn store_cache_entry(
        &self,
        project_id: &str,
        model_id: &str,
        data_hash: &str,
        file_path: &PathBuf,
    ) -> Result<(), String> {
        let file_size = get_file_size(file_path)?;
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let file_path_str = file_path.to_string_lossy().to_string();

        let conn = self.db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

        // Check if entry already exists
        let exists: bool = conn_guard
            .query_row(
                "SELECT COUNT(*) FROM training_data_cache 
                 WHERE project_id = ?1 AND model_id = ?2 AND data_hash = ?3",
                params![project_id, model_id, data_hash],
                |row| Ok(row.get::<_, i32>(0)? > 0),
            )
            .unwrap_or(false);

        if exists {
            // Update existing entry
            conn_guard.execute(
                "UPDATE training_data_cache 
                 SET file_path = ?1, file_size = ?2, last_accessed_at = ?3 
                 WHERE project_id = ?4 AND model_id = ?5 AND data_hash = ?6",
                params![file_path_str, file_size as i64, now, project_id, model_id, data_hash],
            )
            .map_err(|e| format!("Database error: {}", e))?;
        } else {
            // Insert new entry
            conn_guard.execute(
                "INSERT INTO training_data_cache 
                 (id, project_id, model_id, data_hash, file_path, file_size, created_at, last_accessed_at, access_count) 
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 1)",
                params![id, project_id, model_id, data_hash, file_path_str, file_size as i64, now, now],
            )
            .map_err(|e| format!("Database error: {}", e))?;
        }

        // Check cache size and evict if necessary
        self.evict_if_needed()?;

        Ok(())
    }

    /// Remove cache entry
    pub fn remove_cache_entry(
        &self,
        project_id: &str,
        model_id: &str,
        data_hash: &str,
    ) -> Result<(), String> {
        let conn = self.db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

        // Get file path before deleting
        let file_path: Option<String> = match conn_guard
            .query_row(
                "SELECT file_path FROM training_data_cache 
                 WHERE project_id = ?1 AND model_id = ?2 AND data_hash = ?3",
                params![project_id, model_id, data_hash],
                |row| Ok(row.get::<_, String>(0)?),
            ) {
                Ok(path) => Some(path),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(format!("Database error: {}", e)),
            };

        // Delete from database
        conn_guard.execute(
            "DELETE FROM training_data_cache 
             WHERE project_id = ?1 AND model_id = ?2 AND data_hash = ?3",
            params![project_id, model_id, data_hash],
        )
        .map_err(|e| format!("Database error: {}", e))?;

        // Delete file if it exists
        if let Some(path_str) = file_path {
            let path = PathBuf::from(path_str);
            if path.exists() {
                fs::remove_file(&path)
                    .map_err(|e| format!("Failed to remove cache file: {}", e))?;
            }
        }

        Ok(())
    }

    /// Invalidate all cache entries for a project/model
    pub fn invalidate_cache(
        &self,
        project_id: &str,
        model_id: Option<&str>,
    ) -> Result<(), String> {
        let conn = self.db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

        let query = if let Some(_mid) = model_id {
            "SELECT file_path FROM training_data_cache WHERE project_id = ?1 AND model_id = ?2"
        } else {
            "SELECT file_path FROM training_data_cache WHERE project_id = ?1"
        };

        // Get all file paths
        let mut stmt = conn_guard
            .prepare(query)
            .map_err(|e| format!("Database error: {}", e))?;

        let file_paths: Vec<String> = if let Some(mid) = model_id {
            stmt.query_map(params![project_id, mid], |row| Ok(row.get(0)?))
                .map_err(|e| format!("Database error: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Row error: {}", e))?
        } else {
            stmt.query_map(params![project_id], |row| Ok(row.get(0)?))
                .map_err(|e| format!("Database error: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Row error: {}", e))?
        };

        // Delete files
        for path_str in &file_paths {
            let path = PathBuf::from(path_str);
            if path.exists() {
                fs::remove_file(&path).ok(); // Ignore errors
            }
        }

        // Delete from database
        if let Some(mid) = model_id {
            conn_guard.execute(
                "DELETE FROM training_data_cache WHERE project_id = ?1 AND model_id = ?2",
                params![project_id, mid],
            )
            .map_err(|e| format!("Database error: {}", e))?;
        } else {
            conn_guard.execute(
                "DELETE FROM training_data_cache WHERE project_id = ?1",
                params![project_id],
            )
            .map_err(|e| format!("Database error: {}", e))?;
        }

        Ok(())
    }

    /// Evict old cache entries if total size exceeds limit (LRU)
    fn evict_if_needed(&self) -> Result<(), String> {
        let settings = load_settings_sync(&self.db);
        
        let max_size_bytes = settings.cache.max_size_gb * 1024 * 1024 * 1024;
        let eviction_threshold = (max_size_bytes * settings.cache.eviction_threshold_percent / 100) as u64;
        
        let conn = self.db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

        // Get total cache size
        let total_size: i64 = conn_guard
            .query_row(
                "SELECT COALESCE(SUM(file_size), 0) FROM training_data_cache",
                [],
                |row| Ok(row.get(0)?),
            )
            .map_err(|e| format!("Database error: {}", e))?;

        if total_size as u64 > max_size_bytes {
            // Get oldest entries sorted by last_accessed_at
            let mut stmt = conn_guard
                .prepare(
                    "SELECT id, file_path FROM training_data_cache 
                     ORDER BY last_accessed_at ASC, access_count ASC",
                )
                .map_err(|e| format!("Database error: {}", e))?;

            let entries: Vec<(String, String)> = stmt
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|e| format!("Database error: {}", e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("Row error: {}", e))?;

            let mut current_size = total_size as u64;
            let target_size = eviction_threshold;

            for (id, file_path) in entries {
                if current_size <= target_size {
                    break;
                }

                // Get file size
                let file_size: i64 = conn_guard
                    .query_row(
                        "SELECT file_size FROM training_data_cache WHERE id = ?1",
                        params![id],
                        |row| Ok(row.get(0)?),
                    )
                    .unwrap_or(0);

                // Delete file
                let path = PathBuf::from(&file_path);
                if path.exists() {
                    fs::remove_file(&path).ok();
                }

                // Delete from database
                conn_guard.execute(
                    "DELETE FROM training_data_cache WHERE id = ?1",
                    params![id],
                )
                .ok();

                current_size = current_size.saturating_sub(file_size as u64);
            }
        }

        Ok(())
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> Result<CacheStats, String> {
        let settings = load_settings_sync(&self.db);
        
        let max_size_bytes = settings.cache.max_size_gb * 1024 * 1024 * 1024;
        
        let conn = self.db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;

        let (total_entries, total_size): (i64, i64) = conn_guard
            .query_row(
                "SELECT COUNT(*), COALESCE(SUM(file_size), 0) FROM training_data_cache",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| format!("Database error: {}", e))?;

        Ok(CacheStats {
            total_entries: total_entries as u64,
            total_size_bytes: total_size as u64,
            max_size_bytes,
        })
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub max_size_bytes: u64,
}

/// Write training data to JSONL file with optional compression
#[allow(dead_code)]
pub fn write_training_data_to_file(
    data: impl Iterator<Item = (String, String)>,
    file_path: &PathBuf,
    compress: bool,
) -> Result<(), String> {
    if compress {
        let file = fs::File::create(file_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
        let encoder = GzEncoder::new(file, Compression::default());
        let mut writer = BufWriter::new(encoder);

        for (input, output) in data {
            let json_line = serde_json::json!({
                "input": input,
                "output": output
            });
            writeln!(writer, "{}", json_line)
                .map_err(|e| format!("Failed to write: {}", e))?;
        }

        writer.flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;
    } else {
        let file = fs::File::create(file_path)
            .map_err(|e| format!("Failed to create file: {}", e))?;
        let mut writer = BufWriter::new(file);

        for (input, output) in data {
            let json_line = serde_json::json!({
                "input": input,
                "output": output
            });
            writeln!(writer, "{}", json_line)
                .map_err(|e| format!("Failed to write: {}", e))?;
        }

        writer.flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;
    }

    Ok(())
}
