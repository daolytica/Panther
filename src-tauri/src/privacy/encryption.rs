// Encryption layer for conversation storage
// Implements envelope encryption for chat history and redaction maps

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rand::RngCore;
use serde::{Deserialize, Serialize};

/// Encrypted data blob with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct EncryptedBlob {
    pub ciphertext: String,  // Base64-encoded encrypted data
    pub nonce: String,       // Base64-encoded nonce
    pub version: u8,         // Encryption version for future upgrades
}

/// Data Encryption Key (DEK) encrypted by Master Key
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct EncryptedDek {
    pub encrypted_key: String,  // Base64-encoded encrypted DEK
    pub nonce: String,          // Base64-encoded nonce
}

/// Conversation encryption service
#[allow(dead_code)]
pub struct ConversationEncryption {
    master_key: [u8; 32],
}

impl ConversationEncryption {
    /// Create with a master key (should be loaded from secure storage)
    #[allow(dead_code)]
    pub fn new(master_key: [u8; 32]) -> Self {
        Self { master_key }
    }
    
    /// Create with a master key from bytes
    #[allow(dead_code)]
    pub fn from_key_bytes(key: &[u8]) -> Result<Self, String> {
        if key.len() != 32 {
            return Err("Master key must be 32 bytes".to_string());
        }
        let mut master_key = [0u8; 32];
        master_key.copy_from_slice(key);
        Ok(Self { master_key })
    }
    
    /// Generate a new random master key (call once during app setup)
    #[allow(dead_code)]
    pub fn generate_master_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }
    
    /// Generate a new Data Encryption Key (DEK) for a conversation
    #[allow(dead_code)]
    pub fn generate_dek(&self) -> Result<(Vec<u8>, EncryptedDek), String> {
        // Generate random DEK
        let mut dek = [0u8; 32];
        OsRng.fill_bytes(&mut dek);
        
        // Encrypt DEK with master key
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let encrypted = cipher.encrypt(nonce, dek.as_ref())
            .map_err(|e| format!("Failed to encrypt DEK: {}", e))?;
        
        let encrypted_dek = EncryptedDek {
            encrypted_key: STANDARD.encode(&encrypted),
            nonce: STANDARD.encode(nonce_bytes),
        };
        
        Ok((dek.to_vec(), encrypted_dek))
    }
    
    /// Decrypt a DEK
    #[allow(dead_code)]
    pub fn decrypt_dek(&self, encrypted_dek: &EncryptedDek) -> Result<Vec<u8>, String> {
        let cipher = Aes256Gcm::new_from_slice(&self.master_key)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        
        let encrypted_bytes = STANDARD.decode(&encrypted_dek.encrypted_key)
            .map_err(|e| format!("Failed to decode encrypted DEK: {}", e))?;
        
        let nonce_bytes = STANDARD.decode(&encrypted_dek.nonce)
            .map_err(|e| format!("Failed to decode nonce: {}", e))?;
        
        if nonce_bytes.len() != 12 {
            return Err("Invalid nonce length".to_string());
        }
        
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        cipher.decrypt(nonce, encrypted_bytes.as_ref())
            .map_err(|e| format!("Failed to decrypt DEK: {}", e))
    }
    
    /// Encrypt data with a DEK
    #[allow(dead_code)]
    pub fn encrypt_with_dek(&self, plaintext: &[u8], dek: &[u8]) -> Result<EncryptedBlob, String> {
        if dek.len() != 32 {
            return Err("DEK must be 32 bytes".to_string());
        }
        
        let cipher = Aes256Gcm::new_from_slice(dek)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        let ciphertext = cipher.encrypt(nonce, plaintext)
            .map_err(|e| format!("Failed to encrypt: {}", e))?;
        
        Ok(EncryptedBlob {
            ciphertext: STANDARD.encode(&ciphertext),
            nonce: STANDARD.encode(nonce_bytes),
            version: 1,
        })
    }
    
    /// Decrypt data with a DEK
    #[allow(dead_code)]
    pub fn decrypt_with_dek(&self, blob: &EncryptedBlob, dek: &[u8]) -> Result<Vec<u8>, String> {
        if dek.len() != 32 {
            return Err("DEK must be 32 bytes".to_string());
        }
        
        let cipher = Aes256Gcm::new_from_slice(dek)
            .map_err(|e| format!("Failed to create cipher: {}", e))?;
        
        let ciphertext = STANDARD.decode(&blob.ciphertext)
            .map_err(|e| format!("Failed to decode ciphertext: {}", e))?;
        
        let nonce_bytes = STANDARD.decode(&blob.nonce)
            .map_err(|e| format!("Failed to decode nonce: {}", e))?;
        
        if nonce_bytes.len() != 12 {
            return Err("Invalid nonce length".to_string());
        }
        
        let nonce = Nonce::from_slice(&nonce_bytes);
        
        cipher.decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| format!("Failed to decrypt: {}", e))
    }
    
    /// Encrypt a string (convenience method)
    #[allow(dead_code)]
    pub fn encrypt_string(&self, plaintext: &str, dek: &[u8]) -> Result<EncryptedBlob, String> {
        self.encrypt_with_dek(plaintext.as_bytes(), dek)
    }
    
    /// Decrypt to string (convenience method)
    #[allow(dead_code)]
    pub fn decrypt_to_string(&self, blob: &EncryptedBlob, dek: &[u8]) -> Result<String, String> {
        let bytes = self.decrypt_with_dek(blob, dek)?;
        String::from_utf8(bytes).map_err(|e| format!("Invalid UTF-8: {}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let master_key = ConversationEncryption::generate_master_key();
        let encryption = ConversationEncryption::new(master_key);
        
        let (dek, encrypted_dek) = encryption.generate_dek().unwrap();
        
        let plaintext = "Hello, this is a secret message!";
        let blob = encryption.encrypt_string(plaintext, &dek).unwrap();
        let decrypted = encryption.decrypt_to_string(&blob, &dek).unwrap();
        
        assert_eq!(plaintext, decrypted);
    }
    
    #[test]
    fn test_dek_roundtrip() {
        let master_key = ConversationEncryption::generate_master_key();
        let encryption = ConversationEncryption::new(master_key);
        
        let (original_dek, encrypted_dek) = encryption.generate_dek().unwrap();
        let decrypted_dek = encryption.decrypt_dek(&encrypted_dek).unwrap();
        
        assert_eq!(original_dek, decrypted_dek);
    }
    
    #[test]
    fn test_wrong_key_fails() {
        let master_key = ConversationEncryption::generate_master_key();
        let encryption = ConversationEncryption::new(master_key);
        
        let (dek, _) = encryption.generate_dek().unwrap();
        let blob = encryption.encrypt_string("secret", &dek).unwrap();
        
        // Try decrypting with wrong key
        let wrong_dek = vec![0u8; 32];
        let result = encryption.decrypt_to_string(&blob, &wrong_dek);
        
        assert!(result.is_err());
    }
}
