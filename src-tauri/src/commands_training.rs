// Training and local model commands

use crate::db::Database;
use crate::types::{ProviderAccount, PromptPacket};
use crate::cache::{self, training_cache::TrainingCache};
use crate::token_usage::record_token_usage;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;
use chrono::Utc;
use serde_json;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::io::{BufWriter, Write};
use sha2::{Sha256, Digest};

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateLocalModelRequest {
    pub project_id: String,
    /// Optional; when absent, uses base_model as the display name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub base_model: String,
    pub training_config_json: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateTrainingDataRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub input_text: String,
    pub output_text: String,
    pub metadata_json: Option<serde_json::Value>,
}

// ===============================================
// LoRA/QLoRA Training Configuration
// ===============================================

/// LoRA-specific configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraTrainingConfig {
    /// Enable LoRA training
    pub use_lora: bool,
    /// Enable QLoRA (4-bit quantization)
    pub use_qlora: bool,
    /// LoRA rank (dimension of low-rank matrices)
    pub lora_rank: u16,
    /// LoRA alpha (scaling factor)
    pub lora_alpha: u16,
    /// LoRA dropout probability
    pub lora_dropout: f32,
    /// Target modules for LoRA adaptation (e.g., ["q_proj", "v_proj", "k_proj", "o_proj"])
    pub target_modules: Vec<String>,
    /// Bias training mode: "none", "all", or "lora_only"
    pub bias: String,
    /// Task type: "CAUSAL_LM", "SEQ_2_SEQ_LM", etc.
    pub task_type: String,
}

impl Default for LoraTrainingConfig {
    fn default() -> Self {
        Self {
            use_lora: true,
            use_qlora: false,
            lora_rank: 16,
            lora_alpha: 32,
            lora_dropout: 0.05,
            target_modules: vec![
                "q_proj".to_string(),
                "v_proj".to_string(),
                "k_proj".to_string(),
                "o_proj".to_string(),
            ],
            bias: "none".to_string(),
            task_type: "CAUSAL_LM".to_string(),
        }
    }
}

/// Full training configuration for fine-tuning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingConfig {
    /// Number of training epochs
    pub num_train_epochs: f32,
    /// Learning rate
    pub learning_rate: f64,
    /// Batch size per device
    pub per_device_train_batch_size: u16,
    /// Gradient accumulation steps
    pub gradient_accumulation_steps: u16,
    /// Warmup ratio (proportion of total steps)
    pub warmup_ratio: f32,
    /// Weight decay for regularization
    pub weight_decay: f32,
    /// Maximum sequence length
    pub max_seq_length: u32,
    /// Enable FP16 training
    pub fp16: bool,
    /// Enable BF16 training (better precision than FP16)
    pub bf16: bool,
    /// Save checkpoint every N steps
    pub save_steps: u32,
    /// Logging every N steps
    pub logging_steps: u32,
    /// Maximum number of checkpoints to keep
    pub save_total_limit: u16,
    /// LoRA-specific configuration
    pub lora_config: LoraTrainingConfig,
    /// Maximum training samples (safety cap)
    pub max_train_samples: Option<u32>,
    /// Maximum tokens per sample (safety cap)
    pub max_tokens_per_sample: Option<u32>,
    /// Enable 8-bit Adam optimizer (memory efficient)
    pub use_8bit_adam: bool,
    /// Gradient checkpointing (memory efficient but slower)
    pub gradient_checkpointing: bool,
    /// Dry run - only estimate memory, don't train
    pub dry_run: bool,
    /// Dataset format: "alpaca", "sharegpt", "completion", "custom"
    pub dataset_format: String,
    /// Custom prompt template (if dataset_format is "custom")
    pub prompt_template: Option<String>,
    /// Evaluation split ratio (0.0 = no eval set)
    pub eval_split_ratio: f32,
}

impl Default for TrainingConfig {
    fn default() -> Self {
        Self {
            num_train_epochs: 3.0,
            learning_rate: 2e-4,
            per_device_train_batch_size: 4,
            gradient_accumulation_steps: 4,
            warmup_ratio: 0.03,
            weight_decay: 0.001,
            max_seq_length: 2048,
            fp16: true,
            bf16: false,
            save_steps: 100,
            logging_steps: 10,
            save_total_limit: 3,
            lora_config: LoraTrainingConfig::default(),
            max_train_samples: Some(50000),
            max_tokens_per_sample: Some(4096),
            use_8bit_adam: false,
            gradient_checkpointing: true,
            dry_run: false,
            dataset_format: "completion".to_string(),
            prompt_template: None,
            eval_split_ratio: 0.0,
        }
    }
}

/// Request to start LoRA/QLoRA training
#[derive(Debug, Serialize, Deserialize)]
pub struct StartLoraTrainingRequest {
    pub model_id: String,
    pub project_id: String,
    pub base_model: String,
    pub config: TrainingConfig,
}

/// Training progress update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingProgress {
    pub status: String,
    pub current_step: u32,
    pub total_steps: u32,
    pub progress_percent: f32,
    pub current_loss: Option<f64>,
    pub learning_rate: Option<f64>,
    pub eta_seconds: Option<u32>,
    pub gpu_memory_used_gb: Option<f32>,
    pub gpu_memory_total_gb: Option<f32>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
}

/// Memory estimation result for dry run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEstimate {
    pub model_memory_gb: f32,
    pub optimizer_memory_gb: f32,
    pub gradient_memory_gb: f32,
    pub total_estimated_gb: f32,
    pub available_gpu_memory_gb: Option<f32>,
    pub will_fit: bool,
    pub recommendations: Vec<String>,
}

#[tauri::command]
pub async fn create_local_model(
    db: State<'_, Database>,
    request: CreateLocalModelRequest,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    let training_config_json = request.training_config_json
        .map(|v| serde_json::to_string(&v).unwrap_or_default())
        .unwrap_or_default();

    let name = request.name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| request.base_model.clone());
    
    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "INSERT INTO local_models (id, project_id, name, base_model, training_status, training_config_json, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, request.project_id, name, request.base_model, "pending", training_config_json, now, now],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(id)
}

/// Get adaptive chunk size based on available memory
fn get_adaptive_chunk_size(settings: &crate::commands_settings::TrainingSettings) -> usize {
    if !settings.enable_adaptive_memory {
        return settings.streaming_chunk_size;
    }
    
    // Try to detect available memory (simplified - in production, use system APIs)
    // For now, use configured chunk size, but could be enhanced with actual memory detection
    let base_chunk = settings.streaming_chunk_size;
    
    // Ensure chunk size is within bounds
    base_chunk.max(settings.min_chunk_size).min(settings.max_chunk_size)
}

/// Stream training data from database, compute hash, and write to file
/// Returns (data_hash, file_path, count)
fn stream_training_data_to_file(
    db: &Database,
    conn: &rusqlite::Connection,
    project_id: &str,
    model_id: &str,
    file_path: &std::path::PathBuf,
    compress: bool,
) -> Result<(String, i32), String> {
    use crate::commands_settings::load_settings_sync;
    
    let settings = load_settings_sync(db);
    let mut chunk_size = get_adaptive_chunk_size(&settings.training);
    use std::fs::File;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    
    let mut stmt = conn
        .prepare("SELECT input_text, output_text FROM training_data WHERE project_id = ?1 AND (local_model_id = ?2 OR local_model_id IS NULL) ORDER BY id")
        .map_err(|e| format!("Database error: {}", e))?;
    
    let mut hasher = Sha256::new();
    let mut count = 0;
    
    // Create file writer
    let file = File::create(file_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;
    
    let writer: Box<dyn Write> = if compress {
        Box::new(BufWriter::new(GzEncoder::new(file, Compression::default())))
    } else {
        Box::new(BufWriter::new(file))
    };
    
    let mut writer = writer;
    
    // Stream data from database
    let rows = stmt
        .query_map(rusqlite::params![project_id, model_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Database error: {}", e))?;
    
    for row in rows {
        let (input, output) = row.map_err(|e| format!("Row error: {}", e))?;
        
        // Update hash
        hasher.update(input.as_bytes());
        hasher.update(b"\0");
        hasher.update(output.as_bytes());
        hasher.update(b"\n");
        
        // Write to file
        let json_line = serde_json::json!({
            "input": input,
            "output": output
        });
        writeln!(writer, "{}", json_line)
            .map_err(|e| format!("Failed to write: {}", e))?;
        
        count += 1;
        
        // Adaptive memory management: adjust chunk size if memory pressure detected
        if settings.training.enable_adaptive_memory && count % 100 == 0 {
            // In a real implementation, check system memory here
            // For now, use configured chunk size
            chunk_size = get_adaptive_chunk_size(&settings.training);
        }
        
        // Flush at configured chunk size to reduce memory usage
        if count % chunk_size == 0 {
            writer.flush()
                .map_err(|e| format!("Failed to flush: {}", e))?;
            
            // Progress tracking (if enabled)
            if settings.training.enable_progress_tracking 
                && count % settings.training.progress_update_interval == 0 {
                // Could emit progress event here: e.g., "Processed {count} records"
            }
        }
    }
    
    // Final flush
    writer.flush()
        .map_err(|e| format!("Failed to flush: {}", e))?;
    
    let data_hash = format!("{:x}", hasher.finalize());
    
    Ok((data_hash, count as i32))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateLocalModelRequest {
    pub name: String,
    pub base_model: String,
    pub training_config_json: Option<serde_json::Value>,
}

#[tauri::command]
pub async fn update_local_model(
    db: State<'_, Database>,
    model_id: String,
    request: UpdateLocalModelRequest,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    
    let training_config_json = request.training_config_json
        .map(|v| serde_json::to_string(&v).unwrap_or_default())
        .unwrap_or_default();
    
    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "UPDATE local_models SET name = ?1, base_model = ?2, training_config_json = ?3, updated_at = ?4 WHERE id = ?5",
            rusqlite::params![request.name, request.base_model, training_config_json, now, model_id],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn list_local_models(
    db: State<'_, Database>,
    project_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let mut models = Vec::new();
    
    if let Some(pid) = project_id {
        let query = "SELECT id, project_id, name, base_model, model_path, training_status, training_config_json, training_metrics_json, created_at, updated_at FROM local_models WHERE project_id = ?1 ORDER BY created_at DESC";
        let mut stmt = conn_guard
            .prepare(query)
            .map_err(|e| format!("Database error: {}", e))?;
        
        let rows = stmt.query_map([pid], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "project_id": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "base_model": row.get::<_, String>(3)?,
                "model_path": row.get::<_, Option<String>>(4)?,
                "training_status": row.get::<_, String>(5)?,
                "training_config_json": row.get::<_, Option<String>>(6)?,
                "training_metrics_json": row.get::<_, Option<String>>(7)?,
                "created_at": row.get::<_, String>(8)?,
                "updated_at": row.get::<_, String>(9)?,
            }))
        })
        .map_err(|e| format!("Database error: {}", e))?;
        
        for row in rows {
            models.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
    } else {
        let query = "SELECT id, project_id, name, base_model, model_path, training_status, training_config_json, training_metrics_json, created_at, updated_at FROM local_models ORDER BY created_at DESC";
        let mut stmt = conn_guard
            .prepare(query)
            .map_err(|e| format!("Database error: {}", e))?;
        
        let rows = stmt.query_map([], |row| {
            Ok(serde_json::json!({
                "id": row.get::<_, String>(0)?,
                "project_id": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "base_model": row.get::<_, String>(3)?,
                "model_path": row.get::<_, Option<String>>(4)?,
                "training_status": row.get::<_, String>(5)?,
                "training_config_json": row.get::<_, Option<String>>(6)?,
                "training_metrics_json": row.get::<_, Option<String>>(7)?,
                "created_at": row.get::<_, String>(8)?,
                "updated_at": row.get::<_, String>(9)?,
            }))
        })
        .map_err(|e| format!("Database error: {}", e))?;
        
        for row in rows {
            models.push(row.map_err(|e| format!("Row error: {}", e))?);
        }
    }
    
    Ok(models)
}

#[tauri::command]
pub async fn create_training_data(
    db: State<'_, Database>,
    request: CreateTrainingDataRequest,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    
    let metadata_json = request.metadata_json
        .map(|v| serde_json::to_string(&v).unwrap_or_default())
        .unwrap_or_default();
    
    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "INSERT INTO training_data (id, project_id, local_model_id, input_text, output_text, metadata_json, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, request.project_id, request.local_model_id, request.input_text, request.output_text, metadata_json, now],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    
    // Invalidate cache for this project/model
    if let Some(model_id) = &request.local_model_id {
        let cache = TrainingCache::new((*db).clone());
        cache.invalidate_cache(&request.project_id, Some(model_id)).ok();
    } else {
        // Invalidate all caches for this project
        let cache = TrainingCache::new((*db).clone());
        cache.invalidate_cache(&request.project_id, None).ok();
    }
    
    Ok(id)
}

#[tauri::command]
pub async fn list_training_data(
    db: State<'_, Database>,
    project_id: Option<String>,
    local_model_id: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let (query, params) = match (project_id, local_model_id) {
        (Some(pid), Some(mid)) => (
            "SELECT id, project_id, local_model_id, input_text, output_text, metadata_json, created_at FROM training_data WHERE project_id = ?1 AND local_model_id = ?2 ORDER BY created_at DESC",
            (Some(pid), Some(mid))
        ),
        (Some(pid), None) => (
            "SELECT id, project_id, local_model_id, input_text, output_text, metadata_json, created_at FROM training_data WHERE project_id = ?1 ORDER BY created_at DESC",
            (Some(pid), None)
        ),
        (None, Some(mid)) => (
            "SELECT id, project_id, local_model_id, input_text, output_text, metadata_json, created_at FROM training_data WHERE local_model_id = ?1 ORDER BY created_at DESC",
            (None, Some(mid))
        ),
        (None, None) => (
            "SELECT id, project_id, local_model_id, input_text, output_text, metadata_json, created_at FROM training_data ORDER BY created_at DESC",
            (None, None)
        ),
    };
    
    let mut stmt = conn_guard
        .prepare(query)
        .map_err(|e| format!("Database error: {}", e))?;
    
    let mut data = Vec::new();
    
    match params {
        (Some(pid), Some(mid)) => {
            let rows = stmt.query_map(rusqlite::params![pid, mid], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "project_id": row.get::<_, String>(1)?,
                    "local_model_id": row.get::<_, Option<String>>(2)?,
                    "input_text": row.get::<_, String>(3)?,
                    "output_text": row.get::<_, String>(4)?,
                    "metadata_json": row.get::<_, Option<String>>(5)?,
                    "created_at": row.get::<_, String>(6)?,
                }))
            })
            .map_err(|e| format!("Database error: {}", e))?;
            
            for row in rows {
                data.push(row.map_err(|e| format!("Row error: {}", e))?);
            }
        },
        (Some(pid), None) => {
            let rows = stmt.query_map(rusqlite::params![pid], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "project_id": row.get::<_, String>(1)?,
                    "local_model_id": row.get::<_, Option<String>>(2)?,
                    "input_text": row.get::<_, String>(3)?,
                    "output_text": row.get::<_, String>(4)?,
                    "metadata_json": row.get::<_, Option<String>>(5)?,
                    "created_at": row.get::<_, String>(6)?,
                }))
            })
            .map_err(|e| format!("Database error: {}", e))?;
            
            for row in rows {
                data.push(row.map_err(|e| format!("Row error: {}", e))?);
            }
        },
        (None, Some(mid)) => {
            let rows = stmt.query_map(rusqlite::params![mid], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "project_id": row.get::<_, String>(1)?,
                    "local_model_id": row.get::<_, Option<String>>(2)?,
                    "input_text": row.get::<_, String>(3)?,
                    "output_text": row.get::<_, String>(4)?,
                    "metadata_json": row.get::<_, Option<String>>(5)?,
                    "created_at": row.get::<_, String>(6)?,
                }))
            })
            .map_err(|e| format!("Database error: {}", e))?;
            
            for row in rows {
                data.push(row.map_err(|e| format!("Row error: {}", e))?);
            }
        },
        (None, None) => {
            let rows = stmt.query_map([], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "project_id": row.get::<_, String>(1)?,
                    "local_model_id": row.get::<_, Option<String>>(2)?,
                    "input_text": row.get::<_, String>(3)?,
                    "output_text": row.get::<_, String>(4)?,
                    "metadata_json": row.get::<_, Option<String>>(5)?,
                    "created_at": row.get::<_, String>(6)?,
                }))
            })
            .map_err(|e| format!("Database error: {}", e))?;
            
            for row in rows {
                data.push(row.map_err(|e| format!("Row error: {}", e))?);
            }
        },
    }
    
    Ok(data)
}

