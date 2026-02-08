// Sanitized Logger
// Prevents sensitive data from appearing in logs

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Safe fields that can be logged
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct SafeLogFields {
    pub request_id: Option<String>,
    pub conversation_id: Option<String>,
    pub pseudonym_hash: Option<String>,  // Hashed pseudonym only
    pub token_count: Option<usize>,
    pub latency_ms: Option<u64>,
    pub redaction_count: Option<usize>,
    pub event_type: Option<String>,
    pub status_code: Option<u16>,
    pub error_type: Option<String>,  // Error type only, not message
}

impl Default for SafeLogFields {
    fn default() -> Self {
        Self {
            request_id: None,
            conversation_id: None,
            pseudonym_hash: None,
            token_count: None,
            latency_ms: None,
            redaction_count: None,
            event_type: None,
            status_code: None,
            error_type: None,
        }
    }
}

/// Sanitized logger that prevents PII from being logged
#[allow(dead_code)]
pub struct SanitizedLogger {
    allowed_fields: HashSet<String>,
}

impl Default for SanitizedLogger {
    fn default() -> Self {
        Self::new()
    }
}

impl SanitizedLogger {
    #[allow(dead_code)]
    pub fn new() -> Self {
        let mut allowed = HashSet::new();
        // Only these fields are allowed in logs
        allowed.insert("request_id".to_string());
        allowed.insert("conversation_id".to_string());
        allowed.insert("pseudonym_hash".to_string());
        allowed.insert("token_count".to_string());
        allowed.insert("latency_ms".to_string());
        allowed.insert("redaction_count".to_string());
        allowed.insert("event_type".to_string());
        allowed.insert("status_code".to_string());
        allowed.insert("error_type".to_string());
        
        Self {
            allowed_fields: allowed,
        }
    }
    
    /// Log info-level event with safe fields only
    #[allow(dead_code)]
    pub fn log_info(&self, event_name: &str, fields: &SafeLogFields) {
        let json = serde_json::to_string(fields).unwrap_or_default();
        // Using eprintln for now; in production, use a proper logging framework
        eprintln!("[INFO] {}: {}", event_name, json);
    }
    
    /// Log error with safe fields only (no raw error messages that might contain PII)
    #[allow(dead_code)]
    pub fn log_error(&self, event_name: &str, error_type: &str, fields: &SafeLogFields) {
        let mut safe_fields = fields.clone();
        safe_fields.error_type = Some(error_type.to_string());
        let json = serde_json::to_string(&safe_fields).unwrap_or_default();
        eprintln!("[ERROR] {}: {}", event_name, json);
    }
    
    /// Log a warning with safe fields
    #[allow(dead_code)]
    pub fn log_warn(&self, event_name: &str, fields: &SafeLogFields) {
        let json = serde_json::to_string(fields).unwrap_or_default();
        eprintln!("[WARN] {}: {}", event_name, json);
    }
    
    /// Sanitize a string to remove potential PII (for error messages)
    #[allow(dead_code)]
    pub fn sanitize_error_message(&self, message: &str) -> String {
        // Remove common PII patterns from error messages
        let mut sanitized = message.to_string();
        
        // Remove email-like patterns
        let email_regex = regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").ok();
        if let Some(re) = email_regex {
            sanitized = re.replace_all(&sanitized, "[REDACTED_EMAIL]").to_string();
        }
        
        // Remove URL-like patterns
        let url_regex = regex::Regex::new(r"https?://[^\s]+").ok();
        if let Some(re) = url_regex {
            sanitized = re.replace_all(&sanitized, "[REDACTED_URL]").to_string();
        }
        
        // Remove phone-like patterns
        let phone_regex = regex::Regex::new(r"\b\d{3}[-.]?\d{3}[-.]?\d{4}\b").ok();
        if let Some(re) = phone_regex {
            sanitized = re.replace_all(&sanitized, "[REDACTED_PHONE]").to_string();
        }
        
        // Truncate very long messages
        if sanitized.len() > 200 {
            sanitized = format!("{}...[truncated]", &sanitized[..200]);
        }
        
        sanitized
    }
    
    /// Create safe fields builder
    #[allow(dead_code)]
    pub fn fields() -> SafeLogFieldsBuilder {
        SafeLogFieldsBuilder::new()
    }
}

/// Builder for SafeLogFields
#[allow(dead_code)]
pub struct SafeLogFieldsBuilder {
    fields: SafeLogFields,
}

impl SafeLogFieldsBuilder {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            fields: SafeLogFields::default(),
        }
    }
    
    #[allow(dead_code)]
    pub fn request_id(mut self, id: &str) -> Self {
        self.fields.request_id = Some(id.to_string());
        self
    }
    
    #[allow(dead_code)]
    pub fn conversation_id(mut self, id: &str) -> Self {
        self.fields.conversation_id = Some(id.to_string());
        self
    }
    
    #[allow(dead_code)]
    pub fn pseudonym_hash(mut self, hash: &str) -> Self {
        self.fields.pseudonym_hash = Some(hash.to_string());
        self
    }
    
    #[allow(dead_code)]
    pub fn token_count(mut self, count: usize) -> Self {
        self.fields.token_count = Some(count);
        self
    }
    
    #[allow(dead_code)]
    pub fn latency_ms(mut self, ms: u64) -> Self {
        self.fields.latency_ms = Some(ms);
        self
    }
    
    #[allow(dead_code)]
    pub fn redaction_count(mut self, count: usize) -> Self {
        self.fields.redaction_count = Some(count);
        self
    }
    
    #[allow(dead_code)]
    pub fn event_type(mut self, event: &str) -> Self {
        self.fields.event_type = Some(event.to_string());
        self
    }
    
    #[allow(dead_code)]
    pub fn status_code(mut self, code: u16) -> Self {
        self.fields.status_code = Some(code);
        self
    }
    
    #[allow(dead_code)]
    pub fn build(self) -> SafeLogFields {
        self.fields
    }
}

impl Default for SafeLogFieldsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_sanitize_error_message() {
        let logger = SanitizedLogger::new();
        
        let msg = "Failed for user test@example.com at https://api.example.com";
        let sanitized = logger.sanitize_error_message(msg);
        
        assert!(!sanitized.contains("test@example.com"));
        assert!(!sanitized.contains("https://api.example.com"));
        assert!(sanitized.contains("[REDACTED_EMAIL]"));
        assert!(sanitized.contains("[REDACTED_URL]"));
    }
    
    #[test]
    fn test_fields_builder() {
        let fields = SanitizedLogger::fields()
            .request_id("req-123")
            .token_count(500)
            .latency_ms(150)
            .build();
        
        assert_eq!(fields.request_id, Some("req-123".to_string()));
        assert_eq!(fields.token_count, Some(500));
        assert_eq!(fields.latency_ms, Some(150));
    }
}
