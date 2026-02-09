// Profile-related commands

use crate::db::Database;
use crate::provider_resolver::complete_resolving_hybrid;
use crate::types::{PromptPacket, CharacterDefinition};
use serde::{Deserialize, Serialize};
use tauri::State;
use anyhow::Result;
use reqwest::Client;
use scraper::{Html, Selector};
use regex::Regex;
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateCharacterFromUrlRequest {
    pub urls: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub person_name: Option<String>,
    pub provider_account_id: String,
    pub model_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancellation_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateCharacterFromUrlResponse {
    pub character: CharacterDefinition,
    pub extracted_text: String, // The raw extracted text from URLs
}

#[tauri::command]
pub async fn generate_character_from_url(
    db: State<'_, Database>,
    cancellation_tokens: State<'_, Arc<Mutex<HashMap<String, bool>>>>,
    request: GenerateCharacterFromUrlRequest,
) -> Result<GenerateCharacterFromUrlResponse, String> {
    // Generate cancellation token if not provided
    let token = request.cancellation_token.unwrap_or_else(|| Uuid::new_v4().to_string());
    
    // Initialize cancellation token as not cancelled
    {
        let mut tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
        tokens.insert(token.clone(), false);
    }
    
    // Create HTTP client
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Fetch all URLs and combine content
    let mut all_text_content = Vec::new();
    
    for (index, url) in request.urls.iter().enumerate() {
        // Check for cancellation
        {
            let tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
            if tokens.get(&token).copied().unwrap_or(false) {
                // Clean up token
                drop(tokens);
                let mut tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
                tokens.remove(&token);
                return Err("Generation cancelled by user".to_string());
            }
        }
        
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch URL {} ({}): {}", index + 1, url, e))?;

        if !response.status().is_success() {
            eprintln!("Warning: Failed to fetch URL {} ({}): HTTP {}", index + 1, url, response.status());
            continue; // Skip failed URLs but continue with others
        }

        let html = response.text().await
            .map_err(|e| format!("Failed to read response from URL {} ({}): {}", index + 1, url, e))?;

        // Extract text content from HTML
        let text_content = {
            let document = Html::parse_document(&html);
            
            // Extract text from body
            let body_selector = Selector::parse("body").unwrap();
            let body_text = document.select(&body_selector)
                .next()
                .map(|body| {
                    // Get all text nodes
                    body.text().collect::<Vec<_>>().join(" ")
                })
                .unwrap_or_default();

            // Clean up the text - remove extra whitespace
            let re = Regex::new(r"\s+").unwrap();
            let cleaned_text = re.replace_all(&body_text, " ");
            cleaned_text.trim().to_string()
        };
        
        if !text_content.is_empty() {
            all_text_content.push((url.clone(), text_content));
        }
        
        // Check for cancellation after each URL
        {
            let tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
            if tokens.get(&token).copied().unwrap_or(false) {
                drop(tokens);
                let mut tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
                tokens.remove(&token);
                return Err("Generation cancelled by user".to_string());
            }
        }
    }
    
    if all_text_content.is_empty() {
        return Err("No text content found on any of the provided URLs".to_string());
    }
    
    // Combine all text content with source indicators
    let combined_text = all_text_content.iter()
        .map(|(url, text)| {
            // Limit each source to 6000 chars to avoid token limits
            let limited_text = if text.len() > 6000 {
                &text[..6000]
            } else {
                text
            };
            format!("=== Source: {} ===\n{}\n", url, limited_text)
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    
    // Limit total combined text to 15000 chars
    let text_to_analyze = if combined_text.len() > 15000 {
        &combined_text[..15000]
    } else {
        &combined_text
    };
    
    // Check for cancellation before LLM call
    {
        let tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
        if tokens.get(&token).copied().unwrap_or(false) {
            drop(tokens);
            let mut tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
            tokens.remove(&token);
            return Err("Generation cancelled by user".to_string());
        }
    }
    
    // Provider selection (supports provider_type = "hybrid").

    // Create prompt for character analysis - enhanced to be more accurate with multiple URLs and name
    let name_instruction = if let Some(name) = &request.person_name {
        format!("\n\nIMPORTANT: The person you are analyzing is named \"{}\". Focus on information about THIS SPECIFIC PERSON. If multiple people are mentioned, prioritize information about {}.", name, name)
    } else {
        String::new()
    };
    
    let source_count = all_text_content.len();
    let source_note = if source_count > 1 {
        format!("\n\nNote: Information has been gathered from {} different sources. Combine and synthesize information from all sources to create a comprehensive character definition.", source_count)
    } else {
        String::new()
    };
    
    let analysis_prompt = format!(
        "You are an expert at analyzing professional profiles and extracting EXACT character information. Your task is to create a character definition that accurately reflects the person's profile based on information from multiple sources.{}\n\n\
        CRITICAL INSTRUCTIONS:\n\
        - Extract information EXACTLY as it appears in the profiles\n\
        - Do NOT invent or assume information that is not present\n\
        - Match the person's actual personality traits, expertise areas, and communication style as closely as possible\n\
        - If specific information is not available, use \"Unknown\" or leave arrays empty rather than guessing\n\
        - Preserve the person's actual professional background and experience\n\
        - Capture their real goals and objectives if mentioned\n\
        - Synthesize information from all sources to create a comprehensive profile{}\n\n\
        Extract the following information:\n\
        1. Name: The person's full name (exactly as shown, or use the provided name if consistent)\n\
        2. Role: Their professional role or title (exactly as stated, or most common if multiple)\n\
        3. Personality: 3-7 key personality traits based on their profile content, writing style, achievements, and how they present themselves across all sources (as an array)\n\
        4. Expertise: 3-10 specific areas of expertise based on their experience, education, projects, and skills mentioned across all sources (as an array)\n\
        5. Communication Style: How they communicate based on their writing, presentations, or described style across all sources (brief but specific description)\n\
        6. Background: Professional background and experience (2-4 sentences, synthesized from all sources)\n\
        7. Goals: 2-5 key goals or objectives if mentioned or implied across sources (as an array, can be empty if not found)\n\
        8. Constraints: Any limitations, values, or constraints they work within if mentioned (as an array, optional)\n\n\
        Profile Information from Multiple Sources:\n{}\n\n\
        Return ONLY a valid JSON object with this exact structure. Be precise and accurate:\n\
        {{\n\
          \"name\": \"exact name from profiles or provided name\",\n\
          \"role\": \"exact role/title from profiles\",\n\
          \"personality\": [\"trait based on actual profile content\", \"another trait\", ...],\n\
          \"expertise\": [\"specific expertise area from profiles\", \"another area\", ...],\n\
          \"communication_style\": \"description based on their actual communication style\",\n\
          \"background\": \"detailed background synthesized from all sources\",\n\
          \"goals\": [\"goal if mentioned\", ...],\n\
          \"constraints\": [\"constraint if mentioned\", ...]\n\
        }}",
        name_instruction,
        source_note,
        text_to_analyze
    );

    // Create prompt packet
    let prompt_packet = PromptPacket {
        global_instructions: None,
        persona_instructions: "You are an expert at analyzing professional profiles and extracting structured character information. You MUST return ONLY valid JSON with no markdown formatting, no code blocks, no explanations, and no additional text. The response must be a valid JSON object that can be parsed directly.".to_string(),
        user_message: analysis_prompt,
        conversation_context: None,
        params_json: json!({
            "temperature": 0.3,
            "max_tokens": 2000,
        }),
        stream: false,
    };

    // Call LLM to analyze (supports provider_type = "hybrid").
    let timeout_secs = 120u64;
    let (response, _used_provider, _used_model) =
        complete_resolving_hybrid(&db, &request.provider_account_id, &request.model_name, &prompt_packet, timeout_secs, None)
            .await
            .map_err(|e| format!("LLM analysis failed: {}", e))?;

    // Parse JSON response - handle various formats
    let character_json: serde_json::Value = {
        let text = response.text.trim();
        
        // First, try direct JSON parsing
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
            json
        } else {
            // Try to extract JSON from markdown code blocks
            let json_str = if text.contains("```json") {
                // Extract from ```json ... ```
                let start = text.find("```json").map(|i| i + 7).unwrap_or(0);
                let end = text[start..].find("```").map(|i| start + i).unwrap_or(text.len());
                text[start..end].trim()
            } else if text.contains("```") {
                // Extract from ``` ... ```
                let start = text.find("```").map(|i| i + 3).unwrap_or(0);
                let end = text[start..].find("```").map(|i| start + i).unwrap_or(text.len());
                text[start..end].trim()
            } else {
                // Try to find JSON object boundaries
                let json_start = text.find('{').unwrap_or(0);
                let json_end = text.rfind('}').map(|i| i + 1).unwrap_or(text.len());
                &text[json_start..json_end]
            };
            
            // Try parsing the extracted JSON
            serde_json::from_str(json_str)
                .map_err(|e| {
                    format!(
                        "Failed to parse character definition JSON: {}\n\nExtracted JSON: {}\n\nFull response: {}",
                        e,
                        json_str,
                        if text.len() > 500 { format!("{}...", &text[..500]) } else { text.to_string() }
                    )
                })?
        }
    };

    // Try to deserialize directly to CharacterDefinition first (more robust)
    let character = match serde_json::from_value::<CharacterDefinition>(character_json.clone()) {
        Ok(char) => char,
        Err(_) => {
            // Fallback to manual construction if direct deserialization fails
            CharacterDefinition {
                name: character_json.get("name")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Unknown".to_string()),
                role: character_json.get("role")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Unknown".to_string()),
                personality: character_json.get("personality")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default(),
                expertise: character_json.get("expertise")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                    .unwrap_or_default(),
                communication_style: character_json.get("communication_style")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "Professional".to_string()),
                background: character_json.get("background")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                goals: character_json.get("goals")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()),
                constraints: character_json.get("constraints")
                    .and_then(|v| v.as_array())
                    .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()),
            }
        }
    };

    // Validate that we have at least name and role
    if character.name.is_empty() || character.name == "Unknown" {
        return Err(format!(
            "Character definition is missing required 'name' field. Received JSON: {}",
            serde_json::to_string_pretty(&character_json).unwrap_or_else(|_| "Invalid JSON".to_string())
        ));
    }

    // Clean up cancellation token
    {
        let mut tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
        tokens.remove(&token);
    }
    
    // Return both character and extracted text
    Ok(GenerateCharacterFromUrlResponse {
        character,
        extracted_text: combined_text,
    })
}

