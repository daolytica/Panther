// Provider adapter trait

use crate::types::{ProviderAccount, PromptPacket, NormalizedResponse};
use anyhow::Result;

#[async_trait::async_trait]
pub trait ProviderAdapter: Send + Sync {
    async fn validate(&self, config: &ProviderAccount) -> Result<bool>;
    async fn list_models(&self, config: &ProviderAccount) -> Result<Vec<String>>;
    async fn complete(
        &self,
        packet: &PromptPacket,
        config: &ProviderAccount,
        model: &str,
    ) -> Result<NormalizedResponse>;
    #[allow(dead_code)]
    async fn stream(
        &self,
        packet: &PromptPacket,
        config: &ProviderAccount,
        model: &str,
        on_chunk: Box<dyn Fn(String) + Send>,
    ) -> Result<NormalizedResponse>;
}