#[tauri::command]
pub async fn update_local_model_status(
    db: State<'_, Database>,
    model_id: String,
    status: String,
    metrics_json: Option<serde_json::Value>,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    let metrics_json_str = metrics_json
        .map(|v| serde_json::to_string(&v).unwrap_or_default())
        .unwrap_or_default();
    
    let conn = db.get_connection();
    conn.lock()
        .map_err(|e| format!("Database lock error: {}", e))?
        .execute(
            "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2, updated_at = ?3 WHERE id = ?4",
            rusqlite::params![status, metrics_json_str, now, model_id],
        )
        .map_err(|e| format!("Database error: {}", e))?;
    
    Ok(())
}

#[tauri::command]
pub async fn check_training_environment() -> Result<serde_json::Value, String> {
    use std::process::Command;
    
    // Find Python - prefer 3.11 or 3.12 for CUDA support
    let (python_cmd, python_args) = {
        #[cfg(windows)]
        {
            // On Windows, try py launcher with specific versions first
            if Command::new("py").arg("-3.11").arg("--version").output().is_ok() {
                ("py", Some("-3.11"))
            } else if Command::new("py").arg("-3.12").arg("--version").output().is_ok() {
                ("py", Some("-3.12"))
            } else if Command::new("py").arg("--version").output().is_ok() {
                ("py", Some("-3"))
            } else if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Ok(serde_json::json!({
                    "python_available": false,
                    "cuda_available": false,
                    "message": "Python not found. Please install Python 3.8+ first.",
                    "device": "unknown"
                }));
            }
        }
        #[cfg(not(windows))]
        {
            if Command::new("python3").arg("--version").output().is_ok() {
                ("python3", None)
            } else if Command::new("python").arg("--version").output().is_ok() {
                ("python", None)
            } else {
                return Ok(serde_json::json!({
                    "python_available": false,
                    "cuda_available": false,
                    "message": "Python not found. Please install Python 3.8+ first.",
                    "device": "unknown"
                }));
            }
        }
    };
    
    // Helper to run Python commands (currently unused but kept for potential future use)
    #[allow(unused)]
    let _run_python = |args: &[&str]| -> std::io::Result<std::process::Output> {
        let mut cmd = Command::new(python_cmd);
        if let Some(version_arg) = python_args {
            cmd.arg(version_arg);
        }
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output()
    };
    
    // Helper to run Python commands
    let run_python = |args: &[&str]| -> std::io::Result<std::process::Output> {
        let mut cmd = Command::new(python_cmd);
        if let Some(version_arg) = python_args {
            cmd.arg(version_arg);
        }
        for arg in args {
            cmd.arg(arg);
        }
        cmd.output()
    };
    
    // Check if transformers is available
    let transformers_check = run_python(&["-c", "import transformers"]);
    
    let transformers_available = transformers_check.is_ok() && transformers_check.unwrap().status.success();
    
    if !transformers_available {
        return Ok(serde_json::json!({
            "python_available": true,
            "transformers_available": false,
            "cuda_available": false,
            "message": "transformers library not found. Please install: pip install transformers datasets torch",
            "device": "unknown"
        }));
    }
    
    // Check CUDA
    let cuda_check = run_python(&["-c", "import torch; print('CUDA:', torch.cuda.is_available()); print('DEVICE:', torch.cuda.get_device_name(0) if torch.cuda.is_available() and torch.cuda.device_count() > 0 else 'CPU'); print('COUNT:', torch.cuda.device_count() if torch.cuda.is_available() else 0)"]);
    
    if let Ok(output) = cuda_check {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut cuda_available = false;
            let mut device = "CPU".to_string();
            let mut device_count = 0;
            
            for line in stdout.lines() {
                if line.starts_with("CUDA:") {
                    cuda_available = line.split(':').nth(1).map(|s| s.trim() == "True").unwrap_or(false);
                } else if line.starts_with("DEVICE:") {
                    device = line.split(':').nth(1).map(|s| s.trim().to_string()).unwrap_or_else(|| "CPU".to_string());
                } else if line.starts_with("COUNT:") {
                    device_count = line.split(':').nth(1).and_then(|s| s.trim().parse().ok()).unwrap_or(0);
                }
            }
            
            let message = if cuda_available {
                format!("✅ GPU training ready! Using: {} ({} device(s))", device, device_count)
            } else {
                "⚠️ CUDA not available. Training will use CPU (slower). For GPU training, install PyTorch with CUDA support.".to_string()
            };
            
            Ok(serde_json::json!({
                "python_available": true,
                "transformers_available": true,
                "cuda_available": cuda_available,
                "device": device,
                "device_count": device_count,
                "message": message
            }))
        } else {
            Ok(serde_json::json!({
                "python_available": true,
                "transformers_available": true,
                "cuda_available": false,
                "device": "CPU",
                "message": "Could not check CUDA status. Assuming CPU mode."
            }))
        }
    } else {
        Ok(serde_json::json!({
            "python_available": true,
            "transformers_available": true,
            "cuda_available": false,
            "device": "CPU",
            "message": "Could not check CUDA status. Assuming CPU mode."
        }))
    }
}

