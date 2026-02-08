// Keychain abstraction for secure credential storage
// Windows: Credential Manager
// macOS: Keychain (future)

use keyring::Entry;
use anyhow::{Result, Context};

pub struct Keychain;

impl Keychain {
    pub fn new() -> Self {
        Keychain
    }

    pub fn store(&self, service: &str, username: &str, password: &str) -> Result<()> {
        let entry = Entry::new(service, username)
            .context("Failed to create keychain entry")?;
        entry.set_password(password)
            .context("Failed to store password in keychain")?;
        Ok(())
    }

    pub fn retrieve(&self, service: &str, username: &str) -> Result<String> {
        let entry = Entry::new(service, username)
            .context("Failed to create keychain entry")?;
        let password = entry.get_password()
            .context("Failed to retrieve password from keychain")?;
        Ok(password)
    }

    pub fn delete(&self, service: &str, username: &str) -> Result<()> {
        let entry = Entry::new(service, username)
            .context("Failed to create keychain entry")?;
        entry.delete_password()
            .context("Failed to delete password from keychain")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn exists(&self, service: &str, username: &str) -> bool {
        let entry = match Entry::new(service, username) {
            Ok(e) => e,
            Err(_) => return false,
        };
        entry.get_password().is_ok()
    }
}

impl Default for Keychain {
    fn default() -> Self {
        Self::new()
    }
}
