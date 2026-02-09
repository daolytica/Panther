// Cache module for training data caching

pub mod training_cache;

use sha2::{Sha256, Digest};
use std::path::PathBuf;
use std::fs;

/// Compute SHA256 hash of training data
/// For large datasets, this uses streaming to avoid loading all data into memory
#[allow(dead_code)]
pub fn compute_data_hash(inputs: &[String], outputs: &[String]) -> String {
    let mut hasher = Sha256::new();
    
    // Hash each input-output pair
    for (input, output) in inputs.iter().zip(outputs.iter()) {
        hasher.update(input.as_bytes());
        hasher.update(b"\0"); // Separator
        hasher.update(output.as_bytes());
        hasher.update(b"\n"); // Newline separator
    }
    
    format!("{:x}", hasher.finalize())
}

/// Compute hash from streaming iterator (for very large datasets)
#[allow(dead_code)]
pub fn compute_data_hash_streaming<F>(iterator: F) -> Result<String, String>
where
    F: Iterator<Item = Result<(String, String), String>>,
{
    let mut hasher = Sha256::new();
    let mut count = 0;
    
    for item in iterator {
        let (input, output) = item.map_err(|e| format!("Iterator error: {}", e))?;
        hasher.update(input.as_bytes());
        hasher.update(b"\0");
        hasher.update(output.as_bytes());
        hasher.update(b"\n");
        count += 1;
        
        // Progress update every 10k records
        if count % 10000 == 0 {
            // Could emit progress event here if needed
        }
    }
    
    Ok(format!("{:x}", hasher.finalize()))
}

/// Get cache directory path
pub fn get_cache_dir() -> Result<PathBuf, String> {
    let app_data_dir = if cfg!(windows) {
        std::env::var("APPDATA")
            .map(|p| PathBuf::from(p).join("panther").join("training").join("cache"))
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".local").join("share").join("panther").join("training").join("cache"))
                    .unwrap_or_else(|_| PathBuf::from(".").join("cache"))
            })
    } else {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join(".local").join("share").join("panther").join("training").join("cache"))
            .unwrap_or_else(|_| PathBuf::from(".").join("cache"))
    };
    
    // Ensure directory exists
    fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create cache directory: {}", e))?;
    
    Ok(app_data_dir)
}

/// Generate cache file path
pub fn get_cache_file_path(project_id: &str, model_id: &str, data_hash: &str, compressed: bool) -> Result<PathBuf, String> {
    let cache_dir = get_cache_dir()?;
    let extension = if compressed { "jsonl.gz" } else { "jsonl" };
    let filename = format!("{}_{}_{}.{}", project_id, model_id, data_hash, extension);
    Ok(cache_dir.join(filename))
}

/// Get file size in bytes
pub fn get_file_size(path: &PathBuf) -> Result<u64, String> {
    fs::metadata(path)
        .map(|m| m.len())
        .map_err(|e| format!("Failed to get file size: {}", e))
}

/// Check if file should use memory mapping based on size and settings
#[allow(dead_code)]
pub fn should_use_memory_mapping(file_path: &PathBuf, threshold_mb: u64) -> bool {
    if let Ok(metadata) = fs::metadata(file_path) {
        let size_mb = metadata.len() / (1024 * 1024);
        size_mb >= threshold_mb as u64
    } else {
        false
    }
}
