// Pseudonymous Session Manager
// Generates rotating, non-stable identifiers for LLM API calls

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use serde::{Deserialize, Serialize};
use chrono::{Utc, Duration};
use uuid::Uuid;

type HmacSha256 = Hmac<Sha256>;

/// Represents a pseudonymous identifier for a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ConversationPseudonym {
    pub pseudonym_id: String,
    pub conversation_id: String,
    pub created_at: String,
    pub expires_at: String,
}

/// Manager for creating and validating pseudonymous identifiers
pub struct PseudonymManager {
    server_secret: Vec<u8>,
}

impl PseudonymManager {
    /// Create a new PseudonymManager with a server secret
    #[allow(dead_code)]
    pub fn new(server_secret: &[u8]) -> Self {
        Self {
            server_secret: server_secret.to_vec(),
        }
    }
    
    /// Create a new PseudonymManager with a random secret (for initialization)
    pub fn with_random_secret() -> Self {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let secret: [u8; 32] = rng.gen();
        Self {
            server_secret: secret.to_vec(),
        }
    }
    
    /// Generate a pseudonym for a conversation
    /// Uses HMAC(server_secret, user_id + conversation_id + random_salt)
    #[allow(dead_code)]
    pub fn generate_pseudonym(&self, user_id: &str, conversation_id: &str) -> ConversationPseudonym {
        let now = Utc::now();
        let expires = now + Duration::hours(24); // Expires after 24 hours
        
        // Generate random salt for additional entropy
        let salt = Uuid::new_v4().to_string();
        
        // Create HMAC
        let mut mac = HmacSha256::new_from_slice(&self.server_secret)
            .expect("HMAC can take key of any size");
        
        mac.update(user_id.as_bytes());
        mac.update(conversation_id.as_bytes());
        mac.update(salt.as_bytes());
        mac.update(now.timestamp().to_string().as_bytes());
        
        let result = mac.finalize();
        let hash = result.into_bytes();
        
        // Encode as URL-safe base64
        let pseudonym_id = format!("psn_{}", URL_SAFE_NO_PAD.encode(&hash[..16]));
        
        ConversationPseudonym {
            pseudonym_id,
            conversation_id: conversation_id.to_string(),
            created_at: now.to_rfc3339(),
            expires_at: expires.to_rfc3339(),
        }
    }
    
    /// Generate a one-time pseudonym (different each call, even for same conversation)
    /// This is the most privacy-preserving option
    pub fn generate_ephemeral_pseudonym(&self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let random_bytes: [u8; 16] = rng.gen();
        
        let mut mac = HmacSha256::new_from_slice(&self.server_secret)
            .expect("HMAC can take key of any size");
        
        mac.update(&random_bytes);
        mac.update(Utc::now().timestamp_nanos_opt().unwrap_or(0).to_le_bytes().as_slice());
        
        let result = mac.finalize();
        let hash = result.into_bytes();
        
        format!("eph_{}", URL_SAFE_NO_PAD.encode(&hash[..12]))
    }
    
    /// Check if a pseudonym has expired
    #[allow(dead_code)]
    pub fn is_expired(&self, pseudonym: &ConversationPseudonym) -> bool {
        if let Ok(expires) = chrono::DateTime::parse_from_rfc3339(&pseudonym.expires_at) {
            expires < Utc::now()
        } else {
            true // If we can't parse the date, consider it expired
        }
    }
    
    /// Hash a pseudonym for logging purposes (further anonymization)
    #[allow(dead_code)]
    pub fn hash_for_logging(&self, pseudonym_id: &str) -> String {
        use sha2::Digest;
        let mut hasher = Sha256::new();
        hasher.update(pseudonym_id.as_bytes());
        let result = hasher.finalize();
        format!("log_{}", URL_SAFE_NO_PAD.encode(&result[..8]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pseudonym_generation() {
        let manager = PseudonymManager::with_random_secret();
        let p1 = manager.generate_pseudonym("user1", "conv1");
        let p2 = manager.generate_pseudonym("user1", "conv1");
        
        // Different pseudonyms even for same user/conversation (due to random salt)
        assert_ne!(p1.pseudonym_id, p2.pseudonym_id);
        assert!(p1.pseudonym_id.starts_with("psn_"));
    }
    
    #[test]
    fn test_ephemeral_pseudonym() {
        let manager = PseudonymManager::with_random_secret();
        let e1 = manager.generate_ephemeral_pseudonym();
        let e2 = manager.generate_ephemeral_pseudonym();
        
        assert_ne!(e1, e2);
        assert!(e1.starts_with("eph_"));
    }
    
    #[test]
    fn test_logging_hash() {
        let manager = PseudonymManager::with_random_secret();
        let p = manager.generate_pseudonym("user", "conv");
        let hash = manager.hash_for_logging(&p.pseudonym_id);
        
        assert!(hash.starts_with("log_"));
        // Same pseudonym should produce same hash
        let hash2 = manager.hash_for_logging(&p.pseudonym_id);
        assert_eq!(hash, hash2);
    }
}
