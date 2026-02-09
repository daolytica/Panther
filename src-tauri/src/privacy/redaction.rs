// PII Redaction Service
// Detects and redacts personally identifiable information from text

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a single redaction instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionEntry {
    pub original: String,
    pub placeholder: String,
    pub pii_type: PiiType,
    pub start_index: usize,
    pub end_index: usize,
}

/// Types of PII that can be detected
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PiiType {
    Email,
    Phone,
    Url,
    CreditCard,
    Address,
    NationalId,
    Custom,
    NamePattern,
}

impl PiiType {
    pub fn prefix(&self) -> &'static str {
        match self {
            PiiType::Email => "EMAIL",
            PiiType::Phone => "PHONE",
            PiiType::Url => "URL",
            PiiType::CreditCard => "CARD",
            PiiType::Address => "ADDRESS",
            PiiType::NationalId => "ID",
            PiiType::Custom => "CUSTOM",
            PiiType::NamePattern => "NAME",
        }
    }
}

/// Map of placeholders to original values for potential rehydration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RedactionMap {
    pub entries: HashMap<String, RedactionEntry>,
    pub context_id: String,
}

impl RedactionMap {
    pub fn new(context_id: &str) -> Self {
        Self {
            entries: HashMap::new(),
            context_id: context_id.to_string(),
        }
    }
}

/// Statistics about redactions performed
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RedactionStats {
    pub emails_redacted: usize,
    pub phones_redacted: usize,
    pub urls_redacted: usize,
    pub credit_cards_redacted: usize,
    pub addresses_redacted: usize,
    pub national_ids_redacted: usize,
    pub custom_tokens_redacted: usize,
    pub name_patterns_redacted: usize,
    pub total_redactions: usize,
}

/// Result of a redaction operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionResult {
    pub redacted_text: String,
    pub redaction_map: RedactionMap,
    pub stats: RedactionStats,
}

/// PII Redactor - main service for detecting and redacting PII
pub struct PiiRedactor {
    email_regex: Regex,
    phone_regex: Regex,
    url_regex: Regex,
    credit_card_regex: Regex,
    address_regex: Regex,
    ssn_regex: Regex,
    name_pattern_regex: Regex,
}

impl Default for PiiRedactor {
    fn default() -> Self {
        Self::new()
    }
}

impl PiiRedactor {
    pub fn new() -> Self {
        Self {
            // Email: RFC-lite regex
            email_regex: Regex::new(
                r"(?i)[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}"
            ).unwrap(),
            
            // Phone: International formats including E.164
            phone_regex: Regex::new(
                r"(?:\+\d{1,3}[-.\s]?)?\(?\d{2,4}\)?[-.\s]?\d{2,4}[-.\s]?\d{2,4}(?:[-.\s]?\d{1,4})?"
            ).unwrap(),
            
            // URL: HTTP/HTTPS with optional query params
            url_regex: Regex::new(
                r"https?://[^\s<>\[\]{}|\\^`\x00-\x1f]+"
            ).unwrap(),
            
            // Credit card: Common formats (13-19 digits with optional separators)
            credit_card_regex: Regex::new(
                r"\b(?:\d{4}[-\s]?){3}\d{1,4}\b"
            ).unwrap(),
            
            // Address: Heuristic pattern for street addresses
            address_regex: Regex::new(
                r"(?i)\d{1,5}\s+[\w\s]+(?:street|st|road|rd|avenue|ave|drive|dr|lane|ln|way|court|ct|circle|cir|boulevard|blvd|place|pl)\b(?:[,\s]+[\w\s]+)?(?:[,\s]+[A-Z]{2}\s+\d{5}(?:-\d{4})?)?"
            ).unwrap(),
            
            // SSN/National ID: US SSN pattern (can be extended for other countries)
            ssn_regex: Regex::new(
                r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b"
            ).unwrap(),
            
            // Name patterns: "My name is X", "I am X", "I'm X" (captures following words)
            name_pattern_regex: Regex::new(
                r"(?i)(?:my name is|i am|i'm|call me)\s+([A-Z][a-z]+(?:\s+[A-Z][a-z]+)?)"
            ).unwrap(),
        }
    }
    