#[tauri::command]
pub async fn start_training(
    db: State<'_, Database>,
    model_id: String,
    project_id: String,
    processes: State<'_, Arc<Mutex<HashMap<String, u32>>>>,
) -> Result<String, String> {
    use std::process::Command;
    use std::fs;
    use std::path::PathBuf;
    
    // Get model and training data with caching
    let (base_model, training_data_path, model_path, training_data_count) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        // Get model info
        let (base_model, _): (String, Option<String>) = conn_guard
            .query_row(
                "SELECT base_model, training_config_json FROM local_models WHERE id = ?1",
                [&model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| format!("Failed to get model: {}", e))?;
        
        // Get training data count
        let training_data_count: i32 = {
            let count: i32 = conn_guard
                .query_row(
                    "SELECT COUNT(*) FROM training_data WHERE project_id = ?1 AND (local_model_id = ?2 OR local_model_id IS NULL)",
                    rusqlite::params![&project_id, &model_id],
                    |row| Ok(row.get(0)?),
                )
                .map_err(|e| format!("Database error: {}", e))?;
            count
        };
        
        if training_data_count == 0 {
            return Err("No training data found. Please import training data first.".to_string());
        }
        
        // Initialize cache
        let cache = TrainingCache::new((*db).clone());
        
        // Load settings for hash computation
        let settings = crate::commands_settings::load_settings_sync(&db);
        
        // Compute hash of training data (with optional parallel processing for large datasets)
        let data_hash = {
            let mut stmt = conn_guard
                .prepare("SELECT input_text, output_text FROM training_data WHERE project_id = ?1 AND (local_model_id = ?2 OR local_model_id IS NULL) ORDER BY id")
                .map_err(|e| format!("Database error: {}", e))?;
            
            // For large datasets, we could use parallel hashing, but for now use sequential
            // Parallel hashing would require collecting data first, which defeats the purpose
            let mut hasher = Sha256::new();
            let mut count = 0;
            let rows = stmt
                .query_map(rusqlite::params![&project_id, &model_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| format!("Database error: {}", e))?;
            
            for row in rows {
                let (input, output) = row.map_err(|e| format!("Row error: {}", e))?;
                hasher.update(input.as_bytes());
                hasher.update(b"\0");
                hasher.update(output.as_bytes());
                hasher.update(b"\n");
                count += 1;
                
                // Progress update for large datasets (if enabled)
                if settings.training.enable_progress_tracking 
                    && count % settings.training.progress_update_interval == 0 {
                    // Could emit progress event here if needed
                }
            }
            
            format!("{:x}", hasher.finalize())
        };
        
        // Load settings to check compression preference
        let settings = crate::commands_settings::load_settings_sync(&db);
        let use_compression = settings.cache.enable_compression;
        
        // Check cache
        let training_data_file = match cache.get_cached_file(&project_id, &model_id, &data_hash)? {
            Some(cached_path) => {
                // Cache hit - use cached file
                cached_path
            }
            None => {
                // Cache miss - generate new file
                let cache_file_path = cache::get_cache_file_path(&project_id, &model_id, &data_hash, use_compression)
                    .map_err(|e| format!("Failed to get cache path: {}", e))?;
                
                // Ensure parent directory exists
                if let Some(parent) = cache_file_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create cache directory: {}", e))?;
                }
                
                // Stream data to cache file (with compression based on settings)
                stream_training_data_to_file(&db, &conn_guard, &project_id, &model_id, &cache_file_path, use_compression)
                    .map_err(|e| format!("Failed to stream to cache: {}", e))?;
                
                // Store in cache database
                cache.store_cache_entry(&project_id, &model_id, &data_hash, &cache_file_path)
                    .map_err(|e| format!("Failed to store cache entry: {}", e))?;
                
                cache_file_path
            }
        };
        
        // Model output path
        let app_data_dir = std::env::var("APPDATA")
            .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
            .unwrap_or_else(|_| ".".to_string());
        let training_dir = PathBuf::from(&app_data_dir).join("panther").join("training");
        fs::create_dir_all(&training_dir).map_err(|e| format!("Failed to create training directory: {}", e))?;
        let model_output_path = training_dir.join(&model_id);
        
        (base_model, training_data_file, model_output_path, training_data_count)
    };
    
    // Spawn actual training process
    let model_id_clone = model_id.clone();
    let db_clone: Database = (*db).clone();
    let base_model_clone = base_model.clone();
    let training_data_path_str = training_data_path.to_string_lossy().to_string();
    let model_path_str = model_path.to_string_lossy().to_string();
    let training_data_count_for_async = training_data_count;
    
    // Check if Python is available and verify transformers is installed
    // Prefer Python 3.11 or 3.12 for better CUDA support
    let python_cmd = {
        let mut found_python = None;
        
        // On Windows, try py launcher with specific versions first (for CUDA support)
        #[cfg(windows)]
        {
            // Try Python 3.11 first (best CUDA support)
            if Command::new("py").arg("-3.11").arg("--version").output().is_ok() {
                let check = Command::new("py")
                    .arg("-3.11")
                    .arg("-c")
                    .arg("import transformers")
                    .output();
                if check.is_ok() && check.unwrap().status.success() {
                    found_python = Some("py -3.11");
                }
            }
            
            // Try Python 3.12 if 3.11 didn't work
            if found_python.is_none() && Command::new("py").arg("-3.12").arg("--version").output().is_ok() {
                let check = Command::new("py")
                    .arg("-3.12")
                    .arg("-c")
                    .arg("import transformers")
                    .output();
                if check.is_ok() && check.unwrap().status.success() {
                    found_python = Some("py -3.12");
                }
            }
            
            // Fallback to py -3 (default Python 3)
            if found_python.is_none() && Command::new("py").arg("--version").output().is_ok() {
                let check = Command::new("py")
                    .arg("-3")
                    .arg("-c")
                    .arg("import transformers")
                    .output();
                if check.is_ok() && check.unwrap().status.success() {
                    found_python = Some("py -3");
                }
            }
        }
        
        // Try python3 first (non-Windows or fallback)
        if found_python.is_none() && Command::new("python3").arg("--version").output().is_ok() {
            let check = Command::new("python3")
                .arg("-c")
                .arg("import transformers")
                .output();
            if check.is_ok() && check.unwrap().status.success() {
                found_python = Some("python3");
            }
        }
        
        // Try python if python3 didn't work or doesn't have transformers
        if found_python.is_none() && Command::new("python").arg("--version").output().is_ok() {
            let check = Command::new("python")
                .arg("-c")
                .arg("import transformers")
                .output();
            if check.is_ok() && check.unwrap().status.success() {
                found_python = Some("python");
            }
        }
        
        found_python
    };
    
    let python_cmd = match python_cmd {
        Some(cmd) => cmd,
        None => {
            // No Python with transformers - mark as failed
            let conn = db_clone.get_connection();
            if let Ok(conn_guard) = conn.lock() {
                let error_msg = "Python with transformers library not found.\n\nPossible causes:\n1. Python is not installed\n2. transformers package is not installed for the Python being used\n3. Multiple Python installations - packages installed in different Python\n\nSteps to fix:\n1. Go to Settings > Dependencies\n2. Check which Python is detected\n3. Install dependencies using the 'Install All' button\n4. If that doesn't work, install manually in terminal:\n   python -m pip install transformers datasets torch\n   (Use the same 'python' command that works in your terminal)";
                let metrics = serde_json::json!({
                    "error": error_msg,
                    "status": "failed",
                    "error_type": "python_or_packages_not_found"
                });
                let _ = conn_guard.execute(
                    "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                    rusqlite::params!["failed", serde_json::to_string(&metrics).unwrap_or_default(), &model_id_clone],
                );
            };
            return Err("Python with transformers not found".to_string());
        }
    };
    
    // Create Python training script
    // Use raw strings (r"...") for Windows paths to avoid escape sequence issues
    let script_content = format!(
        r#"
import json
import sys
import os
from pathlib import Path

# Check for required libraries
try:
    import transformers
    from transformers import AutoModelForCausalLM, AutoTokenizer, TrainingArguments, Trainer
    from datasets import Dataset
    import torch
except ImportError as e:
    print(f"ERROR: Missing required library: {{e}}")
    print("Please install: pip install transformers datasets torch")
    sys.exit(1)

# Check PyTorch version early and warn if too old
torch_version = torch.__version__
print(f"PyTorch version: {{torch_version}}")
try:
    from packaging import version
    if version.parse(torch_version) < version.parse("2.6.0"):
        print("WARNING: PyTorch version is below 2.6.0. Some models may fail to load.")
        print("Recommended: pip install --upgrade torch")
        print("Alternatively, install safetensors: pip install safetensors")
        print("Attempting to use safetensors format to avoid torch.load restrictions...")
except ImportError:
    print("Note: 'packaging' library not found. Cannot check exact PyTorch version.")
    # Try manual version check
    try:
        major, minor = map(int, torch_version.split('.')[:2])
        if major < 2 or (major == 2 and minor < 6):
            print(f"WARNING: PyTorch version {{torch_version}} may be too old (< 2.6).")
            print("Consider upgrading: pip install --upgrade torch")
    except:
        pass
except Exception:
    pass

base_model = r"{}"
training_data_path = r"{}"
model_output_path = r"{}"

try:
    # Load training data using streaming (memory-efficient)
    import gzip
    
    def load_training_data_generator(file_path):
        \"\"\"Generator that yields training examples one at a time\"\"\"
        # Check if file is gzipped
        is_gzipped = file_path.endswith('.gz')
        
        if is_gzipped:
            file_handle = gzip.open(file_path, 'rt', encoding='utf-8')
        else:
            file_handle = open(file_path, 'r', encoding='utf-8')
        
        count = 0
        with file_handle as f:
            for line in f:
                if line.strip():
                    try:
                        data = json.loads(line)
                        count += 1
                        yield {{
                            "input": data.get("input", ""),
                            "output": data.get("output", "")
                        }}
                    except json.JSONDecodeError as e:
                        print(f"Warning: Skipping invalid JSON line: {{e}}")
                        continue
        
        if count == 0:
            raise ValueError("No training examples found in file")
    
    # Count examples first (for progress tracking)
    print("Counting training examples...")
    example_count = 0
    for _ in load_training_data_generator(training_data_path):
        example_count += 1
        if example_count % 10000 == 0:
            print(f"Counted {{example_count}} examples so far...")
    
    if example_count == 0:
        print("ERROR: No training examples found")
        sys.exit(1)
    
    print(f"Found {{example_count}} training examples")
    print("Using streaming data loader for memory efficiency...")
    
    # For Ollama models, we need to use a compatible base model
    # Since Ollama models are typically GGUF format, we'll use a similar HuggingFace model
    # Use gpt2 as default - it has safetensors weights and works with all PyTorch versions
    if "llama" in base_model.lower() or "mistral" in base_model.lower():
        # Llama and Mistral models are gated, use gpt2 as fallback
        # gpt2 has safetensors weights available, avoiding PyTorch version issues
        hf_model = "gpt2"
        print("Note: Using gpt2 as base model (has safetensors weights). For better results with Llama/Mistral, authenticate with Hugging Face and upgrade PyTorch to 2.6+.")
    elif "gpt" in base_model.lower() or "davinci" in base_model.lower():
        hf_model = "gpt2"  # Non-gated GPT-2 with safetensors
    else:
        # Try the base model name, but have a fallback to gpt2
        try:
            # Test if model is accessible
            from transformers import AutoConfig
            config = AutoConfig.from_pretrained(base_model)
            hf_model = base_model
        except Exception as e:
            print(f"Warning: Could not access model '{{base_model}}': {{e}}")
            print("Falling back to gpt2 (has safetensors weights)")
            hf_model = "gpt2"
    
    print(f"Loading base model: {{hf_model}}")
    
    # Check for Hugging Face token from environment or keychain
    import os
    hf_token = os.environ.get("HF_TOKEN") or os.environ.get("HUGGINGFACE_TOKEN")
    if hf_token:
        print("Using Hugging Face token from environment")
        from huggingface_hub import login
        try:
            login(token=hf_token)
        except Exception as e:
            print(f"Warning: Failed to login with HF token: {{e}}")
    
    # Check for GPU availability and PyTorch version
    import torch
    torch_version = torch.__version__
    print(f"PyTorch version: {{torch_version}}")
    
    # Check if PyTorch version is >= 2.6 (required for torch.load security)
    from packaging import version
    try:
        if version.parse(torch_version) < version.parse("2.6.0"):
            print(f"WARNING: PyTorch version {{torch_version}} is below 2.6.0")
            print("This may cause issues when loading models. Consider upgrading: pip install --upgrade torch")
            print("Attempting to use safetensors format to avoid torch.load restrictions...")
    except Exception:
        # If packaging is not available, try to parse manually
        try:
            major, minor = map(int, torch_version.split('.')[:2])
            if major < 2 or (major == 2 and minor < 6):
                print(f"WARNING: PyTorch version {{torch_version}} may be too old. Consider upgrading.")
        except:
            pass
    
    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"Using device: {{device}}")
    if device == "cuda":
        print(f"GPU: {{torch.cuda.get_device_name(0)}}")
        print(f"CUDA Version: {{torch.version.cuda}}")
        print(f"GPU Memory: {{torch.cuda.get_device_properties(0).total_memory / 1024**3:.2f}} GB")
    else:
        print("No GPU detected. Training will use CPU (this will be slower).")
    
    # Check if safetensors is installed
    safetensors_available = False
    try:
        import safetensors
        safetensors_available = True
        print("safetensors library is available")
    except ImportError:
        print("safetensors library is not installed")
    
    # Load tokenizer and model with error handling for gated repos
    # Use safetensors=True to avoid torch.load restrictions on older PyTorch versions
    try:
        tokenizer = AutoTokenizer.from_pretrained(hf_model)
        if tokenizer.pad_token is None:
            tokenizer.pad_token = tokenizer.eos_token
        
        # Try to load with safetensors first (avoids torch.load restriction)
        # If safetensors is not available or model doesn't have safetensors weights, fall back
        model_loaded = False
        if safetensors_available:
            try:
                # Try to load with safetensors - this will work if model has safetensors weights
                model = AutoModelForCausalLM.from_pretrained(hf_model, use_safetensors=True)
                print("Loaded model using safetensors format (avoids PyTorch version restrictions)")
                model_loaded = True
            except Exception as safetensors_error:
                safetensors_error_str = str(safetensors_error)
                print(f"Model does not have safetensors weights or error: {{safetensors_error_str}}")
                print("Attempting to load with regular format...")
        
        if not model_loaded:
            # Check PyTorch version before attempting regular load
            try:
                from packaging import version
                if version.parse(torch_version) < version.parse("2.6.0"):
                    raise ValueError(
                        f"PyTorch version {{torch_version}} is below 2.6.0. "
                        "Due to security restrictions, models cannot be loaded with torch.load. "
                        "Solutions:\n"
                        "1. Upgrade PyTorch: pip install --upgrade torch\n"
                        "2. Use a model that has safetensors weights (e.g., gpt2, microsoft/DialoGPT-medium)\n"
                        "3. The model '{{hf_model}}' may not have safetensors weights available"
                    )
            except ValueError as ve:
                # Re-raise the ValueError we just created
                raise
            except:
                # If packaging check failed, try manual version check
                try:
                    major, minor = map(int, torch_version.split('.')[:2])
                    if major < 2 or (major == 2 and minor < 6):
                        raise ValueError(
                            f"PyTorch version {{torch_version}} is below 2.6.0. "
                            "Please upgrade: pip install --upgrade torch"
                        )
                except ValueError:
                    raise
                except:
                    pass
            # If we get here, PyTorch version is OK or we couldn't check
            model = AutoModelForCausalLM.from_pretrained(hf_model)
        model = model.to(device)
    except Exception as e:
        error_msg = str(e)
        if "gated" in error_msg.lower() or "401" in error_msg or "unauthorized" in error_msg.lower():
            print(f"ERROR: Model '{{hf_model}}' is gated and requires Hugging Face authentication.")
            print("To use gated models:")
            print("1. Get a token from https://huggingface.co/settings/tokens")
            print("2. Set environment variable: export HF_TOKEN=your_token_here")
            print("3. Or use a non-gated model like 'gpt2' or 'microsoft/DialoGPT-medium'")
            print(f"Falling back to 'gpt2' (non-gated model)...")
            hf_model = "gpt2"
            tokenizer = AutoTokenizer.from_pretrained(hf_model)
            if tokenizer.pad_token is None:
                tokenizer.pad_token = tokenizer.eos_token
            # Use gpt2 which has safetensors weights
            hf_model = "gpt2"
            try:
                model = AutoModelForCausalLM.from_pretrained(hf_model, use_safetensors=True)
                print("Successfully loaded gpt2 with safetensors as fallback model.")
            except:
                model = AutoModelForCausalLM.from_pretrained(hf_model)
                print("Successfully loaded gpt2 as fallback model.")
            model = model.to(device)
        else:
            raise  # Re-raise if it's a different error
    
    # Prepare dataset using streaming generator (memory-efficient)
    def format_prompt(example):
        return f"Input: {{example['input']}}\nOutput: {{example['output']}}"
    
    def training_data_generator():
        \"\"\"Generator that yields formatted training examples\"\"\"
        for example in load_training_data_generator(training_data_path):
            yield {{"text": format_prompt(example)}}
    
    # Create dataset from generator (streaming, memory-efficient)
    # For very large datasets, use IterableDataset
    # Threshold is configurable but defaults to 100K for IterableDataset
    # This can be adjusted in settings if needed
    use_iterable = example_count > 100000
    if use_iterable:
        print("Large dataset detected - using IterableDataset for streaming")
        from torch.utils.data import IterableDataset
        
        class StreamingDataset(IterableDataset):
            def __init__(self, generator):
                self.generator = generator
            
            def __iter__(self):
                return iter(self.generator())
        
        dataset = StreamingDataset(training_data_generator)
        # For IterableDataset, we need to tokenize on-the-fly during training
        # We'll handle this in a custom data collator
    else:
        # For smaller datasets, use regular Dataset (still memory-efficient with generator)
        dataset = Dataset.from_generator(training_data_generator)
    
    def tokenize_function(examples):
        # When batched=True, examples["text"] is a list of strings
        texts = examples["text"]
        # Ensure texts is a list of strings
        if isinstance(texts, str):
            texts = [texts]
        # Tokenize the text
        tokenized = tokenizer(texts, truncation=True, padding=True, max_length=512, return_tensors=None)
        # For causal LM training, labels are the same as input_ids
        # The model will automatically shift them internally
        tokenized["labels"] = tokenized["input_ids"].copy()
        return tokenized
    
    # Tokenize dataset (with adaptive batching for efficiency)
    # Use larger batch size for tokenization if dataset is small, smaller for large datasets
    tokenization_batch_size = 1000 if example_count < 50000 else 500
    if use_iterable:
        # For IterableDataset, tokenization happens during training
        tokenized_dataset = dataset
    else:
        tokenized_dataset = dataset.map(tokenize_function, batched=True, batch_size=tokenization_batch_size, remove_columns=["text"])
    
    # Training arguments with adaptive memory management
    # Adjust batch size based on device and available memory
    # Base batch size: GPU can handle larger batches
    base_batch_size = 4 if device == "cuda" else 1
    base_gradient_accumulation = 4 if device == "cuda" else 8
    
    # Adaptive batch size adjustment based on dataset size and memory
    # For very large datasets, reduce batch size to avoid OOM
    if example_count > 100000:
        # Large dataset - use smaller batches
        batch_size = max(1, base_batch_size // 2)
        gradient_accumulation = base_gradient_accumulation * 2
    elif example_count > 50000:
        # Medium-large dataset - slightly reduce
        batch_size = max(1, int(base_batch_size * 0.75))
        gradient_accumulation = int(base_gradient_accumulation * 1.5)
    else:
        # Small-medium dataset - use base settings
        batch_size = base_batch_size
        gradient_accumulation = base_gradient_accumulation
    
    num_epochs = 3
    # Calculate total steps - ensure at least 1 to avoid division by zero
    # For IterableDataset, we already have example_count
    if use_iterable:
        dataset_size = example_count
    else:
        dataset_size = len(tokenized_dataset)
    steps_per_epoch = max(1, dataset_size // (batch_size * gradient_accumulation))
    total_steps = max(1, steps_per_epoch * num_epochs)
    
    print(f"Adaptive batch size: {{batch_size}} (device: {{device}}, dataset size: {{example_count}})")
    print(f"Gradient accumulation: {{gradient_accumulation}}")
    
    # Check transformers version for API compatibility
    import transformers
    transformers_version = transformers.__version__
    print(f"Transformers version: {{transformers_version}}")
    
    # Build training arguments - handle API changes between versions
    # evaluation_strategy was renamed to eval_strategy in transformers 4.46+
    training_args_kwargs = {{
        "output_dir": model_output_path,
        "num_train_epochs": num_epochs,
        "per_device_train_batch_size": batch_size,
        "gradient_accumulation_steps": 4 if device == "cuda" else 8,
        "warmup_steps": 10,
        "logging_steps": 1,
        "save_steps": 100,
        "save_total_limit": 1,
        "fp16": device == "cuda",
        "dataloader_pin_memory": device == "cuda",
        "report_to": [],
    }}
    
    # Handle API change: evaluation_strategy -> eval_strategy in newer versions
    try:
        from packaging import version
        if version.parse(transformers_version) >= version.parse("4.46.0"):
            training_args_kwargs["eval_strategy"] = "no"
        else:
            training_args_kwargs["evaluation_strategy"] = "no"
    except ImportError:
        # If packaging not available, try new API first
        training_args_kwargs["eval_strategy"] = "no"
    except Exception:
        training_args_kwargs["evaluation_strategy"] = "no"
    
    training_args = TrainingArguments(**training_args_kwargs)
    
    # Create trainer with custom callback for progress tracking
    from transformers import TrainerCallback
    import time
    
    class ProgressCallback(TrainerCallback):
        def __init__(self, total_steps):
            self.total_steps = max(1, total_steps)  # Ensure at least 1 to avoid division by zero
            self.start_time = None
            self.current_step = 0
            
        def on_train_begin(self, args, state, control, **kwargs):
            self.start_time = time.time()
            # Use state.max_steps if available (more accurate)
            if hasattr(state, 'max_steps') and state.max_steps > 0:
                self.total_steps = state.max_steps
            print(f"PROGRESS:0:0:Training started. Total steps: {{self.total_steps}}")
            
        def on_log(self, args, state, control, logs=None, **kwargs):
            if state.global_step > 0:
                self.current_step = state.global_step
                # Use max_steps from state if available
                actual_total = state.max_steps if hasattr(state, 'max_steps') and state.max_steps > 0 else self.total_steps
                actual_total = max(1, actual_total)  # Ensure no division by zero
                progress_pct = min(100, int((self.current_step / actual_total) * 100))
                elapsed = time.time() - self.start_time if self.start_time else 0
                if self.current_step > 0 and elapsed > 0:
                    avg_time_per_step = elapsed / self.current_step
                    remaining_steps = max(0, actual_total - self.current_step)
                    eta_seconds = remaining_steps * avg_time_per_step
                    eta_minutes = int(eta_seconds / 60)
                    eta_seconds_remainder = int(eta_seconds % 60)
                    print(f"PROGRESS:{{progress_pct}}:{{eta_minutes}}m{{eta_seconds_remainder}}s:Step {{self.current_step}}/{{actual_total}}")
                    
                    # Monitor GPU/CPU usage
                    try:
                        if device == "cuda":
                            gpu_memory = torch.cuda.memory_allocated(0) / 1024**3
                            gpu_memory_total = torch.cuda.get_device_properties(0).total_memory / 1024**3
                            if gpu_memory_total > 0:
                                gpu_util = (gpu_memory / gpu_memory_total) * 100
                                print(f"RESOURCE:GPU:{{gpu_util:.1f}}%:{{gpu_memory:.2f}}GB/{{gpu_memory_total:.2f}}GB")
                        else:
                            try:
                                import psutil
                                cpu_percent = psutil.cpu_percent(interval=0.1)
                                memory = psutil.virtual_memory()
                                print(f"RESOURCE:CPU:{{cpu_percent:.1f}}%:Memory {{memory.percent:.1f}}%")
                            except ImportError:
                                pass  # psutil not installed, skip resource monitoring
                    except Exception:
                        pass  # Ignore resource monitoring errors
    
    # For IterableDataset, we need a custom data collator
    # The dataset already yields formatted text, so we just need to tokenize
    if use_iterable:
        from transformers import DataCollatorForLanguageModeling
        
        # Create a custom collator that handles the text field
        class CustomDataCollator:
            def __init__(self, tokenizer):
                self.tokenizer = tokenizer
            
            def __call__(self, examples):
                # Extract text from examples
                texts = [ex["text"] for ex in examples]
                # Tokenize
                tokenized = self.tokenizer(texts, truncation=True, padding=True, max_length=512, return_tensors="pt")
                # For causal LM, labels are same as input_ids
                tokenized["labels"] = tokenized["input_ids"].clone()
                return tokenized
        
        data_collator = CustomDataCollator(tokenizer)
    else:
        data_collator = None
    
    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=tokenized_dataset,
        data_collator=data_collator,
        callbacks=[ProgressCallback(total_steps)],
    )
    
    print("Starting training...")
    trainer.train()
    
    # Save model
    print(f"Saving model to {{model_output_path}}")
    model.save_pretrained(model_output_path)
    tokenizer.save_pretrained(model_output_path)
    
    print("Training completed successfully")
    
except Exception as e:
    print(f"ERROR: Training failed: {{e}}")
    import traceback
    traceback.print_exc()
    sys.exit(1)
"#,
        base_model_clone, training_data_path_str, model_path_str
    );
    
    let script_path = PathBuf::from(&training_data_path_str)
        .parent()
        .unwrap()
        .join(format!("{}_train.py", model_id_clone));
    
    if let Err(e) = fs::write(&script_path, script_content) {
        eprintln!("Failed to write training script: {}", e);
        let conn = db_clone.get_connection();
        if let Ok(conn_guard) = conn.lock() {
            let metrics = serde_json::json!({
                "error": format!("Failed to create training script: {}", e),
                "status": "failed"
            });
            let _ = conn_guard.execute(
                "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                rusqlite::params!["failed", serde_json::to_string(&metrics).unwrap_or_default(), &model_id_clone],
            );
        };
        return Err(format!("Failed to create training script: {}", e));
    }
    
    // Update status to training ONLY after Python is confirmed and script is created
    {
        let conn = db_clone.get_connection();
        if let Ok(conn_guard) = conn.lock() {
            let _ = conn_guard.execute(
                "UPDATE local_models SET training_status = ?1 WHERE id = ?2",
                rusqlite::params!["training", &model_id_clone],
            );
        };
    }
    
    // Execute training script using the Python we verified has transformers
    // Parse python_cmd - it might be "py -3.11" or just "python"
    // Use tokio::process for async execution with real-time output parsing
    let db_for_monitor = db_clone.clone();
    let model_id_monitor = model_id_clone.clone();
    let script_path_clone = script_path.clone();
    let python_cmd_clone = python_cmd; // &str is Copy, no need to clone
    let model_path_str_clone = model_path_str.clone();
    let training_data_count_for_async_clone = training_data_count_for_async;
    let processes_clone: Arc<Mutex<HashMap<String, u32>>> = Arc::clone(processes.inner());
    
    // Get HF token from keychain if available
    let hf_token_clone = {
        use crate::keychain::Keychain;
        let keychain = Keychain::new();
        keychain.retrieve("panther", "hf_token").ok()
    };
    
    tokio::spawn(async move {
            use tokio::process::Command as TokioCommand;
            use tokio::io::{AsyncBufReadExt, BufReader};
            
            // Build command
            let mut cmd = if python_cmd_clone.starts_with("py ") {
                let parts: Vec<&str> = python_cmd_clone.splitn(2, ' ').collect();
                let version_arg = parts.get(1).unwrap_or(&"-3");
                let mut c = TokioCommand::new("py");
                c.arg(version_arg).arg(&script_path_clone);
                c
            } else {
                let mut c = TokioCommand::new(&python_cmd_clone);
                c.arg(&script_path_clone);
                c
            };
            
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());
            
            // Set HF_TOKEN environment variable if available
            if let Some(token) = &hf_token_clone {
                cmd.env("HF_TOKEN", token);
                cmd.env("HUGGINGFACE_TOKEN", token); // Some tools use this name
            }
            
            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to spawn training process: {}", e);
                    let conn = db_for_monitor.get_connection();
                    if let Ok(conn_guard) = conn.lock() {
                        let metrics = serde_json::json!({
                            "error": format!("Failed to spawn training process: {}", e),
                            "status": "failed"
                        });
                        let _ = conn_guard.execute(
                            "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                            rusqlite::params!["failed", serde_json::to_string(&metrics).unwrap_or_default(), &model_id_monitor],
                        );
                    }
                    return;
                }
            };
            
            // Store process ID for cancellation BEFORE starting to read output
            // This ensures stop_training can find the process even if it fails immediately
            let pid = child.id();
            if let Some(pid_value) = pid {
                if let Ok(mut procs) = processes_clone.lock() {
                    procs.insert(model_id_monitor.clone(), pid_value);
                    eprintln!("[Training] Stored PID {} for model {}", pid_value, model_id_monitor);
                }
            } else {
                eprintln!("[Training] Warning: Could not get PID for spawned process");
            }
            
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();
            
            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();
            
            // Parse output lines for progress and resource usage
            loop {
                tokio::select! {
                    line = stdout_reader.next_line() => {
                        match line {
                            Ok(Some(line)) => {
                                // Parse PROGRESS and RESOURCE lines
                                if line.starts_with("PROGRESS:") {
                                    let parts: Vec<&str> = line.splitn(4, ':').collect();
                                    if parts.len() >= 4 {
                                        if let Ok(progress) = parts[1].parse::<u32>() {
                                            let metrics_json = serde_json::to_string(&serde_json::json!({
                                                "progress": progress,
                                                "eta": parts[2],
                                                "message": parts[3]
                                            })).unwrap_or_default();
                                            let model_id_for_update = model_id_monitor.clone();
                                            {
                                                let conn = db_for_monitor.get_connection();
                                                if let Ok(conn_guard) = conn.lock() {
                                                    let _ = conn_guard.execute(
                                                        "UPDATE local_models SET training_metrics_json = ?1 WHERE id = ?2",
                                                        rusqlite::params![metrics_json, &model_id_for_update],
                                                    );
                                                };
                                            }
                                        }
                                    }
                                } else if line.starts_with("RESOURCE:") {
                                    let parts: Vec<&str> = line.splitn(4, ':').collect();
                                    if parts.len() >= 4 {
                                        let model_id_for_update = model_id_monitor.clone();
                                        
                                        // Get existing metrics
                                        let existing_metrics_str: Option<String> = {
                                            let conn = db_for_monitor.get_connection();
                                            let result = if let Ok(conn_guard) = conn.lock() {
                                                conn_guard.query_row(
                                                    "SELECT training_metrics_json FROM local_models WHERE id = ?1",
                                                    [&model_id_for_update],
                                                    |row| row.get(0)
                                                ).ok().flatten()
                                            } else {
                                                None
                                            };
                                            result
                                        };
                                        
                                        let mut metrics: serde_json::Value = existing_metrics_str
                                            .and_then(|s| serde_json::from_str(&s).ok())
                                            .unwrap_or(serde_json::json!({}));
                                        
                                        if parts[1] == "GPU" {
                                            metrics["gpu_usage"] = serde_json::json!({
                                                "percent": parts[2],
                                                "memory": parts[3]
                                            });
                                        } else if parts[1] == "CPU" {
                                            metrics["cpu_usage"] = serde_json::json!({
                                                "percent": parts[2],
                                                "memory": parts[3]
                                            });
                                        }
                                        
                                        let metrics_json = serde_json::to_string(&metrics).unwrap_or_default();
                                        {
                                            let conn = db_for_monitor.get_connection();
                                            if let Ok(conn_guard) = conn.lock() {
                                                let _ = conn_guard.execute(
                                                    "UPDATE local_models SET training_metrics_json = ?1 WHERE id = ?2",
                                                    rusqlite::params![metrics_json, &model_id_for_update],
                                                );
                                            };
                                        }
                                    }
                                }
                            }
                            Ok(None) | Err(_) => break,
                        }
                    }
                    line = stderr_reader.next_line() => {
                        match line {
                            Ok(Some(stderr_line)) => {
                                // Log errors and capture them for error reporting
                                eprintln!("[Training] Python stderr: {}", stderr_line);
                                
                                // If it's an error line, capture it
                                if stderr_line.contains("ERROR:") || stderr_line.contains("Error:") || stderr_line.contains("Traceback") {
                                    let conn = db_for_monitor.get_connection();
                                    if let Ok(conn_guard) = conn.lock() {
                                        // Get existing metrics
                                        let existing_metrics_str: Option<String> = conn_guard
                                            .query_row(
                                                "SELECT training_metrics_json FROM local_models WHERE id = ?1",
                                                [&model_id_monitor],
                                                |row| row.get(0)
                                            )
                                            .ok()
                                            .flatten();
                                        
                                        let mut metrics: serde_json::Value = existing_metrics_str
                                            .and_then(|s| serde_json::from_str(&s).ok())
                                            .unwrap_or(serde_json::json!({}));
                                        
                                        // Append error to stderr field
                                        let current_stderr = metrics.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
                                        let new_stderr = if current_stderr.is_empty() {
                                            stderr_line
                                        } else {
                                            format!("{}\n{}", current_stderr, stderr_line)
                                        };
                                        metrics["stderr"] = serde_json::Value::String(new_stderr);
                                        
                                        let metrics_json = serde_json::to_string(&metrics).unwrap_or_default();
                                        let _ = conn_guard.execute(
                                            "UPDATE local_models SET training_metrics_json = ?1 WHERE id = ?2",
                                            rusqlite::params![metrics_json, &model_id_monitor],
                                        );
                                    };
                                }
                            }
                            Ok(None) | Err(_) => break,
                        }
                    }
                }
            }
            
            // Wait for process to complete
            let status = child.wait().await;
            
            let conn = db_for_monitor.get_connection();
            if let Ok(conn_guard) = conn.lock() {
                if let Ok(exit_status) = status {
                    if exit_status.success() {
                        let metrics = serde_json::json!({
                            "status": "complete",
                            "progress": 100,
                            "examples": training_data_count_for_async_clone,
                            "model_path": model_path_str_clone
                        });
                        let _ = conn_guard.execute(
                            "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2, model_path = ?3 WHERE id = ?4",
                            rusqlite::params!["complete", serde_json::to_string(&metrics).unwrap_or_default(), model_path_str_clone, &model_id_monitor],
                        );
                    } else {
                        // Get exit code and any captured stderr
                        let exit_code = exit_status.code().unwrap_or(-1);
                        
                        // Get existing metrics to preserve stderr
                        let existing_metrics_str: Option<String> = conn_guard
                            .query_row(
                                "SELECT training_metrics_json FROM local_models WHERE id = ?1",
                                [&model_id_monitor],
                                |row| row.get(0)
                            )
                            .ok()
                            .flatten();
                        
                        let mut metrics: serde_json::Value = existing_metrics_str
                            .and_then(|s| serde_json::from_str(&s).ok())
                            .unwrap_or(serde_json::json!({}));
                        
                        metrics["status"] = serde_json::json!("failed");
                        metrics["exit_code"] = serde_json::json!(exit_code);
                        
                        // Build error message from stderr if available
                        let error_msg = if let Some(stderr) = metrics.get("stderr").and_then(|v| v.as_str()) {
                            if !stderr.trim().is_empty() {
                                format!("Training process exited with error (code: {}):\n\n{}", exit_code, stderr)
                            } else {
                                format!("Training process exited with error code: {}", exit_code)
                            }
                        } else {
                            format!("Training process exited with error code: {}", exit_code)
                        };
                        
                        metrics["error"] = serde_json::json!(error_msg);
                        
                        let _ = conn_guard.execute(
                            "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                            rusqlite::params!["failed", serde_json::to_string(&metrics).unwrap_or_default(), &model_id_monitor],
                        );
                    }
                }
            }
            
            // Remove from processes map
            if let Ok(mut procs) = processes_clone.lock() {
                procs.remove(&model_id_monitor);
            }
        });
    
    Ok(format!("Training started for model {}. This may take several minutes depending on your data size and hardware.", model_id))
}

