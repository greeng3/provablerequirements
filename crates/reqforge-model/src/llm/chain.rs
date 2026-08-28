//! Fallback chain dispatcher.
//!
//! Walks the priority-ordered `&[Box<dyn Adapter>]` top-to-
//! bottom. For each slot:
//!
//! 1. If `HealthTracker::should_skip` says skip, skip.
//! 2. Call `send_prompt`.
//! 3. On success: record success, return the response.
//! 4. On failure: record failure (drives the state machine),
//!    then try the next slot.
//!
//! If every slot is skipped or fails, returns `ChainError`
//! containing the per-slot outcome so callers can surface
//! actionable detail ("provider X was rate-limited, provider
//! Y has no API key, provider Z is hard-disabled").

use super::health::HealthTracker;
use super::provider::{Adapter, AdapterError, PromptRequest, PromptResponse};

/// Outcome for one slot that the chain tried (or skipped)
/// during a single `run_chain` call. Included in the error
/// returned when every slot fails, so operators can see why
/// fallback didn't help.
#[derive(Debug)]
pub enum SlotOutcome {
    /// Slot was skipped without an attempt because the
    /// health tracker said so (transient-degraded with an
    /// open window, or hard-disabled).
    Skipped { reason: &'static str },
    /// Slot was attempted and the adapter returned an error.
    Failed(AdapterError),
}

/// Failure modes for the chain itself.
#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("no LLM providers are configured")]
    NoProviders,
    #[error("all {count} configured provider(s) were skipped or failed")]
    AllFailed {
        count: usize,
        outcomes: Vec<SlotOutcome>,
    },
}

