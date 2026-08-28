//! Process-lifetime LLM runtime.
//!
//! Bundles the configured adapters with the per-process
//! health tracker and privacy tracker. Built once from the
//! typed `ProviderConfig` list (itself parsed from
//! `SystemConfig.llm`) and mounted on [`AppState`]. All
//! mutable state (backoff windows, hard-disable flags, ack
//! records) is in memory only — per the locked decision that
//! restart clears LLM state.

use std::sync::Arc;

use super::adapters::build_adapter;
use super::chain::{ChainError, SlotOutcome};
use super::config::ProviderConfig;
use super::health::HealthTracker;
use super::privacy::PrivacyTracker;
use super::provider::{
    Adapter, AdapterError, PromptMessage, PromptRequest, PromptResponse, PromptRole,
};

pub struct LlmRuntime {
    providers: Vec<ProviderConfig>,
    adapters: Vec<Box<dyn Adapter>>,
    health: HealthTracker,
    privacy: PrivacyTracker,
}

impl std::fmt::Debug for LlmRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LlmRuntime")
            .field("provider_count", &self.adapters.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RuntimeBuildError {
    #[error("llm[{index}]: {source}")]
    Adapter {
        index: usize,
        #[source]
        source: super::adapters::BuildError,
    },
}

impl LlmRuntime {
    /// Build a runtime from the typed provider list.
    /// Returns an empty runtime when the list is empty
    /// (`LLM-optional`: the server runs fine with no LLM).
    pub fn build(configs: Vec<ProviderConfig>) -> Result<Arc<Self>, RuntimeBuildError> {
        let mut adapters: Vec<Box<dyn Adapter>> = Vec::with_capacity(configs.len());
        for (index, cfg) in configs.iter().enumerate() {
            let adapter = build_adapter(index, cfg)
                .map_err(|source| RuntimeBuildError::Adapter { index, source })?;
            adapters.push(adapter);
        }
        Ok(Arc::new(Self {
            providers: configs,
            adapters,
            health: HealthTracker::new(),
            privacy: PrivacyTracker::new(),
        }))
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    pub fn provider_count(&self) -> usize {
        self.adapters.len()
    }

    pub fn providers(&self) -> &[ProviderConfig] {
        &self.providers
    }

    pub fn adapters(&self) -> &[Box<dyn Adapter>] {
        &self.adapters
    }

    pub fn health(&self) -> &HealthTracker {
        &self.health
    }

    pub fn privacy(&self) -> &PrivacyTracker {
        &self.privacy
    }

    /// Bounds-check a provider index against the configured
    /// list. Handlers use this before reaching into any of
    /// the trackers.
    pub fn valid_index(&self, index: usize) -> bool {
        index < self.adapters.len()
    }

    /// Run the fallback chain over the full adapter list,
    /// skipping:
    ///
    /// - providers whose `health.should_skip` says skip
    ///   (transient-degraded window open, or hard-disabled);
    /// - non-local providers whose privacy warning hasn't
    ///   been acknowledged this process (`LLM-privacyWarning`).
    ///
    /// Per-slot success/failure updates the central health
    /// tracker. Returns the original slot index that served
    /// the response.
    pub async fn run_prompt(
        &self,
        req: &PromptRequest,
    ) -> Result<(usize, PromptResponse), ChainError> {
        if self.adapters.is_empty() {
            return Err(ChainError::NoProviders);
        }
        let mut outcomes = Vec::with_capacity(self.adapters.len());
        for (index, adapter) in self.adapters.iter().enumerate() {
            if !self.providers[index].is_enabled() {
                outcomes.push(SlotOutcome::Skipped {
                    reason: "provider is disabled in the System config",
                });
                continue;
            }
            if self.health.should_skip(index) {
                outcomes.push(SlotOutcome::Skipped {
                    reason: "provider is transient-degraded or hard-disabled",
                });
                continue;
            }
            if self.privacy.requires_ack(index, adapter.endpoint()) {
                outcomes.push(SlotOutcome::Skipped {
                    reason: "privacy warning not yet acknowledged for this provider",
                });
                continue;
            }
            match adapter.send_prompt(req).await {
                Ok(resp) => {
                    self.health.record_success(index);
                    return Ok((index, resp));
                }
                Err(err) => {
                    self.health.record_failure(index, &err);
                    outcomes.push(SlotOutcome::Failed(err));
                }
            }
        }
        Err(ChainError::AllFailed {
            count: self.adapters.len(),
            outcomes,
        })
    }

    /// Operator-triggered retest. Forces the slot back to
    /// `Healthy`, then fires a minimal probe. The probe's
    /// success or failure drives the normal health-state
    /// recording, which means a still-broken provider just
    /// lands back in `HardDisabled` / `TransientDegraded`.
    ///
    /// Bypasses the privacy-ack gate — retest is a signal
    /// from the operator that they want to hit the provider
    /// right now.
    pub async fn retest(&self, index: usize) -> Result<(), AdapterError> {
        if !self.valid_index(index) {
            return Err(AdapterError::Malformed {
                family: "llm-runtime",
                detail: format!("provider index {index} out of range"),
            });
        }
        self.health.force_healthy(index);
        let req = PromptRequest {
            system: None,
            messages: vec![PromptMessage {
                role: PromptRole::User,
                content: "ping".into(),
            }],
            max_tokens: 4,
            temperature: 0.0,
            timeout_ms: None,
        };
        match self.adapters[index].send_prompt(&req).await {
            Ok(_) => {
                self.health.record_success(index);
                Ok(())
            }
            Err(err) => {
                self.health.record_failure(index, &err);
                Err(err)
            }
        }
    }
}