/// Generate the Python script for LoRA/QLoRA training
fn generate_lora_training_script(
    base_model: &str,
    training_data_path: &str,
    output_path: &str,
    config: &TrainingConfig,
) -> String {
    let lora_config = &config.lora_config;
    let target_modules_json = serde_json::to_string(&lora_config.target_modules).unwrap_or("[]".to_string());
    
    format!(r####"#!/usr/bin/env python3
# -*- coding: utf-8 -*-
# Panther LoRA/QLoRA Training Script
# Generated by Panther AI - Do not edit manually

import json
import sys
import os
import gc
from pathlib import Path

# ============================================
# Configuration
# ============================================
BASE_MODEL = r"{base_model}"
TRAINING_DATA_PATH = r"{training_data_path}"
OUTPUT_PATH = r"{output_path}"

# Training hyperparameters
NUM_TRAIN_EPOCHS = {num_train_epochs}
LEARNING_RATE = {learning_rate}
PER_DEVICE_TRAIN_BATCH_SIZE = {per_device_train_batch_size}
GRADIENT_ACCUMULATION_STEPS = {gradient_accumulation_steps}
WARMUP_RATIO = {warmup_ratio}
WEIGHT_DECAY = {weight_decay}
MAX_SEQ_LENGTH = {max_seq_length}
FP16 = {fp16}
BF16 = {bf16}
SAVE_STEPS = {save_steps}
LOGGING_STEPS = {logging_steps}
SAVE_TOTAL_LIMIT = {save_total_limit}
USE_8BIT_ADAM = {use_8bit_adam}
GRADIENT_CHECKPOINTING = {gradient_checkpointing}
DRY_RUN = {dry_run}
DATASET_FORMAT = "{dataset_format}"
EVAL_SPLIT_RATIO = {eval_split_ratio}

# LoRA configuration
USE_LORA = {use_lora}
USE_QLORA = {use_qlora}
LORA_RANK = {lora_rank}
LORA_ALPHA = {lora_alpha}
LORA_DROPOUT = {lora_dropout}
TARGET_MODULES = {target_modules}
LORA_BIAS = "{lora_bias}"
LORA_TASK_TYPE = "{lora_task_type}"

# Safety caps
MAX_TRAIN_SAMPLES = {max_train_samples}
MAX_TOKENS_PER_SAMPLE = {max_tokens_per_sample}

# ============================================
# Dependency checks
# ============================================
def check_dependencies():
    # Check and report on available dependencies
    deps = {{}}
    warnings = []
    
    # Core dependencies
    try:
        import torch
        deps['torch'] = torch.__version__
        deps['cuda_available'] = torch.cuda.is_available()
        if deps['cuda_available']:
            deps['gpu_name'] = torch.cuda.get_device_name(0)
            deps['gpu_memory_gb'] = torch.cuda.get_device_properties(0).total_memory / 1024**3
        else:
            warnings.append("No CUDA GPU detected. Training will use CPU (very slow for large models).")
    except ImportError:
        print("ERROR: PyTorch not installed. Run: pip install torch")
        sys.exit(1)
    
    try:
        import transformers
        deps['transformers'] = transformers.__version__
    except ImportError:
        print("ERROR: transformers not installed. Run: pip install transformers")
        sys.exit(1)
    
    try:
        from datasets import Dataset
        import datasets
        deps['datasets'] = datasets.__version__
    except ImportError:
        print("ERROR: datasets not installed. Run: pip install datasets")
        sys.exit(1)
    
    try:
        from peft import LoraConfig, get_peft_model
        import peft
        deps['peft'] = peft.__version__
    except ImportError:
        if USE_LORA or USE_QLORA:
            print("ERROR: peft not installed (required for LoRA/QLoRA). Run: pip install peft")
            sys.exit(1)
        else:
            warnings.append("peft not installed. LoRA training unavailable.")
    
    # Optional dependencies
    try:
        import bitsandbytes as bnb
        deps['bitsandbytes'] = bnb.__version__
    except ImportError:
        deps['bitsandbytes'] = None
        if USE_QLORA:
            warnings.append("bitsandbytes not installed. QLoRA (4-bit) training unavailable. Using LoRA instead.")
        if USE_8BIT_ADAM:
            warnings.append("bitsandbytes not installed. 8-bit Adam unavailable. Using standard Adam.")
    
    # Check bitsandbytes on Windows
    if sys.platform == 'win32' and deps.get('bitsandbytes'):
        warnings.append("bitsandbytes on Windows can be unstable. If training fails, try: pip install bitsandbytes-windows")
    
    try:
        import accelerate
        deps['accelerate'] = accelerate.__version__
    except ImportError:
        warnings.append("accelerate not installed. Some optimizations unavailable. Run: pip install accelerate")
    
    try:
        from trl import SFTTrainer, DataCollatorForCompletionOnlyLM
        import trl
        deps['trl'] = trl.__version__
    except ImportError:
        deps['trl'] = None
        warnings.append("trl not installed. Using standard Trainer. Run: pip install trl")
    
    return deps, warnings

# ============================================
# Data loading
# ============================================
def load_training_data(file_path, max_samples=None):
    # Load training data from JSONL file
    import gzip
    
    data = []
    is_gzipped = file_path.endswith('.gz')
    
    opener = gzip.open if is_gzipped else open
    mode = 'rt' if is_gzipped else 'r'
    
    with opener(file_path, mode, encoding='utf-8') as f:
        for i, line in enumerate(f):
            if max_samples and i >= max_samples:
                print(f"INFO: Reached max_samples limit ({{max_samples}})")
                break
            if line.strip():
                try:
                    data.append(json.loads(line))
                except json.JSONDecodeError as e:
                    print(f"WARNING: Skipping invalid JSON at line {{i+1}}: {{e}}")
    
    if not data:
        raise ValueError("No training data found in file")
    
    return data

def format_training_example(example, format_type="completion", template=None):
    # Format a training example based on dataset format
    inp = example.get("input", "")
    out = example.get("output", "")
    
    NL = chr(10)  # Newline character
    if format_type == "alpaca":
        # Alpaca-style format
        if inp:
            return f"### Instruction:{{NL}}{{inp}}{{NL}}{{NL}}### Response:{{NL}}{{out}}"
        else:
            return f"### Response:{{NL}}{{out}}"
    elif format_type == "sharegpt":
        # ShareGPT-style format (for chat models)
        return f"<|user|>{{NL}}{{inp}}<|end|>{{NL}}<|assistant|>{{NL}}{{out}}<|end|>"
    elif format_type == "custom" and template:
        return template.replace("{{{{input}}}}", inp).replace("{{{{output}}}}", out)
    else:
        # Simple completion format
        return f"Input: {{inp}}{{NL}}Output: {{out}}"

# ============================================
# Memory estimation
# ============================================
def estimate_memory_requirements(model_name, config, num_samples, seq_length):
    # Estimate GPU memory requirements for training
    import torch
    
    # Rough estimates based on model size
    # These are approximations and can vary significantly
    model_sizes = {{
        'gpt2': 0.5,
        'gpt2-medium': 1.5,
        'gpt2-large': 3.0,
        'gpt2-xl': 6.0,
        'llama-7b': 14.0,
        'llama-13b': 26.0,
        'llama-70b': 140.0,
        'mistral-7b': 14.0,
    }}
    
    # Try to match model name
    model_lower = model_name.lower()
    base_size_gb = 1.0  # Default
    for name, size in model_sizes.items():
        if name in model_lower:
            base_size_gb = size
            break
    
    # LoRA reduces memory significantly
    if USE_LORA:
        # LoRA typically uses 1-5% of model parameters
        trainable_ratio = (LORA_RANK * 2) / 1000  # Rough approximation
        base_size_gb = base_size_gb * 0.3 + base_size_gb * trainable_ratio
    
    # QLoRA (4-bit) reduces memory by ~4x
    if USE_QLORA:
        base_size_gb = base_size_gb / 4
    
    # Optimizer states (Adam uses 2x model size for momentum/variance)
    optimizer_gb = base_size_gb * 2 if not USE_8BIT_ADAM else base_size_gb * 0.5
    
    # Gradient memory
    gradient_gb = base_size_gb * 0.5 if GRADIENT_CHECKPOINTING else base_size_gb
    
    # Batch memory (rough estimate)
    batch_memory = (PER_DEVICE_TRAIN_BATCH_SIZE * seq_length * 4) / (1024**3)  # 4 bytes per float
    
    total_gb = base_size_gb + optimizer_gb + gradient_gb + batch_memory
    
    # Get available GPU memory
    available_gb = None
    if torch.cuda.is_available():
        available_gb = torch.cuda.get_device_properties(0).total_memory / (1024**3)
        # Leave some headroom (20%)
        usable_gb = available_gb * 0.8
    else:
        usable_gb = float('inf')
    
    will_fit = total_gb < usable_gb
    
    recommendations = []
    if not will_fit:
        recommendations.append(f"Estimated memory ({{total_gb:.1f}}GB) exceeds available GPU memory ({{available_gb:.1f}}GB)")
        if not USE_QLORA:
            recommendations.append("Consider enabling QLoRA (4-bit quantization) to reduce memory by ~4x")
        if not GRADIENT_CHECKPOINTING:
            recommendations.append("Consider enabling gradient checkpointing to reduce memory")
        if PER_DEVICE_TRAIN_BATCH_SIZE > 1:
            recommendations.append("Consider reducing batch size")
        if not USE_8BIT_ADAM:
            recommendations.append("Consider enabling 8-bit Adam optimizer")
    
    return {{
        'model_memory_gb': base_size_gb,
        'optimizer_memory_gb': optimizer_gb,
        'gradient_memory_gb': gradient_gb,
        'total_estimated_gb': total_gb,
        'available_gpu_memory_gb': available_gb,
        'will_fit': will_fit,
        'recommendations': recommendations,
    }}

# ============================================
# Main training function
# ============================================
def main():
    print("=" * 60)
    print("Panther LoRA/QLoRA Training")
    print("=" * 60)
    
    # Check dependencies
    print("\\nChecking dependencies...")
    deps, warnings = check_dependencies()
    
    for name, version in deps.items():
        if isinstance(version, str):
            print(f"  {{name}}: {{version}}")
    
    if warnings:
        print("\\nWarnings:")
        for w in warnings:
            print(f"  - {{w}}")
    
    # Import libraries
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer, TrainingArguments
    from datasets import Dataset
    
    # Determine device
    device = "cuda" if torch.cuda.is_available() else "cpu"
    print(f"\\nDevice: {{device}}")
    if device == "cuda":
        print(f"GPU: {{torch.cuda.get_device_name(0)}}")
        print(f"GPU Memory: {{torch.cuda.get_device_properties(0).total_memory / 1024**3:.2f}} GB")
        print(f"CUDA Version: {{torch.version.cuda}}")
    
    # Load training data
    print(f"\\nLoading training data from {{TRAINING_DATA_PATH}}...")
    data = load_training_data(TRAINING_DATA_PATH, max_samples=MAX_TRAIN_SAMPLES)
    print(f"Loaded {{len(data)}} training examples")
    
    # Estimate memory
    print("\\nEstimating memory requirements...")
    memory_est = estimate_memory_requirements(BASE_MODEL, None, len(data), MAX_SEQ_LENGTH)
    print(f"  Model memory: {{memory_est['model_memory_gb']:.2f}} GB")
    print(f"  Optimizer memory: {{memory_est['optimizer_memory_gb']:.2f}} GB")
    print(f"  Gradient memory: {{memory_est['gradient_memory_gb']:.2f}} GB")
    print(f"  Total estimated: {{memory_est['total_estimated_gb']:.2f}} GB")
    if memory_est['available_gpu_memory_gb']:
        print(f"  Available GPU: {{memory_est['available_gpu_memory_gb']:.2f}} GB")
    print(f"  Will fit: {{memory_est['will_fit']}}")
    
    if memory_est['recommendations']:
        print("\\nRecommendations:")
        for rec in memory_est['recommendations']:
            print(f"  - {{rec}}")
    
    if DRY_RUN:
        print("\\n*** DRY RUN - Stopping before actual training ***")
        result = {{
            'status': 'dry_run_complete',
            'memory_estimate': memory_est,
            'num_samples': len(data),
            'config': {{
                'use_lora': USE_LORA,
                'use_qlora': USE_QLORA,
                'lora_rank': LORA_RANK,
                'batch_size': PER_DEVICE_TRAIN_BATCH_SIZE,
                'epochs': NUM_TRAIN_EPOCHS,
            }}
        }}
        print(f"DRY_RUN_RESULT:{{json.dumps(result)}}")
        return
    
    # Load tokenizer
    print(f"\\nLoading tokenizer for {{BASE_MODEL}}...")
    try:
        tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL, trust_remote_code=True)
    except Exception as e:
        print(f"Failed to load tokenizer: {{e}}")
        print("Falling back to gpt2 tokenizer...")
        tokenizer = AutoTokenizer.from_pretrained("gpt2")
    
    # Set up tokenizer
    tokenizer.padding_side = "right"
    tokenizer.truncation_side = "right"
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    
    # Prepare quantization config for QLoRA
    bnb_config = None
    actual_use_qlora = USE_QLORA and deps.get('bitsandbytes') is not None
    
    if actual_use_qlora:
        print("\\nConfiguring QLoRA (4-bit quantization)...")
        from transformers import BitsAndBytesConfig
        bnb_config = BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=torch.float16 if FP16 else torch.bfloat16 if BF16 else torch.float32,
            bnb_4bit_use_double_quant=True,
        )
    
    # Load model
    print(f"\\nLoading model {{BASE_MODEL}}...")
    try:
        model = AutoModelForCausalLM.from_pretrained(
            BASE_MODEL,
            quantization_config=bnb_config,
            device_map="auto" if device == "cuda" else None,
            trust_remote_code=True,
            torch_dtype=torch.float16 if FP16 else torch.bfloat16 if BF16 else torch.float32,
        )
    except Exception as e:
        print(f"Failed to load model {{BASE_MODEL}}: {{e}}")
        print("Falling back to gpt2...")
        model = AutoModelForCausalLM.from_pretrained(
            "gpt2",
            quantization_config=bnb_config,
            device_map="auto" if device == "cuda" else None,
            torch_dtype=torch.float16 if FP16 else torch.bfloat16 if BF16 else torch.float32,
        )
    
    if GRADIENT_CHECKPOINTING:
        print("Enabling gradient checkpointing...")
        model.gradient_checkpointing_enable()
    
    # Apply LoRA if enabled
    if USE_LORA or actual_use_qlora:
        print("\\nApplying LoRA configuration...")
        from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training, TaskType
        
        if actual_use_qlora:
            print("Preparing model for k-bit training...")
            model = prepare_model_for_kbit_training(model)
        
        # Determine target modules
        target_mods = TARGET_MODULES if TARGET_MODULES else None
        
        # Get task type
        task_type_map = {{
            "CAUSAL_LM": TaskType.CAUSAL_LM,
            "SEQ_2_SEQ_LM": TaskType.SEQ_2_SEQ_LM,
            "TOKEN_CLS": TaskType.TOKEN_CLS,
            "SEQ_CLS": TaskType.SEQ_CLS,
        }}
        task_type = task_type_map.get(LORA_TASK_TYPE, TaskType.CAUSAL_LM)
        
        lora_config = LoraConfig(
            r=LORA_RANK,
            lora_alpha=LORA_ALPHA,
            lora_dropout=LORA_DROPOUT,
            target_modules=target_mods,
            bias=LORA_BIAS,
            task_type=task_type,
        )
        
        model = get_peft_model(model, lora_config)
        model.print_trainable_parameters()
    
    # Format and prepare dataset
    print("\\nPreparing dataset...")
    formatted_data = []
    for example in data:
        text = format_training_example(example, DATASET_FORMAT)
        formatted_data.append({{"text": text}})
    
    dataset = Dataset.from_list(formatted_data)
    
    # Split for evaluation if requested
    if EVAL_SPLIT_RATIO > 0:
        print(f"Splitting dataset ({{1-EVAL_SPLIT_RATIO:.0%}} train, {{EVAL_SPLIT_RATIO:.0%}} eval)...")
        split = dataset.train_test_split(test_size=EVAL_SPLIT_RATIO)
        train_dataset = split['train']
        eval_dataset = split['test']
        print(f"  Train: {{len(train_dataset)}} samples")
        print(f"  Eval: {{len(eval_dataset)}} samples")
    else:
        train_dataset = dataset
        eval_dataset = None
    
    # Tokenize function
    def tokenize_function(examples):
        tokenized = tokenizer(
            examples["text"],
            truncation=True,
            padding="max_length",
            max_length=MAX_SEQ_LENGTH,
            return_tensors=None,
        )
        tokenized["labels"] = tokenized["input_ids"].copy()
        return tokenized
    
    print("Tokenizing dataset...")
    train_dataset = train_dataset.map(tokenize_function, batched=True, remove_columns=["text"])
    if eval_dataset:
        eval_dataset = eval_dataset.map(tokenize_function, batched=True, remove_columns=["text"])
    
    # Training arguments
    print("\\nConfiguring training arguments...")
    
    # Handle transformers API changes
    import transformers
    from packaging import version
    
    training_args_kwargs = {{
        "output_dir": OUTPUT_PATH,
        "num_train_epochs": NUM_TRAIN_EPOCHS,
        "per_device_train_batch_size": PER_DEVICE_TRAIN_BATCH_SIZE,
        "gradient_accumulation_steps": GRADIENT_ACCUMULATION_STEPS,
        "learning_rate": LEARNING_RATE,
        "warmup_ratio": WARMUP_RATIO,
        "weight_decay": WEIGHT_DECAY,
        "fp16": FP16 and device == "cuda",
        "bf16": BF16 and device == "cuda" and torch.cuda.is_bf16_supported(),
        "logging_steps": LOGGING_STEPS,
        "save_steps": SAVE_STEPS,
        "save_total_limit": SAVE_TOTAL_LIMIT,
        "report_to": [],
        "remove_unused_columns": False,
        "dataloader_pin_memory": device == "cuda",
    }}
    
    # Handle API change: evaluation_strategy -> eval_strategy
    try:
        if version.parse(transformers.__version__) >= version.parse("4.46.0"):
            training_args_kwargs["eval_strategy"] = "steps" if eval_dataset else "no"
        else:
            training_args_kwargs["evaluation_strategy"] = "steps" if eval_dataset else "no"
    except:
        training_args_kwargs["eval_strategy"] = "steps" if eval_dataset else "no"
    
    if eval_dataset:
        training_args_kwargs["eval_steps"] = SAVE_STEPS
        training_args_kwargs["load_best_model_at_end"] = True
    
    # 8-bit Adam optimizer
    if USE_8BIT_ADAM and deps.get('bitsandbytes'):
        training_args_kwargs["optim"] = "adamw_bnb_8bit"
    
    training_args = TrainingArguments(**training_args_kwargs)
    
    # Create trainer with progress callback
    from transformers import Trainer, TrainerCallback
    import time
    
    class ProgressCallback(TrainerCallback):
        def __init__(self):
            self.start_time = None
            
        def on_train_begin(self, args, state, control, **kwargs):
            self.start_time = time.time()
            print(f"PROGRESS:0:0:Training started. Total steps: {{state.max_steps}}")
            
        def on_log(self, args, state, control, logs=None, **kwargs):
            if state.global_step > 0 and state.max_steps > 0:
                progress_pct = min(100, int((state.global_step / state.max_steps) * 100))
                elapsed = time.time() - self.start_time if self.start_time else 0
                
                if elapsed > 0:
                    avg_time_per_step = elapsed / state.global_step
                    remaining_steps = state.max_steps - state.global_step
                    eta_seconds = int(remaining_steps * avg_time_per_step)
                    eta_str = f"{{eta_seconds // 60}}m{{eta_seconds % 60}}s"
                else:
                    eta_str = "calculating..."
                
                loss = logs.get('loss', 0) if logs else 0
                lr = logs.get('learning_rate', 0) if logs else 0
                
                print(f"PROGRESS:{{progress_pct}}:{{eta_str}}:Step {{state.global_step}}/{{state.max_steps}}, Loss: {{loss:.4f}}, LR: {{lr:.2e}}")
                
                # GPU memory monitoring
                if torch.cuda.is_available():
                    gpu_mem = torch.cuda.memory_allocated(0) / 1024**3
                    gpu_total = torch.cuda.get_device_properties(0).total_memory / 1024**3
                    print(f"RESOURCE:GPU:{{gpu_mem:.2f}}GB/{{gpu_total:.2f}}GB")
    
    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=eval_dataset,
        callbacks=[ProgressCallback()],
    )
    
    # Train
    print("\\nStarting training...")
    print("=" * 60)
    
    try:
        trainer.train()
    except Exception as e:
        print(f"\\nERROR during training: {{e}}")
        # Save checkpoint even on error
        try:
            error_checkpoint_path = os.path.join(OUTPUT_PATH, "error_checkpoint")
            model.save_pretrained(error_checkpoint_path)
            print(f"Saved error checkpoint to {{error_checkpoint_path}}")
        except:
            pass
        raise
    
    print("\\nTraining complete!")
    
    # Save final model
    print(f"Saving model to {{OUTPUT_PATH}}...")
    model.save_pretrained(OUTPUT_PATH)
    tokenizer.save_pretrained(OUTPUT_PATH)
    
    # Save training config for reproducibility (Phase 4: versioning)
    import datetime
    config_path = os.path.join(OUTPUT_PATH, "training_config.json")
    with open(config_path, 'w') as f:
        json.dump({{
            'base_model': BASE_MODEL,
            'adapter_version': datetime.datetime.utcnow().isoformat() + 'Z',
            'use_lora': USE_LORA,
            'use_qlora': actual_use_qlora,
            'lora_rank': LORA_RANK,
            'lora_alpha': LORA_ALPHA,
            'lora_dropout': LORA_DROPOUT,
            'target_modules': TARGET_MODULES,
            'num_train_epochs': NUM_TRAIN_EPOCHS,
            'learning_rate': LEARNING_RATE,
            'batch_size': PER_DEVICE_TRAIN_BATCH_SIZE,
            'max_seq_length': MAX_SEQ_LENGTH,
            'num_samples': len(data),
            'dependencies': deps,
            'tool_versions': {{k: v for k, v in deps.items() if isinstance(v, str)}},
        }}, f, indent=2)
    print(f"Saved training config to {{config_path}}")
    
    print("\\nTRAINING_COMPLETE")
    print(f"Model saved to: {{OUTPUT_PATH}}")