#[tauri::command]
pub async fn cancel_character_generation(
    cancellation_tokens: State<'_, Arc<Mutex<HashMap<String, bool>>>>,
    token: String,
) -> Result<(), String> {
    let mut tokens = cancellation_tokens.lock().map_err(|e| format!("Failed to lock cancellation tokens: {}", e))?;
    tokens.insert(token, true);
    Ok(())
}

#[tauri::command]
pub async fn get_latest_profile(
    db: State<'_, Database>,
) -> Result<Option<serde_json::Value>, String> {
    let conn = db.get_connection();
    let conn_guard = conn.lock().map_err(|e| format!("Database lock error: {}", e))?;
    
    // First, try to get the most recently interacted profile (from chat_messages)
    let latest_interacted: Option<serde_json::Value> = conn_guard
        .query_row(
            "SELECT p.id, p.name, p.photo_url, p.created_at, p.updated_at
             FROM prompt_profiles p
             INNER JOIN chat_messages cm ON p.id = cm.profile_id
             ORDER BY cm.created_at DESC
             LIMIT 1",
            [],
            |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "photo_url": row.get::<_, Option<String>>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                    "updated_at": row.get::<_, String>(4)?,
                    "source": "interacted"
                }))
            },
        )
        .ok();
    
    if latest_interacted.is_some() {
        return Ok(latest_interacted);
    }
    
    // If no interactions, get the most recently created profile
    let latest_created: Option<serde_json::Value> = conn_guard
        .query_row(
            "SELECT id, name, photo_url, created_at, updated_at
             FROM prompt_profiles
             ORDER BY created_at DESC
             LIMIT 1",
            [],
            |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "name": row.get::<_, String>(1)?,
                    "photo_url": row.get::<_, Option<String>>(2)?,
                    "created_at": row.get::<_, String>(3)?,
                    "updated_at": row.get::<_, String>(4)?,
                    "source": "created"
                }))
            },
        )
        .ok();
    
    Ok(latest_created)
}
