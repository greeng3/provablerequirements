//! Concrete provider adapters.
//!
//! One adapter per family shipped in 10a. [`build_adapter`]
//! is the factory that turns a typed [`ProviderConfig`] into
//! the trait-object the chain walks — all three families
//! share the same `Adapter` surface, so upstream code never
//! branches on family after this point.

mod anthropic;
mod common;
mod gemini;
mod openai;

use super::config::{ProviderConfig, ProviderFamily};
use super::provider::Adapter;

pub use anthropic::AnthropicAdapter;
pub use gemini::GeminiAdapter;
pub use openai::OpenAiCompatibleAdapter;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("`llm[{index}].endpoint` is required for the openai-compatible adapter")]
    EndpointRequired { index: usize },
}

/// Turn a typed `ProviderConfig` into a concrete adapter.
///
/// `index` is the slot position from `SystemConfig.llm` —
/// only used to make `BuildError` messages locate the
/// offending entry. Config parsing already validates that
/// `openai-compatible` entries carry an endpoint (the index
/// check here is a belt-and-braces safeguard for callers
/// that skip `llm::config::parse_llm`).
pub fn build_adapter(
    index: usize,
    config: &ProviderConfig,
) -> Result<Box<dyn Adapter>, BuildError> {
    Ok(match config.provider {
        ProviderFamily::OpenaiCompatible => {
            let endpoint = config
                .endpoint
                .clone()
                .ok_or(BuildError::EndpointRequired { index })?;
            Box::new(OpenAiCompatibleAdapter::new(
                endpoint,
                config.model.clone(),
                config.api_key.clone(),
            ))
        }
        ProviderFamily::Anthropic => Box::new(AnthropicAdapter::new(
            config.endpoint.clone(),
            config.model.clone(),
            config.api_key.clone(),
        )),
        ProviderFamily::Gemini => Box::new(GeminiAdapter::new(
            config.endpoint.clone(),
            config.model.clone(),
            config.api_key.clone(),
        )),
    })
}