    /// Redact PII from input text
    pub fn redact_text(
        &self,
        input: &str,
        custom_tokens: &[String],
        context_id: &str,
    ) -> RedactionResult {
        let mut redacted = input.to_string();
        let mut redaction_map = RedactionMap::new(context_id);
        let mut stats = RedactionStats::default();
        
        // Track counters for each PII type
        let mut counters: HashMap<PiiType, usize> = HashMap::new();
        
        // Collect all matches first to avoid overlapping issues
        let mut all_matches: Vec<(usize, usize, String, PiiType)> = Vec::new();
        
        // Detect emails
        for mat in self.email_regex.find_iter(input) {
            all_matches.push((mat.start(), mat.end(), mat.as_str().to_string(), PiiType::Email));
        }
        
        // Detect phones (filter out obvious non-phones like years)
        for mat in self.phone_regex.find_iter(input) {
            let matched = mat.as_str();
            // Filter out 4-digit numbers that might be years
            if matched.len() >= 7 && !matched.chars().all(|c| c.is_ascii_digit()) || matched.len() > 4 {
                all_matches.push((mat.start(), mat.end(), matched.to_string(), PiiType::Phone));
            }
        }
        
        // Detect URLs
        for mat in self.url_regex.find_iter(input) {
            all_matches.push((mat.start(), mat.end(), mat.as_str().to_string(), PiiType::Url));
        }
        
        // Detect credit cards
        for mat in self.credit_card_regex.find_iter(input) {
            let digits: String = mat.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() >= 13 && digits.len() <= 19 {
                // Optional: Luhn check
                if self.luhn_check(&digits) {
                    all_matches.push((mat.start(), mat.end(), mat.as_str().to_string(), PiiType::CreditCard));
                }
            }
        }
        
        // Detect addresses
        for mat in self.address_regex.find_iter(input) {
            all_matches.push((mat.start(), mat.end(), mat.as_str().to_string(), PiiType::Address));
        }
        
        // Detect SSN/National IDs
        for mat in self.ssn_regex.find_iter(input) {
            all_matches.push((mat.start(), mat.end(), mat.as_str().to_string(), PiiType::NationalId));
        }
        
        // Detect name patterns ("My name is John")
        for cap in self.name_pattern_regex.captures_iter(input) {
            if let Some(name_match) = cap.get(1) {
                all_matches.push((name_match.start(), name_match.end(), name_match.as_str().to_string(), PiiType::NamePattern));
            }
        }
        
        // Detect custom tokens (case-insensitive, whole word)
        for token in custom_tokens {
            if token.is_empty() {
                continue;
            }
            let escaped = regex::escape(token);
            if let Ok(custom_regex) = Regex::new(&format!(r"(?i)\b{}\b", escaped)) {
                for mat in custom_regex.find_iter(input) {
                    all_matches.push((mat.start(), mat.end(), mat.as_str().to_string(), PiiType::Custom));
                }
            }
        }
        
        // Sort by start position (descending) to replace from end to start
        all_matches.sort_by(|a, b| b.0.cmp(&a.0));
        
        // Remove overlapping matches (keep the first/longer one)
        let mut filtered_matches: Vec<(usize, usize, String, PiiType)> = Vec::new();
        for mat in all_matches {
            let overlaps = filtered_matches.iter().any(|existing| {
                mat.0 < existing.1 && mat.1 > existing.0
            });
            if !overlaps {
                filtered_matches.push(mat);
            }
        }
        
        // Sort again for consistent replacement order
        filtered_matches.sort_by(|a, b| b.0.cmp(&a.0));
        
        // Perform replacements
        for (start, end, original, pii_type) in filtered_matches {
            let counter = counters.entry(pii_type).or_insert(0);
            *counter += 1;
            
            let placeholder = format!("[{}_{:02}]", pii_type.prefix(), counter);
            
            let entry = RedactionEntry {
                original: original.clone(),
                placeholder: placeholder.clone(),
                pii_type,
                start_index: start,
                end_index: end,
            };
            
            redaction_map.entries.insert(placeholder.clone(), entry);
            
            // Replace in text (working from end to preserve indices)
            redacted.replace_range(start..end, &placeholder);
            
            // Update stats
            match pii_type {
                PiiType::Email => stats.emails_redacted += 1,
                PiiType::Phone => stats.phones_redacted += 1,
                PiiType::Url => stats.urls_redacted += 1,
                PiiType::CreditCard => stats.credit_cards_redacted += 1,
                PiiType::Address => stats.addresses_redacted += 1,
                PiiType::NationalId => stats.national_ids_redacted += 1,
                PiiType::Custom => stats.custom_tokens_redacted += 1,
                PiiType::NamePattern => stats.name_patterns_redacted += 1,
            }
            stats.total_redactions += 1;
        }
        
        RedactionResult {
            redacted_text: redacted,
            redaction_map,
            stats,
        }
    }
    
