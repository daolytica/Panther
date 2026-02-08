// Privacy module - Identity Concealment Layer for LLM API Chat
// Provides PII redaction, pseudonymous sessions, and encrypted storage

pub mod redaction;
pub mod pseudonym;
pub mod encryption;
pub mod sanitized_logger;
pub mod context_compactor;

pub use redaction::{PiiRedactor, RedactionStats};
pub use pseudonym::PseudonymManager;
pub use context_compactor::ContextCompactor;

// These are available for future use when encryption is fully implemented
// They are intentionally not exported to avoid unused import warnings
// Uncomment when needed:
// pub use redaction::{RedactionResult, RedactionMap};
// pub use pseudonym::ConversationPseudonym;
// pub use encryption::{ConversationEncryption, EncryptedBlob};
// pub use sanitized_logger::SanitizedLogger;