/// Walk the provider chain, trying each eligible slot until
/// one succeeds. Returns the successful response and the
/// index of the slot that served it (for telemetry and the
/// "served by" surface in Phase 10b).
pub async fn run_chain<C: crate::llm::health::Clock>(
    providers: &[Box<dyn Adapter>],
    health: &HealthTracker<C>,
    req: &PromptRequest,
) -> Result<(usize, PromptResponse), ChainError> {
    if providers.is_empty() {
        return Err(ChainError::NoProviders);
    }
    let mut outcomes = Vec::with_capacity(providers.len());
    for (index, adapter) in providers.iter().enumerate() {
        if health.should_skip(index) {
            outcomes.push(SlotOutcome::Skipped {
                reason: "health tracker says skip",
            });
            continue;
        }
        match adapter.send_prompt(req).await {
            Ok(resp) => {
                health.record_success(index);
                return Ok((index, resp));
            }
            Err(err) => {
                health.record_failure(index, &err);
                outcomes.push(SlotOutcome::Failed(err));
            }
        }
    }
    Err(ChainError::AllFailed {
        count: providers.len(),
        outcomes,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::llm::config::ProviderFamily;
    use crate::llm::health::SystemClock;
    use crate::llm::provider::{BoxFuture, PromptMessage, PromptRole};

    /// Test adapter: a fixed-response factory. Each instance
    /// owns a queue of pre-baked `Result`s; `send_prompt`
    /// pops the front each call.
    struct FakeAdapter {
        family: ProviderFamily,
        model: String,
        endpoint: String,
        api_key_available: bool,
        responses: Mutex<std::collections::VecDeque<Result<PromptResponse, AdapterError>>>,
    }

    impl FakeAdapter {
        fn new(
            family: ProviderFamily,
            responses: Vec<Result<PromptResponse, AdapterError>>,
        ) -> Self {
            Self {
                family,
                model: "test-model".into(),
                endpoint: "http://fake.test".into(),
                api_key_available: true,
                responses: Mutex::new(responses.into_iter().collect()),
            }
        }
    }

    impl Adapter for FakeAdapter {
        fn family(&self) -> ProviderFamily {
            self.family
        }
        fn model(&self) -> &str {
            &self.model
        }
        fn endpoint(&self) -> &str {
            &self.endpoint
        }
        fn api_key_available(&self) -> bool {
            self.api_key_available
        }
        fn send_prompt<'a>(
            &'a self,
            _req: &'a PromptRequest,
        ) -> BoxFuture<'a, Result<PromptResponse, AdapterError>> {
            let next = self.responses.lock().unwrap().pop_front().unwrap_or(Err(
                AdapterError::Malformed {
                    family: "fake",
                    detail: "queue exhausted".into(),
                },
            ));
            Box::pin(async move { next })
        }
    }

    fn resp(text: &str) -> PromptResponse {
        PromptResponse {
            text: text.into(),
            usage: None,
        }
    }

    fn req() -> PromptRequest {
        PromptRequest {
            system: None,
            messages: vec![PromptMessage {
                role: PromptRole::User,
                content: "hi".into(),
            }],
            max_tokens: 8,
            temperature: 0.0,
            timeout_ms: None,
        }
    }

    fn timeout() -> AdapterError {
        AdapterError::Timeout {
            family: "fake",
            ms: 1000,
        }
    }

    fn auth() -> AdapterError {
        AdapterError::Auth {
            family: "fake",
            detail: "401".into(),
        }
    }

    #[tokio::test]
    async fn first_success_returns_immediately_and_marks_healthy() {
        let providers: Vec<Box<dyn Adapter>> = vec![
            Box::new(FakeAdapter::new(
                ProviderFamily::Anthropic,
                vec![Ok(resp("a"))],
            )),
            Box::new(FakeAdapter::new(
                ProviderFamily::Gemini,
                vec![Ok(resp("b"))],
            )),
        ];
        let health = HealthTracker::<SystemClock>::new();
        let (index, response) = run_chain(&providers, &health, &req()).await.unwrap();
        assert_eq!(index, 0);
        assert_eq!(response.text, "a");
    }

    #[tokio::test]
    async fn transient_failure_falls_through_to_next_slot() {
        let providers: Vec<Box<dyn Adapter>> = vec![
            Box::new(FakeAdapter::new(
                ProviderFamily::Anthropic,
                vec![Err(timeout())],
            )),
            Box::new(FakeAdapter::new(
                ProviderFamily::Gemini,
                vec![Ok(resp("fallback"))],
            )),
        ];
        let health = HealthTracker::<SystemClock>::new();
        let (index, response) = run_chain(&providers, &health, &req()).await.unwrap();
        assert_eq!(index, 1);
        assert_eq!(response.text, "fallback");
        // Slot 0 is now transient-degraded.
        assert!(health.should_skip(0));
    }

    #[tokio::test]
    async fn permanent_failure_falls_through_and_hard_disables() {
        let providers: Vec<Box<dyn Adapter>> = vec![
            Box::new(FakeAdapter::new(
                ProviderFamily::Anthropic,
                vec![Err(auth())],
            )),
            Box::new(FakeAdapter::new(
                ProviderFamily::Gemini,
                vec![Ok(resp("b"))],
            )),
        ];
        let health = HealthTracker::<SystemClock>::new();
        let (index, _) = run_chain(&providers, &health, &req()).await.unwrap();
        assert_eq!(index, 1);
        assert!(matches!(
            health.state(0),
            crate::llm::health::HealthState::HardDisabled
        ));
    }

    #[tokio::test]
    async fn all_failed_returns_outcomes_per_slot() {
        let providers: Vec<Box<dyn Adapter>> = vec![
            Box::new(FakeAdapter::new(
                ProviderFamily::Anthropic,
                vec![Err(timeout())],
            )),
            Box::new(FakeAdapter::new(ProviderFamily::Gemini, vec![Err(auth())])),
        ];
        let health = HealthTracker::<SystemClock>::new();
        let err = run_chain(&providers, &health, &req()).await.unwrap_err();
        match err {
            ChainError::AllFailed { count, outcomes } => {
                assert_eq!(count, 2);
                assert_eq!(outcomes.len(), 2);
                assert!(matches!(outcomes[0], SlotOutcome::Failed(_)));
                assert!(matches!(outcomes[1], SlotOutcome::Failed(_)));
            }
            other => panic!("expected AllFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn empty_chain_errors() {
        let providers: Vec<Box<dyn Adapter>> = Vec::new();
        let health = HealthTracker::<SystemClock>::new();
        let err = run_chain(&providers, &health, &req()).await.unwrap_err();
        assert!(matches!(err, ChainError::NoProviders));
    }

    #[tokio::test]
    async fn hard_disabled_slots_are_skipped_without_calling_adapter() {
        // Slot 0 has no responses queued — if the chain
        // tried to call it, the queue-empty path would
        // produce a Malformed error, which we'd detect.
        let providers: Vec<Box<dyn Adapter>> = vec![
            Box::new(FakeAdapter::new(ProviderFamily::Anthropic, vec![])),
            Box::new(FakeAdapter::new(
                ProviderFamily::Gemini,
                vec![Ok(resp("b"))],
            )),
        ];
        let health = HealthTracker::<SystemClock>::new();
        // Pre-disable slot 0. If the chain had called its
        // adapter, that adapter's empty response queue would
        // yield a Malformed error — which would flip slot 0
        // off HardDisabled and onto transient state. The
        // post-call state check below is the assertion.
        health.record_failure(0, &auth());
        let (index, _) = run_chain(&providers, &health, &req()).await.unwrap();
        assert_eq!(index, 1);
        assert!(matches!(
            health.state(0),
            crate::llm::health::HealthState::HardDisabled
        ));
    }
}