if __name__ == "__main__":
    main()
"####,
        base_model = base_model,
        training_data_path = training_data_path,
        output_path = output_path,
        num_train_epochs = config.num_train_epochs,
        learning_rate = config.learning_rate,
        per_device_train_batch_size = config.per_device_train_batch_size,
        gradient_accumulation_steps = config.gradient_accumulation_steps,
        warmup_ratio = config.warmup_ratio,
        weight_decay = config.weight_decay,
        max_seq_length = config.max_seq_length,
        fp16 = if config.fp16 { "True" } else { "False" },
        bf16 = if config.bf16 { "True" } else { "False" },
        save_steps = config.save_steps,
        logging_steps = config.logging_steps,
        save_total_limit = config.save_total_limit,
        use_8bit_adam = if config.use_8bit_adam { "True" } else { "False" },
        gradient_checkpointing = if config.gradient_checkpointing { "True" } else { "False" },
        dry_run = if config.dry_run { "True" } else { "False" },
        dataset_format = config.dataset_format,
        eval_split_ratio = config.eval_split_ratio,
        use_lora = if lora_config.use_lora { "True" } else { "False" },
        use_qlora = if lora_config.use_qlora { "True" } else { "False" },
        lora_rank = lora_config.lora_rank,
        lora_alpha = lora_config.lora_alpha,
        lora_dropout = lora_config.lora_dropout,
        target_modules = target_modules_json,
        lora_bias = lora_config.bias,
        lora_task_type = lora_config.task_type,
        max_train_samples = config.max_train_samples.map(|v| v.to_string()).unwrap_or("None".to_string()),
        max_tokens_per_sample = config.max_tokens_per_sample.map(|v| v.to_string()).unwrap_or("None".to_string()),
    )
}

