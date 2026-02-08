// Provider adapters module

pub mod openai;
pub mod local_http;
pub mod anthropic;
pub mod google;
pub mod ollama;
pub mod grok;
pub mod adapter_trait;

pub use adapter_trait::ProviderAdapter;
pub use openai::OpenAIAdapter;
pub use local_http::LocalHTTPAdapter;
pub use anthropic::AnthropicAdapter;
pub use google::GoogleAdapter;
pub use ollama::OllamaAdapter;
pub use grok::GrokAdapter;

use anyhow::Result;

pub fn get_adapter(provider_type: &str) -> Result<Box<dyn ProviderAdapter>> {
    match provider_type {
        "openai_compatible" => Ok(Box::new(OpenAIAdapter::new())),
        "local_http" => Ok(Box::new(LocalHTTPAdapter::new())),
        "anthropic" => Ok(Box::new(AnthropicAdapter::new())),
        "google" => Ok(Box::new(GoogleAdapter::new())),
        "ollama" => Ok(Box::new(OllamaAdapter::new())),
        "grok" => Ok(Box::new(GrokAdapter::new())),
        _ => anyhow::bail!("Unsupported provider type: '{}'. Supported types: 'openai_compatible', 'local_http', 'anthropic', 'google', 'ollama', 'grok'", provider_type),
    }
}