    /// Rehydrate redacted text back to original (for local display only)
    #[allow(dead_code)]
    pub fn rehydrate_text(&self, redacted_text: &str, redaction_map: &RedactionMap) -> String {
        let mut result = redacted_text.to_string();
        
        for (placeholder, entry) in &redaction_map.entries {
            result = result.replace(placeholder, &entry.original);
        }
        
        result
    }
    
    /// Luhn algorithm for credit card validation
    fn luhn_check(&self, digits: &str) -> bool {
        let mut sum = 0;
        let mut alternate = false;
        
        for c in digits.chars().rev() {
            if let Some(mut digit) = c.to_digit(10) {
                if alternate {
                    digit *= 2;
                    if digit > 9 {
                        digit -= 9;
                    }
                }
                sum += digit;
                alternate = !alternate;
            }
        }
        
        sum % 10 == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_email_redaction() {
        let redactor = PiiRedactor::new();
        let result = redactor.redact_text("Contact me at john.doe@example.com", &[], "test");
        assert!(result.redacted_text.contains("[EMAIL_01]"));
        assert!(!result.redacted_text.contains("john.doe@example.com"));
        assert_eq!(result.stats.emails_redacted, 1);
    }
    
    #[test]
    fn test_phone_redaction() {
        let redactor = PiiRedactor::new();
        let result = redactor.redact_text("Call me at +1-555-123-4567", &[], "test");
        assert!(result.redacted_text.contains("[PHONE_"));
        assert_eq!(result.stats.phones_redacted, 1);
    }
    
    #[test]
    fn test_url_redaction() {
        let redactor = PiiRedactor::new();
        let result = redactor.redact_text("Visit https://example.com/user/123?token=abc", &[], "test");
        assert!(result.redacted_text.contains("[URL_01]"));
        assert_eq!(result.stats.urls_redacted, 1);
    }
    
    #[test]
    fn test_custom_token_redaction() {
        let redactor = PiiRedactor::new();
        let custom = vec!["Reza".to_string(), "Panther Corp".to_string()];
        let result = redactor.redact_text("Hi, I'm Reza from Panther Corp.", &custom, "test");
        assert!(!result.redacted_text.contains("Reza"));
        assert!(!result.redacted_text.contains("Panther Corp"));
        assert_eq!(result.stats.custom_tokens_redacted, 2);
    }
    
    #[test]
    fn test_name_pattern_redaction() {
        let redactor = PiiRedactor::new();
        let result = redactor.redact_text("My name is John Smith and I need help.", &[], "test");
        assert!(result.redacted_text.contains("[NAME_"));
        assert!(!result.redacted_text.contains("John Smith"));
    }
    
    #[test]
    fn test_rehydration() {
        let redactor = PiiRedactor::new();
        let original = "Contact me at test@example.com";
        let result = redactor.redact_text(original, &[], "test");
        let rehydrated = redactor.rehydrate_text(&result.redacted_text, &result.redaction_map);
        assert_eq!(rehydrated, original);
    }
    
    #[test]
    fn test_multiple_pii() {
        let redactor = PiiRedactor::new();
        let text = "Email: a@b.com, Phone: 555-1234567, another: x@y.org";
        let result = redactor.redact_text(text, &[], "test");
        assert_eq!(result.stats.emails_redacted, 2);
        assert!(result.stats.total_redactions >= 2);
    }
}