/// Start LoRA/QLoRA training with enhanced configuration
#[tauri::command]
pub async fn start_lora_training(
    db: State<'_, Database>,
    request: StartLoraTrainingRequest,
    processes: State<'_, Arc<Mutex<HashMap<String, u32>>>>,
) -> Result<String, String> {
    use std::process::Command;
    use std::fs;
    use std::path::PathBuf;
    
    let model_id = request.model_id.clone();
    let project_id = request.project_id.clone();
    let base_model = request.base_model.clone();
    let config = request.config.clone();
    
    // Get training data path with caching
    let (training_data_path, model_output_path, training_data_count) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        // Get training data count
        let training_data_count: i32 = conn_guard
            .query_row(
                "SELECT COUNT(*) FROM training_data WHERE project_id = ?1 AND (local_model_id = ?2 OR local_model_id IS NULL)",
                rusqlite::params![&project_id, &model_id],
                |row| Ok(row.get(0)?),
            )
            .map_err(|e| format!("Database error: {}", e))?;
        
        if training_data_count == 0 {
            return Err("No training data found. Please import training data first.".to_string());
        }
        
        // Initialize cache
        let cache = TrainingCache::new((*db).clone());
        
        // Compute hash of training data
        let data_hash = {
            let mut stmt = conn_guard
                .prepare("SELECT input_text, output_text FROM training_data WHERE project_id = ?1 AND (local_model_id = ?2 OR local_model_id IS NULL) ORDER BY id")
                .map_err(|e| format!("Database error: {}", e))?;
            
            let mut hasher = Sha256::new();
            let rows = stmt
                .query_map(rusqlite::params![&project_id, &model_id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(|e| format!("Database error: {}", e))?;
            
            for row in rows {
                let (input, output) = row.map_err(|e| format!("Row error: {}", e))?;
                hasher.update(input.as_bytes());
                hasher.update(b"\0");
                hasher.update(output.as_bytes());
                hasher.update(b"\n");
            }
            
            format!("{:x}", hasher.finalize())
        };
        
        // Load settings for compression preference
        let settings = crate::commands_settings::load_settings_sync(&db);
        let use_compression = settings.cache.enable_compression;
        
        // Check/create cached training data file
        let training_data_file = match cache.get_cached_file(&project_id, &model_id, &data_hash)? {
            Some(cached_path) => cached_path,
            None => {
                let cache_file_path = cache::get_cache_file_path(&project_id, &model_id, &data_hash, use_compression)
                    .map_err(|e| format!("Failed to get cache path: {}", e))?;
                
                if let Some(parent) = cache_file_path.parent() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create cache directory: {}", e))?;
                }
                
                stream_training_data_to_file(&db, &conn_guard, &project_id, &model_id, &cache_file_path, use_compression)
                    .map_err(|e| format!("Failed to stream to cache: {}", e))?;
                
                cache.store_cache_entry(&project_id, &model_id, &data_hash, &cache_file_path)
                    .map_err(|e| format!("Failed to store cache entry: {}", e))?;
                
                cache_file_path
            }
        };
        
        // Model output path
        let app_data_dir = std::env::var("APPDATA")
            .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
            .unwrap_or_else(|_| ".".to_string());
        let training_dir = PathBuf::from(&app_data_dir).join("panther").join("lora_training");
        fs::create_dir_all(&training_dir).map_err(|e| format!("Failed to create training directory: {}", e))?;
        let model_output_path = training_dir.join(&model_id);
        
        (training_data_file, model_output_path, training_data_count)
    };
    
    // Update model status to training
    {
        let conn = db.get_connection();
        conn.lock()
            .map_err(|e| format!("Database lock error: {}", e))?
            .execute(
                "UPDATE local_models SET training_status = ?1, training_config_json = ?2, updated_at = ?3 WHERE id = ?4",
                rusqlite::params![
                    "training",
                    serde_json::to_string(&config).unwrap_or_default(),
                    Utc::now().to_rfc3339(),
                    &model_id
                ],
            )
            .map_err(|e| format!("Database error: {}", e))?;
    }
    
    // Find Python command
    let python_cmd = {
        let mut found = None;
        
        #[cfg(windows)]
        {
            for version in &["3.11", "3.12", "3.10"] {
                if Command::new("py").arg(format!("-{}", version)).arg("--version").output().is_ok() {
                    let check = Command::new("py")
                        .arg(format!("-{}", version))
                        .arg("-c")
                        .arg("import transformers, peft")
                        .output();
                    if check.is_ok() && check.unwrap().status.success() {
                        found = Some(format!("py -{}", version));
                        break;
                    }
                }
            }
            
            if found.is_none() && Command::new("py").arg("-3").arg("--version").output().is_ok() {
                let check = Command::new("py").arg("-3").arg("-c").arg("import transformers, peft").output();
                if check.is_ok() && check.unwrap().status.success() {
                    found = Some("py -3".to_string());
                }
            }
        }
        
        if found.is_none() {
            for cmd in &["python3", "python"] {
                if Command::new(cmd).arg("--version").output().is_ok() {
                    let check = Command::new(cmd).arg("-c").arg("import transformers, peft").output();
                    if check.is_ok() && check.unwrap().status.success() {
                        found = Some(cmd.to_string());
                        break;
                    }
                }
            }
        }
        
        found
    };
    
    let python_cmd = match python_cmd {
        Some(cmd) => cmd,
        None => {
            let conn = db.get_connection();
            if let Ok(conn_guard) = conn.lock() {
                let error_msg = "Python with transformers and peft libraries not found.\n\nTo fix:\n1. Go to Settings > Dependencies\n2. Install missing packages (transformers, peft, datasets, torch)\n3. Or run: pip install transformers peft datasets torch accelerate";
                let metrics = serde_json::json!({
                    "error": error_msg,
                    "status": "failed",
                    "error_type": "dependencies_not_found"
                });
                let _ = conn_guard.execute(
                    "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                    rusqlite::params!["failed", serde_json::to_string(&metrics).unwrap_or_default(), &model_id],
                );
            }
            return Err("Python with transformers and peft not found. Check Settings > Dependencies.".to_string());
        }
    };
    
    // Generate training script
    let script_content = generate_lora_training_script(
        &base_model,
        &training_data_path.to_string_lossy(),
        &model_output_path.to_string_lossy(),
        &config,
    );
    
    // Write script to temp file
    let app_data_dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
        .unwrap_or_else(|_| ".".to_string());
    let scripts_dir = PathBuf::from(&app_data_dir).join("panther").join("scripts");
    fs::create_dir_all(&scripts_dir).map_err(|e| format!("Failed to create scripts directory: {}", e))?;
    
    let script_path = scripts_dir.join(format!("lora_training_{}.py", model_id));
    fs::write(&script_path, &script_content)
        .map_err(|e| format!("Failed to write training script: {}", e))?;
    
    // Spawn training process
    let model_id_clone = model_id.clone();
    let db_clone: Database = (*db).clone();
    let script_path_str = script_path.to_string_lossy().to_string();
    let processes_clone = processes.inner().clone();
    
    std::thread::spawn(move || {
        // Build command
        let mut cmd = if python_cmd.starts_with("py ") {
            let parts: Vec<&str> = python_cmd.split_whitespace().collect();
            let mut c = Command::new(parts[0]);
            for p in &parts[1..] {
                c.arg(p);
            }
            c.arg(&script_path_str);
            c
        } else {
            let mut c = Command::new(&python_cmd);
            c.arg(&script_path_str);
            c
        };
        
        cmd.stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                let conn = db_clone.get_connection();
                if let Ok(conn_guard) = conn.lock() {
                    let metrics = serde_json::json!({
                        "error": format!("Failed to spawn training process: {}", e),
                        "status": "failed"
                    });
                    let _ = conn_guard.execute(
                        "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                        rusqlite::params!["failed", serde_json::to_string(&metrics).unwrap_or_default(), &model_id_clone],
                    );
                }
                return;
            }
        };
        
        // Store process ID
        if let Ok(mut procs) = processes_clone.lock() {
            procs.insert(model_id_clone.clone(), child.id());
        }
        
        // Read output and update progress
        use std::io::BufRead;
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        
        let mut last_progress = 0;
        let mut last_loss: Option<f64> = None;
        let mut training_complete = false;
        let mut error_output = String::new();
        
        if let Some(stdout) = stdout {
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines() {
                if let Ok(line) = line {
                    println!("{}", line); // Log to console
                    
                    // Parse progress updates
                    if line.starts_with("PROGRESS:") {
                        let parts: Vec<&str> = line[9..].split(':').collect();
                        if parts.len() >= 3 {
                            if let Ok(pct) = parts[0].parse::<i32>() {
                                last_progress = pct;
                                // Extract loss if present
                                if let Some(loss_part) = parts.get(2) {
                                    if let Some(loss_str) = loss_part.split("Loss: ").nth(1) {
                                        if let Ok(loss) = loss_str.split(',').next().unwrap_or("").parse::<f64>() {
                                            last_loss = Some(loss);
                                        }
                                    }
                                }
                            }
                        }
                        
                        // Update database with progress
                        {
                            let conn = db_clone.get_connection();
                            let lock_result = conn.lock();
                            if let Ok(conn_guard) = lock_result {
                                let metrics = serde_json::json!({
                                    "progress": last_progress,
                                    "loss": last_loss,
                                    "status": "training"
                                });
                                let _ = conn_guard.execute(
                                    "UPDATE local_models SET training_metrics_json = ?1 WHERE id = ?2",
                                    rusqlite::params![serde_json::to_string(&metrics).unwrap_or_default(), &model_id_clone],
                                );
                            }
                        }
                    }
                    
                    // Check for completion
                    if line.contains("TRAINING_COMPLETE") {
                        training_complete = true;
                    }
                    
                    // Check for dry run result
                    if line.starts_with("DRY_RUN_RESULT:") {
                        training_complete = true;
                        // Parse and store dry run results
                        {
                            let conn = db_clone.get_connection();
                            let lock_result = conn.lock();
                            if let Ok(conn_guard) = lock_result {
                                let result_json = &line[15..];
                                let metrics = serde_json::json!({
                                    "dry_run": true,
                                    "result": serde_json::from_str::<serde_json::Value>(result_json).unwrap_or_default()
                                });
                                let _ = conn_guard.execute(
                                    "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                                    rusqlite::params!["dry_run_complete", serde_json::to_string(&metrics).unwrap_or_default(), &model_id_clone],
                                );
                            }
                        }
                    }
                }
            }
        }
        
        // Capture stderr
        if let Some(stderr) = stderr {
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                if let Ok(line) = line {
                    eprintln!("{}", line);
                    error_output.push_str(&line);
                    error_output.push('\n');
                }
            }
        }
        
        // Wait for process to finish
        let status = child.wait();
        
        // Remove from process list
        if let Ok(mut procs) = processes_clone.lock() {
            procs.remove(&model_id_clone);
        }
        
        // Update final status
        {
            let conn = db_clone.get_connection();
            let lock_result = conn.lock();
            if let Ok(conn_guard) = lock_result {
                let final_status = if training_complete {
                    "completed"
                } else if status.map(|s| s.success()).unwrap_or(false) {
                    "completed"
                } else {
                    "failed"
                };
                
                let metrics = if final_status == "completed" {
                    serde_json::json!({
                        "progress": 100,
                        "loss": last_loss,
                        "status": "completed"
                    })
                } else {
                    serde_json::json!({
                        "error": error_output,
                        "status": "failed"
                    })
                };
                
                let _ = conn_guard.execute(
                    "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2, updated_at = ?3 WHERE id = ?4",
                    rusqlite::params![
                        final_status,
                        serde_json::to_string(&metrics).unwrap_or_default(),
                        Utc::now().to_rfc3339(),
                        &model_id_clone
                    ],
                );
            }
        }
    });
    
    Ok(format!("LoRA training started for model {}. {} training samples loaded.", model_id, training_data_count))
}

