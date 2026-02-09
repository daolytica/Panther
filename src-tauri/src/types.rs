// Type definitions for Rust side

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderAccount {
    pub id: String,
    pub provider_type: String,
    pub display_name: String,
    pub base_url: Option<String>,
    pub region: Option<String>,
    pub auth_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub provider_metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptPacket {
    pub global_instructions: Option<String>,
    pub persona_instructions: String,
    pub user_message: String,
    pub conversation_context: Option<Vec<Message>>,
    pub params_json: serde_json::Value,
    pub stream: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub run_id: String,
    pub author_type: String,
    pub profile_id: Option<String>,
    pub round_index: Option<i32>,
    pub turn_index: Option<i32>,
    pub text: String,
    pub created_at: String,
    pub provider_metadata_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedResponse {
    pub text: String,
    pub finish_reason: Option<String>,
    pub request_id: Option<String>,
    pub usage_json: Option<serde_json::Value>,
    pub raw_provider_payload_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterDefinition {
    pub name: String,
    pub role: String,
    pub personality: Vec<String>,
    pub expertise: Vec<String>,
    pub communication_style: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub goals: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<Vec<String>>,
}
