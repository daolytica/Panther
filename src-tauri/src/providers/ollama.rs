// Ollama provider adapter for local LLM support

use crate::types::{ProviderAccount, PromptPacket, NormalizedResponse};
use crate::providers::adapter_trait::ProviderAdapter;
use anyhow::{Result, Context};
use serde_json::json;
use std::time::Duration;

pub struct OllamaAdapter {
    client: reqwest::Client,
}

impl OllamaAdapter {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client");
        
        OllamaAdapter { client }
    }

    fn get_base_url(&self, config: &ProviderAccount) -> String {
        config.base_url.clone().unwrap_or_else(|| "http://localhost:11434".to_string())
    }

    fn build_messages(&self, packet: &PromptPacket, _config: &ProviderAccount) -> serde_json::Value {
        let mut messages = Vec::new();
        
        // Add system message if present
        if let Some(system) = &packet.global_instructions {
            messages.push(json!({
                "role": "system",
                "content": system
            }));
        }
        
        // Add conversation context
        if let Some(context) = &packet.conversation_context {
            for msg in context {
                messages.push(json!({
                    "role": if msg.author_type == "user" { "user" } else { "assistant" },
                    "content": msg.text
                }));
            }
        }
        
        // Add current user message
        messages.push(json!({
            "role": "user",
            "content": packet.user_message
        }));
        
        json!({ "messages": messages })
    }
}

#[async_trait::async_trait]
impl ProviderAdapter for OllamaAdapter {
    async fn validate(&self, config: &ProviderAccount) -> Result<bool> {
        let base_url = self.get_base_url(config);
        let url = format!("{}/api/tags", base_url);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false), // Ollama might not be running, that's okay
        }
    }

    async fn list_models(&self, config: &ProviderAccount) -> Result<Vec<String>> {
        let base_url = self.get_base_url(config);
        let url = format!("{}/api/tags", base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to Ollama")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Ollama API returned error: {}", response.status());
        }
        
        let json: serde_json::Value = response.json().await?;
        let models = json["models"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid response format"))?
            .iter()
            .filter_map(|m| m["name"].as_str().map(|s| s.to_string()))
            .collect();
        
        Ok(models)
    }

    async fn complete(
        &self,
        packet: &PromptPacket,
        config: &ProviderAccount,
        model: &str,
    ) -> Result<NormalizedResponse> {
        let base_url = self.get_base_url(config);
        let url = format!("{}/api/chat", base_url);
        
        let messages = self.build_messages(packet, config);
        
        // Extract parameters
        let params = &packet.params_json;
        let temperature = params["temperature"]
            .as_f64()
            .unwrap_or(0.7)
            .clamp(0.0, 2.0);
        let max_tokens = params["max_tokens"]
            .as_u64()
            .unwrap_or(2048) as i32;
        
        let request_body = json!({
            "model": model,
            "messages": messages["messages"],
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens,
            },
            "stream": false
        });
        
        let response = self.client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Ollama")?;
        
        let status = response.status();
        let json: serde_json::Value = if status.is_success() {
            response.json().await.context("Failed to parse Ollama response")?
        } else {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error: {} - {}", status, error_text);
        };
        
        let content = json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        // Extract usage if available
        let usage = if json["usage"].is_object() {
            Some(json["usage"].clone())
        } else {
            None
        };
        
        Ok(NormalizedResponse {
            text: content,
            finish_reason: json["done"].as_bool().and_then(|d| if d { Some("stop".to_string()) } else { None }),
            request_id: None,
            usage_json: usage,
            raw_provider_payload_json: Some(json),
        })
    }

    async fn stream(
        &self,
        packet: &PromptPacket,
        config: &ProviderAccount,
        model: &str,
        on_chunk: Box<dyn Fn(String) + Send>,
    ) -> Result<NormalizedResponse> {
        let base_url = self.get_base_url(config);
        let url = format!("{}/api/chat", base_url);
        
        let messages = self.build_messages(packet, config);
        
        let params = &packet.params_json;
        let temperature = params["temperature"]
            .as_f64()
            .unwrap_or(0.7)
            .clamp(0.0, 2.0);
        let max_tokens = params["max_tokens"]
            .as_u64()
            .unwrap_or(2048) as i32;
        
        let request_body = json!({
            "model": model,
            "messages": messages["messages"],
            "options": {
                "temperature": temperature,
                "num_predict": max_tokens,
            },
            "stream": true
        });
        
        let response = self.client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Ollama")?;
        
        let status = response.status();
        let json: serde_json::Value = if status.is_success() {
            response.json().await.context("Failed to parse Ollama response")?
        } else {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error: {} - {}", status, error_text);
        };
        
        // For streaming, we'll use the non-streaming endpoint and simulate chunks
        // In a production system, you'd want to properly handle SSE streaming
        // Note: Ollama streaming returns newline-delimited JSON, but for simplicity
        // we'll use non-streaming mode and simulate chunks
        let content = json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        // Simulate streaming by sending chunks
        let chunk_size = 10;
        for (i, chunk) in content.as_bytes().chunks(chunk_size).enumerate() {
            if let Ok(text) = String::from_utf8(chunk.to_vec()) {
                on_chunk(text);
            }
            // Small delay to simulate streaming
            if i % 10 == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }
        
        Ok(NormalizedResponse {
            text: content,
            finish_reason: json["done"].as_bool().and_then(|d| if d { Some("stop".to_string()) } else { None }),
            request_id: None,
            usage_json: json.get("usage").cloned(),
            raw_provider_payload_json: Some(json),
        })
    }
}