#[tauri::command]
pub async fn get_training_progress(
    db: State<'_, Database>,
    model_id: String,
) -> Result<serde_json::Value, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let (status, metrics_json): (String, Option<String>) = conn_guard
        .query_row(
            "SELECT training_status, training_metrics_json FROM local_models WHERE id = ?1",
            [&model_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| format!("Failed to get model: {}", e))?;
    
    let metrics = if let Some(metrics_str) = metrics_json {
        serde_json::from_str(&metrics_str).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };
    
    Ok(serde_json::json!({
        "status": status,
        "metrics": metrics,
    }))
}

#[tauri::command]
pub async fn stop_training(
    model_id: String,
    processes: State<'_, Arc<Mutex<HashMap<String, u32>>>>,
    db: State<'_, Database>,
) -> Result<String, String> {
    use std::process::Command;
    
    // Get process ID
    let pid = {
        if let Ok(procs) = processes.lock() {
            procs.get(&model_id).copied()
        } else {
            None
        }
    };
    
    if let Some(pid) = pid {
        // Kill the process
        #[cfg(windows)]
        {
            let _ = Command::new("taskkill")
                .args(&["/F", "/PID", &pid.to_string()])
                .output();
        }
        #[cfg(not(windows))]
        {
            let _ = Command::new("kill")
                .args(&["-9", &pid.to_string()])
                .output();
        }
        
        // Update database
        let conn = db.get_connection();
        if let Ok(conn_guard) = conn.lock() {
            let metrics = serde_json::json!({
                "status": "cancelled",
                "error": "Training cancelled by user"
            });
            let _ = conn_guard.execute(
                "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                rusqlite::params!["failed", serde_json::to_string(&metrics).unwrap_or_default(), &model_id],
            );
        }
        
        // Remove from processes map
        if let Ok(mut procs) = processes.lock() {
            procs.remove(&model_id);
        }
        
        Ok(format!("Training stopped for model {} (PID: {})", model_id, pid))
    } else {
        // Check if the model status is actually "training" - if so, reset it
        let conn = db.get_connection();
        if let Ok(conn_guard) = conn.lock() {
            let current_status: Result<String, _> = conn_guard.query_row(
                "SELECT training_status FROM local_models WHERE id = ?1",
                [&model_id],
                |row| row.get(0)
            );
            
            if let Ok(status) = current_status {
                if status == "training" {
                    // Process not found but status says training - likely a stale state
                    let metrics = serde_json::json!({
                        "error": "Training process not found. It may have already completed or crashed.",
                        "status": "failed"
                    });
                    let _ = conn_guard.execute(
                        "UPDATE local_models SET training_status = ?1, training_metrics_json = ?2 WHERE id = ?3",
                        rusqlite::params!["failed", serde_json::to_string(&metrics).unwrap_or_default(), &model_id],
                    );
                    return Ok(format!("No active process found for model {}. Status has been reset to 'failed'.", model_id));
                }
            }
        }
        
        Err(format!("No active training process found for model {}. The process may have already completed or was never started.", model_id))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatWithTrainingDataRequest {
    pub project_id: String,
    pub local_model_id: Option<String>,
    pub query: String,
    pub profile_id: Option<String>, // Optional profile to use for chat
    pub max_examples: Option<usize>, // Max number of training examples to use as context
    pub use_local: Option<bool>, // If true, prefer/require local Ollama models
    pub local_model_name: Option<String>, // Specific local model name to use (e.g., "llama3", "mistral")
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatWithTrainingDataResponse {
    pub response: String,
    pub examples_used: Vec<serde_json::Value>,
    pub example_count: usize,
    pub provider_used: String, // Which provider was actually used
    pub model_used: String, // Which model was actually used
}

#[tauri::command]
pub async fn chat_with_training_data(
    db: State<'_, Database>,
    request: ChatWithTrainingDataRequest,
) -> Result<ChatWithTrainingDataResponse, String> {
    use crate::providers::get_adapter;
    
    // Collect all data synchronously before any async operations
    let (examples, provider_account, model_name, prompt) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        let max_examples = request.max_examples.unwrap_or(5);
        
        // Collect examples
        let examples: Vec<(String, String, Option<String>)> = if let Some(model_id) = &request.local_model_id {
            // Query with model_id filter
            conn_guard
                .prepare("SELECT input_text, output_text, metadata_json FROM training_data WHERE project_id = ?1 AND (local_model_id = ?2 OR local_model_id IS NULL) ORDER BY created_at DESC LIMIT ?3")
                .and_then(|mut stmt| {
                    stmt.query_map(rusqlite::params![&request.project_id, model_id, max_examples as i32], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    })
                    .and_then(|iter| {
                        iter.collect::<Result<Vec<_>, _>>()
                    })
                })
                .map_err(|e: rusqlite::Error| format!("Database error: {}", e))?
        } else {
            // Query without model_id filter
            conn_guard
                .prepare("SELECT input_text, output_text, metadata_json FROM training_data WHERE project_id = ?1 ORDER BY created_at DESC LIMIT ?2")
                .and_then(|mut stmt| {
                    stmt.query_map(rusqlite::params![&request.project_id, max_examples as i32], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, Option<String>>(2)?,
                        ))
                    })
                    .and_then(|iter| {
                        iter.collect::<Result<Vec<_>, _>>()
                    })
                })
                .map_err(|e: rusqlite::Error| format!("Database error: {}", e))?
        };
        
        if examples.is_empty() {
            return Err("No training data found for this project. Please import training data first.".to_string());
        }
        
        // Build context from examples
        let context = examples.iter()
            .enumerate()
            .map(|(i, (input, output, _))| {
                format!("Example {}:\nInput: {}\nOutput: {}", i + 1, input, output)
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        
        // Build prompt
        let prompt = format!(
            "Based on the following training examples from this project, please answer the user's question.\n\n\
            Training Examples:\n{}\n\n\
            User Question: {}\n\n\
            Please provide a helpful answer based on the patterns and information shown in the training examples.",
            context,
            request.query
        );
        
        // Get a provider/profile to use
        let (provider_account, model_name) = if let Some(profile_id) = &request.profile_id {
            // Use specified profile
            let profile: (String, Option<String>) = conn_guard
                .query_row(
                    "SELECT provider_account_id, model_name FROM profiles WHERE id = ?1",
                    [profile_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|e| format!("Failed to get profile: {}", e))?;
            
            let provider: (String, String, Option<String>, Option<String>) = conn_guard
                .query_row(
                    "SELECT provider_type, display_name, base_url, provider_metadata_json FROM provider_accounts WHERE id = ?1",
                    [&profile.0],
                    |row| Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                    )),
                )
                .map_err(|e| format!("Failed to get provider: {}", e))?;
            
            let provider_metadata = provider.3
                .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
            
            let account = ProviderAccount {
                id: profile.0,
                provider_type: provider.0,
                display_name: provider.1,
                base_url: provider.2,
                region: None,
                auth_ref: None,
                provider_metadata_json: provider_metadata,
                created_at: String::new(),
                updated_at: String::new(),
            };
            
            (account, profile.1.unwrap_or_else(|| "gpt-3.5-turbo".to_string()))
        } else {
            // Check if user wants to use local models only
            let prefer_local = request.use_local.unwrap_or(false);
            
            // Find provider - prefer Ollama if use_local is true, otherwise try any
            let mut provider_result: Result<(String, String, String, Option<String>, Option<String>), _> = 
                conn_guard
                    .query_row(
                        "SELECT id, provider_type, display_name, base_url, provider_metadata_json FROM provider_accounts WHERE provider_type = 'ollama' LIMIT 1",
                        [],
                        |row| Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        )),
                    );
            
            // If no Ollama and not requiring local, try any provider
            if provider_result.is_err() && !prefer_local {
                provider_result = conn_guard
                    .query_row(
                        "SELECT id, provider_type, display_name, base_url, provider_metadata_json FROM provider_accounts LIMIT 1",
                        [],
                        |row| Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                        )),
                    );
            }
            
            if let Ok(provider) = provider_result {
                let provider_metadata = provider.4
                    .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok());
                
                // Determine default model based on provider type and user preference
                let default_model = if provider.1 == "ollama" {
                    // Use specified local model name or default to llama3
                    request.local_model_name.unwrap_or_else(|| "llama3".to_string())
                } else if provider.1 == "openai_compatible" || provider.1 == "openai" {
                    "gpt-3.5-turbo".to_string()
                } else if provider.1 == "anthropic" {
                    "claude-3-haiku-20240307".to_string()
                } else if provider.1 == "google" {
                    "gemini-pro".to_string()
                } else {
                    "gpt-3.5-turbo".to_string() // Default fallback
                };
                
                let account = ProviderAccount {
                    id: provider.0.clone(),
                    provider_type: provider.1,
                    display_name: provider.2,
                    base_url: provider.3,
                    region: None,
                    auth_ref: None,
                    provider_metadata_json: provider_metadata,
                    created_at: String::new(),
                    updated_at: String::new(),
                };
                
                (account, default_model)
            } else {
                if prefer_local {
                    // Create a default Ollama provider account on-the-fly
                    // This allows using Ollama without requiring setup in Settings
                    let default_model = request.local_model_name.unwrap_or_else(|| "llama3".to_string());
                    
                    let account = ProviderAccount {
                        id: Uuid::new_v4().to_string(),
                        provider_type: "ollama".to_string(),
                        display_name: "Local Ollama (Default)".to_string(),
                        base_url: Some("http://localhost:11434".to_string()), // Default Ollama URL
                        region: None,
                        auth_ref: None,
                        provider_metadata_json: None,
                        created_at: chrono::Utc::now().to_rfc3339(),
                        updated_at: chrono::Utc::now().to_rfc3339(),
                    };
                    
                    (account, default_model)
                } else {
                    return Err("No LLM provider available. Please configure a provider in Settings (OpenAI, Anthropic, Google, or Ollama).".to_string());
                }
            }
        };
        
        // All database operations complete, return all collected data
        (examples, provider_account, model_name, prompt)
    };
    
    // Use the adapter to get response
    let adapter = get_adapter(&provider_account.provider_type)
        .map_err(|e| format!("Unsupported provider type: {}", e))?;
    
    let packet = PromptPacket {
        global_instructions: Some("You are a helpful assistant that answers questions based on training examples.".to_string()),
        persona_instructions: String::new(),
        user_message: prompt,
        conversation_context: None,
        params_json: serde_json::json!({
            "temperature": 0.7,
            "max_tokens": 1000,
        }),
        stream: false,
    };
    
    let response = adapter.complete(&packet, &provider_account, &model_name).await
        .map_err(|e| format!("Failed to get LLM response: {}", e))?;

    // Record token usage for training-data chat
    let _ = record_token_usage(
        &db,
        Some(&provider_account.id),
        &model_name,
        &response.usage_json,
        "training_chat",
        None,
        None,
    );
    
    // Format examples for response
    let examples_used: Vec<serde_json::Value> = examples.iter()
        .map(|(input, output, metadata)| {
            serde_json::json!({
                "input": input,
                "output": output,
                "metadata": metadata.as_ref().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()),
            })
        })
        .collect();
    
    Ok(ChatWithTrainingDataResponse {
        response: response.text,
        examples_used,
        example_count: examples.len(),
        provider_used: provider_account.display_name.clone(),
        model_used: model_name.clone(),
    })
}

#[tauri::command]
pub async fn clear_training_cache(
    db: State<'_, Database>,
    project_id: Option<String>,
    model_id: Option<String>,
) -> Result<serde_json::Value, String> {
    let cache = TrainingCache::new((*db).clone());
    
    if let Some(pid) = project_id {
        cache.invalidate_cache(&pid, model_id.as_deref())
            .map_err(|e| format!("Failed to clear cache: {}", e))?;
        Ok(serde_json::json!({
            "success": true,
            "message": format!("Cache cleared for project {}", pid)
        }))
    } else {
        // Clear all cache entries
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        // Get all cache entries
        let mut stmt = conn_guard
            .prepare("SELECT id, file_path FROM training_data_cache")
            .map_err(|e| format!("Database error: {}", e))?;
        
        let entries: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(|e| format!("Database error: {}", e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("Row error: {}", e))?;
        
        // Delete files
        let mut deleted_count = 0;
        for (_, file_path) in &entries {
            let path = std::path::PathBuf::from(file_path);
            if path.exists() {
                std::fs::remove_file(&path).ok();
                deleted_count += 1;
            }
        }
        
        // Delete from database
        conn_guard.execute("DELETE FROM training_data_cache", [])
            .map_err(|e| format!("Database error: {}", e))?;
        
        Ok(serde_json::json!({
            "success": true,
            "message": format!("Cleared all cache entries ({} files deleted)", deleted_count)
        }))
    }
}

#[tauri::command]
pub async fn get_training_cache_stats(
    db: State<'_, Database>,
) -> Result<serde_json::Value, String> {
    let cache = TrainingCache::new((*db).clone());
    let stats = cache.get_cache_stats()
        .map_err(|e| format!("Failed to get cache stats: {}", e))?;
    
    Ok(serde_json::json!({
        "total_entries": stats.total_entries,
        "total_size_bytes": stats.total_size_bytes,
        "total_size_gb": (stats.total_size_bytes as f64) / (1024.0 * 1024.0 * 1024.0),
        "max_size_bytes": stats.max_size_bytes,
        "max_size_gb": (stats.max_size_bytes as f64) / (1024.0 * 1024.0 * 1024.0),
        "usage_percent": (stats.total_size_bytes as f64 / stats.max_size_bytes as f64) * 100.0
    }))
}

// ===============================================
// Model Export Functions
// ===============================================

/// Request for exporting a trained model
#[derive(Debug, Serialize, Deserialize)]
pub struct ExportModelRequest {
    pub model_id: String,
    pub export_format: String, // "huggingface", "gguf", "ollama"
    pub export_path: Option<String>, // Custom export path, or None for default
    pub merge_adapters: bool, // For LoRA: merge adapters into base model
    pub quantization: Option<String>, // For GGUF: "q4_k_m", "q5_k_m", etc.
    pub ollama_model_name: Option<String>, // For Ollama: custom model name
    pub ollama_system_prompt: Option<String>, // For Ollama: system prompt for Modelfile
}

/// Export progress update
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportProgress {
    pub status: String,
    pub progress_percent: f32,
    pub current_step: String,
    pub error: Option<String>,
}

/// Get the model's training output path
fn get_model_training_path(model_id: &str) -> std::path::PathBuf {
    let app_data_dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("HOME").map(|h| format!("{}/.local/share", h)))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(&app_data_dir)
        .join("panther")
        .join("lora_training")
        .join(model_id)
}

