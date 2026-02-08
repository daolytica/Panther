// Ollama-specific commands for installation check and model management

use serde::{Deserialize, Serialize};
use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaHealth {
    pub installed: bool,
    pub running: bool,
    pub version: Option<String>,
    pub models: Vec<String>,
    pub base_url: String,
}

#[tauri::command]
pub async fn check_ollama_installation(base_url: Option<String>) -> Result<OllamaHealth, String> {
    let base_url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    // Check if Ollama is running
    let tags_url = format!("{}/api/tags", base_url);
    let running = client.get(&tags_url).send().await.is_ok();
    
    let mut version = None;
    let mut models = Vec::new();
    
    if running {
        // Try to get version and models
        match client.get(&tags_url).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    if let Ok(json) = response.json::<serde_json::Value>().await {
                        if let Some(models_array) = json["models"].as_array() {
                            models = models_array
                                .iter()
                                .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
                                .collect();
                        }
                    }
                }
            }
            Err(_) => {}
        }
        
        // Try to get version from /api/version endpoint
        let version_url = format!("{}/api/version", base_url);
        if let Ok(response) = client.get(&version_url).send().await {
            if response.status().is_success() {
                if let Ok(json) = response.json::<serde_json::Value>().await {
                    version = json["version"].as_str().map(|s| s.to_string());
                }
            }
        }
    }
    
    // Check if ollama command exists (for installation check)
    let installed = std::process::Command::new("ollama")
        .arg("--version")
        .output()
        .is_ok() || running;
    
    Ok(OllamaHealth {
        installed,
        running,
        version,
        models,
        base_url,
    })
}

#[tauri::command]
pub async fn pull_ollama_model(
    base_url: Option<String>,
    model_name: String,
) -> Result<String, String> {
    let base_url = base_url.unwrap_or_else(|| "http://localhost:11434".to_string());
    let client = Client::builder()
        .timeout(Duration::from_secs(300)) // 5 minutes for model pulling
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let pull_url = format!("{}/api/pull", base_url);
    let response = client
        .post(&pull_url)
        .json(&serde_json::json!({ "name": model_name }))
        .send()
        .await
        .map_err(|e| format!("Failed to pull model: {}", e))?;
    
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Failed to pull model: {}", error_text));
    }
    
    // Stream the response to show progress
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    
    while let Some(chunk_result) = stream.next().await {
        let _ = chunk_result.map_err(|e| format!("Stream error: {}", e))?;
        // In a real implementation, you'd emit progress events here
    }
    
    Ok(format!("Model {} pulled successfully", model_name))
}