/// Check if llama.cpp convert script is available
fn find_llama_cpp_convert() -> Option<std::path::PathBuf> {
    use std::process::Command;
    
    // Common locations for llama.cpp
    let possible_paths = [
        "llama.cpp/convert.py",
        "../llama.cpp/convert.py",
        "~/llama.cpp/convert.py",
        "/opt/llama.cpp/convert.py",
        "C:\\llama.cpp\\convert.py",
    ];
    
    for path in possible_paths {
        let expanded = shellexpand::tilde(path);
        let path_buf = std::path::PathBuf::from(expanded.as_ref());
        if path_buf.exists() {
            return Some(path_buf);
        }
    }
    
    // Try to find via which/where command
    #[cfg(windows)]
    {
        if let Ok(output) = Command::new("where").arg("llama-cpp-convert").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                return Some(std::path::PathBuf::from(path.trim()));
            }
        }
    }
    
    #[cfg(not(windows))]
    {
        if let Ok(output) = Command::new("which").arg("llama-cpp-convert").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout);
                return Some(std::path::PathBuf::from(path.trim()));
            }
        }
    }
    
    None
}

/// Export model to HuggingFace format (zip with adapters or merged model)
#[tauri::command]
pub async fn export_model_huggingface(
    db: State<'_, Database>,
    request: ExportModelRequest,
) -> Result<serde_json::Value, String> {
    use std::fs;
    use std::io::Write;
    
    let model_id = &request.model_id;
    
    // Get model info from database
    let (model_name, base_model, training_config): (String, String, Option<String>) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT name, base_model, training_config_json FROM local_models WHERE id = ?1",
                [model_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|e| format!("Model not found: {}", e))?
    };
    
    let model_path = get_model_training_path(model_id);
    
    if !model_path.exists() {
        return Err("Trained model not found. Please train the model first.".to_string());
    }
    
    // Determine export path
    let export_path = if let Some(path) = request.export_path {
        std::path::PathBuf::from(path)
    } else {
        let downloads_dir = dirs::download_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."));
        downloads_dir.join(format!("{}_hf_export.zip", model_name.replace(" ", "_")))
    };
    
    // Create export metadata
    let export_metadata = serde_json::json!({
        "model_name": model_name,
        "base_model": base_model,
        "model_id": model_id,
        "export_format": "huggingface",
        "merge_adapters": request.merge_adapters,
        "training_config": training_config.as_ref().and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok()),
        "exported_at": Utc::now().to_rfc3339(),
        "panther_version": env!("CARGO_PKG_VERSION"),
    });
    
    // Create zip file
    let file = fs::File::create(&export_path)
        .map_err(|e| format!("Failed to create export file: {}", e))?;
    let mut zip = zip::ZipWriter::new(file);
    
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    
    // Add training_config.json
    zip.start_file("training_config.json", options)
        .map_err(|e| format!("Failed to add file to zip: {}", e))?;
    zip.write_all(serde_json::to_string_pretty(&export_metadata).unwrap_or_default().as_bytes())
        .map_err(|e| format!("Failed to write to zip: {}", e))?;
    
    // Add all files from model directory
    fn add_directory_to_zip(
        zip: &mut zip::ZipWriter<std::fs::File>,
        dir: &std::path::Path,
        prefix: &str,
        options: zip::write::FileOptions,
    ) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| format!("Failed to read directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();
            let name = format!("{}/{}", prefix, path.file_name().unwrap_or_default().to_string_lossy());
            
            if path.is_dir() {
                add_directory_to_zip(zip, &path, &name, options)?;
            } else {
                zip.start_file(&name, options)
                    .map_err(|e| format!("Failed to add file to zip: {}", e))?;
                let contents = fs::read(&path)
                    .map_err(|e| format!("Failed to read file: {}", e))?;
                zip.write_all(&contents)
                    .map_err(|e| format!("Failed to write to zip: {}", e))?;
            }
        }
        Ok(())
    }
    
    add_directory_to_zip(&mut zip, &model_path, "model", options)?;
    
    zip.finish().map_err(|e| format!("Failed to finalize zip: {}", e))?;
    
    Ok(serde_json::json!({
        "success": true,
        "export_path": export_path.to_string_lossy(),
        "format": "huggingface",
        "message": format!("Model exported to {}", export_path.display())
    }))
}

/// Create Ollama Modelfile content
fn create_modelfile(
    base_model_path: &str,
    system_prompt: Option<&str>,
    parameters: Option<&serde_json::Value>,
) -> String {
    let mut content = format!("FROM {}\n\n", base_model_path);
    
    // Add system prompt if provided
    if let Some(system) = system_prompt {
        content.push_str(&format!("SYSTEM \"\"\"\n{}\n\"\"\"\n\n", system));
    }
    
    // Add default parameters
    content.push_str("PARAMETER temperature 0.7\n");
    content.push_str("PARAMETER top_p 0.9\n");
    content.push_str("PARAMETER top_k 40\n");
    content.push_str("PARAMETER repeat_penalty 1.1\n");
    
    // Add custom parameters if provided
    if let Some(params) = parameters {
        if let Some(obj) = params.as_object() {
            for (key, value) in obj {
                content.push_str(&format!("PARAMETER {} {}\n", key, value));
            }
        }
    }
    
    content
}

/// Export model to Ollama (creates model from trained weights)
#[tauri::command]
pub async fn export_model_ollama(
    db: State<'_, Database>,
    request: ExportModelRequest,
) -> Result<serde_json::Value, String> {
    use std::process::Command;
    use std::fs;
    
    let model_id = &request.model_id;
    
    // Get model info
    let (model_name, base_model): (String, String) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT name, base_model FROM local_models WHERE id = ?1",
                [model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| format!("Model not found: {}", e))?
    };
    
    let model_path = get_model_training_path(model_id);
    
    if !model_path.exists() {
        return Err("Trained model not found. Please train the model first.".to_string());
    }
    
    // Determine Ollama model name
    let ollama_name = request.ollama_model_name
        .unwrap_or_else(|| format!("panther-{}", model_name.to_lowercase().replace(" ", "-")));
    
    // Check if we need to convert to GGUF first
    let gguf_path = model_path.join("model.gguf");
    
    if !gguf_path.exists() {
        // Need to convert to GGUF first
        // For now, return an error suggesting manual conversion
        return Err(format!(
            "Model needs to be converted to GGUF format first. \
            The model is located at: {}\n\n\
            To convert manually:\n\
            1. Install llama.cpp\n\
            2. Run: python convert.py {} --outfile {}\n\n\
            Or use the 'Export to GGUF' option first.",
            model_path.display(),
            model_path.display(),
            gguf_path.display()
        ));
    }
    
    // Create Modelfile
    let modelfile_path = model_path.join("Modelfile");
    let modelfile_content = create_modelfile(
        &gguf_path.to_string_lossy(),
        request.ollama_system_prompt.as_deref(),
        None,
    );
    
    fs::write(&modelfile_path, &modelfile_content)
        .map_err(|e| format!("Failed to write Modelfile: {}", e))?;
    
    // Create Ollama model
    let output = Command::new("ollama")
        .arg("create")
        .arg(&ollama_name)
        .arg("-f")
        .arg(&modelfile_path)
        .output()
        .map_err(|e| format!("Failed to run ollama create: {}. Is Ollama installed?", e))?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Ollama create failed: {}", stderr));
    }
    
    // Verify the model was created
    let verify_output = Command::new("ollama")
        .arg("list")
        .output()
        .map_err(|e| format!("Failed to verify model: {}", e))?;
    
    let model_list = String::from_utf8_lossy(&verify_output.stdout);
    let model_created = model_list.lines().any(|line| line.contains(&ollama_name));
    
    if !model_created {
        return Err("Model was not found in Ollama after creation. Check Ollama logs.".to_string());
    }
    
    // Update database with exported model info
    {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        let export_info = serde_json::json!({
            "ollama_model_name": ollama_name,
            "exported_at": Utc::now().to_rfc3339(),
            "format": "ollama"
        });
        
        // Get existing metrics and merge
        let existing_metrics: Option<String> = conn_guard
            .query_row(
                "SELECT training_metrics_json FROM local_models WHERE id = ?1",
                [model_id],
                |row| row.get(0),
            )
            .ok();
        
        let mut metrics: serde_json::Value = existing_metrics
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        
        if let Some(obj) = metrics.as_object_mut() {
            obj.insert("export".to_string(), export_info);
        }
        
        conn_guard.execute(
            "UPDATE local_models SET training_metrics_json = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![
                serde_json::to_string(&metrics).unwrap_or_default(),
                Utc::now().to_rfc3339(),
                model_id
            ],
        ).map_err(|e| format!("Failed to update database: {}", e))?;
    }
    
    Ok(serde_json::json!({
        "success": true,
        "ollama_model_name": ollama_name,
        "modelfile_path": modelfile_path.to_string_lossy(),
        "message": format!("Model '{}' created in Ollama. Run: ollama run {}", ollama_name, ollama_name)
    }))
}

/// Get list of supported GGUF quantization formats
#[tauri::command]
pub async fn get_gguf_quantization_options() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "options": [
            {"id": "q4_0", "name": "Q4_0", "description": "4-bit quantization (smallest, fastest)", "recommended": false},
            {"id": "q4_k_m", "name": "Q4_K_M", "description": "4-bit K-quant (good balance)", "recommended": true},
            {"id": "q5_0", "name": "Q5_0", "description": "5-bit quantization", "recommended": false},
            {"id": "q5_k_m", "name": "Q5_K_M", "description": "5-bit K-quant (higher quality)", "recommended": true},
            {"id": "q6_k", "name": "Q6_K", "description": "6-bit K-quant (high quality)", "recommended": false},
            {"id": "q8_0", "name": "Q8_0", "description": "8-bit quantization (highest quality)", "recommended": false},
            {"id": "f16", "name": "F16", "description": "16-bit float (no quantization)", "recommended": false},
        ],
        "default": "q4_k_m"
    }))
}

/// Check export readiness and available options
#[tauri::command]
pub async fn check_export_options(
    db: State<'_, Database>,
    model_id: String,
) -> Result<serde_json::Value, String> {
    use std::process::Command;
    
    // Check model exists and is trained
    let (model_name, training_status): (String, String) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT name, training_status FROM local_models WHERE id = ?1",
                [&model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| format!("Model not found: {}", e))?
    };
    
    let model_path = get_model_training_path(&model_id);
    let model_exists = model_path.exists();
    
    // Check for GGUF file
    let gguf_exists = model_path.join("model.gguf").exists();
    
    // Check if llama.cpp convert is available
    let llama_cpp_available = find_llama_cpp_convert().is_some();
    
    // Check if Ollama is running
    let ollama_available = Command::new("ollama")
        .arg("list")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    
    Ok(serde_json::json!({
        "model_id": model_id,
        "model_name": model_name,
        "training_status": training_status,
        "model_path": model_path.to_string_lossy(),
        "model_exists": model_exists,
        "ready_to_export": model_exists && (training_status == "completed" || training_status == "complete"),
        "export_options": {
            "huggingface": {
                "available": model_exists,
                "description": "Export as HuggingFace format (zip with model files)"
            },
            "gguf": {
                "available": model_exists && llama_cpp_available,
                "llama_cpp_found": llama_cpp_available,
                "gguf_exists": gguf_exists,
                "description": "Convert to GGUF format for llama.cpp/Ollama"
            },
            "ollama": {
                "available": model_exists && ollama_available && gguf_exists,
                "ollama_running": ollama_available,
                "gguf_required": !gguf_exists,
                "description": "Register as Ollama model for local inference"
            }
        }
    }))
}

/// List Ollama model names that we have trained and exported (for profile editor dropdown marking)
#[tauri::command]
pub async fn list_trained_ollama_models(
    db: State<'_, Database>,
) -> Result<Vec<String>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    let mut stmt = conn_guard
        .prepare("SELECT training_metrics_json FROM local_models WHERE training_status IN ('complete', 'completed') AND training_metrics_json IS NOT NULL")
        .map_err(|e| format!("Database error: {}", e))?;
    
    let rows = stmt
        .query_map([], |row| Ok(row.get::<_, Option<String>>(0)?))
        .map_err(|e| format!("Database error: {}", e))?;
    
    let mut names = Vec::new();
    for row in rows {
        if let Ok(Some(json_str)) = row {
            if let Ok(metrics) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(export) = metrics.get("export").and_then(|e| e.as_object()) {
                    if let Some(name) = export.get("ollama_model_name").and_then(|n| n.as_str()) {
                        names.push(name.to_string());
                    }
                }
            }
        }
    }
    
    Ok(names)
}

/// Convert model to GGUF format
#[tauri::command]
pub async fn convert_model_to_gguf(
    db: State<'_, Database>,
    model_id: String,
    quantization: Option<String>,
) -> Result<serde_json::Value, String> {
    use std::process::Command;
    
    const LLAMA_FAMILY: &[&str] = &["llama", "mistral", "gemma", "phi", "qwen", "yi", "deepseek"];
    
    let (base_model, _model_name): (String, String) = {
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        
        conn_guard
            .query_row(
                "SELECT base_model, name FROM local_models WHERE id = ?1",
                [&model_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .map_err(|e| format!("Model not found: {}", e))?
    };
    
    let base_lower = base_model.to_lowercase();
    let is_supported = LLAMA_FAMILY.iter().any(|&f| base_lower.contains(f));
    if !is_supported {
        return Err(format!(
            "GGUF conversion supports Llama-family models. Base '{}' may not be compatible.",
            base_model
        ));
    }
    
    let model_path = get_model_training_path(&model_id);
    if !model_path.exists() {
        return Err("Trained model not found.".to_string());
    }
    
    let quant = quantization.as_deref().unwrap_or("q4_k_m");
    let gguf_path = model_path.join("model.gguf");
    
    if let Some(convert_script) = find_llama_cpp_convert() {
        let output = Command::new("python")
            .arg(&convert_script)
            .arg(&model_path)
            .arg("--outfile")
            .arg(&gguf_path)
            .arg("--outtype")
            .arg(quant)
            .output()
            .map_err(|e| format!("Failed to run conversion: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("GGUF conversion failed: {}", stderr));
        }
        
        let conn = db.get_connection();
        let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
        let existing: Option<String> = conn_guard
            .query_row("SELECT training_metrics_json FROM local_models WHERE id = ?1", [&model_id], |row| row.get(0))
            .ok();
        let mut metrics: serde_json::Value = existing
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = metrics.as_object_mut() {
            obj.insert("gguf_path".to_string(), serde_json::json!(gguf_path.to_string_lossy()));
            obj.insert("gguf_quantization".to_string(), serde_json::json!(quant));
        }
        conn_guard.execute(
            "UPDATE local_models SET training_metrics_json = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![serde_json::to_string(&metrics).unwrap_or_default(), Utc::now().to_rfc3339(), &model_id],
        ).map_err(|e| format!("Database error: {}", e))?;
        
        return Ok(serde_json::json!({
            "success": true,
            "gguf_path": gguf_path.to_string_lossy(),
            "quantization": quant,
        }));
    }
    
    Err(format!(
        "llama.cpp convert script not found. Manual: python convert-hf-to-gguf.py \"{}\" --outfile \"{}\" --outtype {}",
        model_path.display(), gguf_path.display(), quant
    ))
}

/// Check if we can start a new training (resource protection)
#[tauri::command]
pub async fn can_start_training(
    db: State<'_, Database>,
    processes: State<'_, Arc<Mutex<HashMap<String, u32>>>>,
) -> Result<serde_json::Value, String> {
    let max = 1u32; // Max concurrent trainings
    let current: usize = processes.lock()
        .map_err(|e| format!("Lock error: {}", e))?
        .len();
    
    Ok(serde_json::json!({
        "can_start": current < max as usize,
        "current_count": current,
        "max_allowed": max,
        "message": if current >= max as usize {
            format!("Maximum {} training(s) in progress.", max)
        } else {
            "Ready to start training".to_string()
        }
    }))
}
