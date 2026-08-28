//! Three-state health tracker per `LLM-healthTracking`.
//!
//! Transitions:
//!
//! - `Healthy` → `TransientDegraded` on a transient failure
//!   (`Timeout` / `RateLimited` / `ServerError`). The first
//!   backoff is `BASE_BACKOFF`; each subsequent transient
//!   failure while already degraded doubles the backoff,
//!   capped at `MAX_BACKOFF`.
//! - `Healthy` → `HardDisabled` on a permanent failure
//!   (`Auth` / `ModelNotFound` / `Connection` / `Malformed`).
//!   `HardDisabled` is sticky for the lifetime of the
//!   process — operators must fix the config and restart.
//! - `TransientDegraded` → `Healthy` on success.
//! - `TransientDegraded` → `HardDisabled` on any permanent
//!   failure (same as from `Healthy`).
//! - `HardDisabled` → anything: never. Only a restart clears
//!   it.
//!
//! The tracker is keyed by provider index (the array index
//! from `SystemConfig.llm`) so it doesn't need to know about
//! provider identity beyond "which slot in the chain".

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;

use super::provider::{AdapterError, AdapterErrorKind};

pub const BASE_BACKOFF: Duration = Duration::from_secs(30);
pub const MAX_BACKOFF: Duration = Duration::from_secs(30 * 60);

/// Three-state view of a provider's health at a point in
/// time. Serialised directly into the provider-list response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum HealthState {
    Healthy,
    /// Backoff window still open; `retry_after_secs` tells
    /// operators (and the chain) when the provider will next
    /// be retried. Zero means the window has already passed
    /// and the next `run_chain` call will include it again.
    TransientDegraded {
        retry_after_secs: u64,
    },
    /// Permanently out for this process. Won't be retried.
    HardDisabled,
}

/// Abstraction over "now" so backoff tests don't have to
/// sleep. Production uses [`SystemClock`]; tests use a
/// custom implementation that moves time on demand.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> Instant;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

/// Internal state for one provider slot. Not exported —
/// external callers see `HealthState` snapshots instead.
#[derive(Debug, Clone, Copy)]
enum Internal {
    Healthy,
    Degraded {
        /// Time after which the next call attempt is allowed.
        ready_at: Instant,
        /// Current backoff length (doubles on each failure
        /// while degraded, cap at `MAX_BACKOFF`).
        current: Duration,
    },
    Hard,
}

pub struct HealthTracker<C: Clock = SystemClock> {
    clock: C,
    slots: Mutex<HashMap<usize, Internal>>,
}

impl HealthTracker<SystemClock> {
    pub fn new() -> Self {
        Self::with_clock(SystemClock)
    }
}

impl Default for HealthTracker<SystemClock> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: Clock> HealthTracker<C> {
    pub fn with_clock(clock: C) -> Self {
        Self {
            clock,
            slots: Mutex::new(HashMap::new()),
        }
    }

    /// Snapshot the provider's current health. Defaults to
    /// `Healthy` for a slot that has never been exercised.
    pub fn state(&self, index: usize) -> HealthState {
        let slots = self.slots.lock().expect("health tracker poisoned");
        match slots.get(&index).copied() {
            None | Some(Internal::Healthy) => HealthState::Healthy,
            Some(Internal::Degraded { ready_at, .. }) => {
                let now = self.clock.now();
                let remaining = ready_at.saturating_duration_since(now);
                HealthState::TransientDegraded {
                    retry_after_secs: remaining.as_secs(),
                }
            }
            Some(Internal::Hard) => HealthState::HardDisabled,
        }
    }

    /// Whether the fallback chain should skip this provider
    /// right now. A transient-degraded provider is skipped
    /// until its backoff window elapses, at which point it
    /// becomes eligible again (and the next success returns
    /// it to `Healthy`).
    pub fn should_skip(&self, index: usize) -> bool {
        let slots = self.slots.lock().expect("health tracker poisoned");
        match slots.get(&index).copied() {
            None | Some(Internal::Healthy) => false,
            Some(Internal::Degraded { ready_at, .. }) => self.clock.now() < ready_at,
            Some(Internal::Hard) => true,
        }
    }

    /// Record a successful call — promotes a degraded slot
    /// back to healthy and clears any accumulated backoff.
    /// No-op if already healthy; does NOT clear hard-disabled
    /// (those only clear on restart).
    pub fn record_success(&self, index: usize) {
        let mut slots = self.slots.lock().expect("health tracker poisoned");
        match slots.get(&index).copied() {
            Some(Internal::Hard) => { /* sticky */ }
            _ => {
                slots.insert(index, Internal::Healthy);
            }
        }
    }

    /// Record a failure. Drives the transient-vs-permanent
    /// transition based on `AdapterError::kind`.
    pub fn record_failure(&self, index: usize, err: &AdapterError) {
        let mut slots = self.slots.lock().expect("health tracker poisoned");
        let current = slots.get(&index).copied();
        let next = match (current, err.kind()) {
            (Some(Internal::Hard), _) => Internal::Hard,
            (_, AdapterErrorKind::Permanent) => Internal::Hard,
            (Some(Internal::Degraded { current, .. }), AdapterErrorKind::Transient) => {
                let doubled = current.saturating_mul(2);
                let next = doubled.min(MAX_BACKOFF);
                Internal::Degraded {
                    ready_at: self.clock.now() + next,
                    current: next,
                }
            }
            (_, AdapterErrorKind::Transient) => Internal::Degraded {
                ready_at: self.clock.now() + BASE_BACKOFF,
                current: BASE_BACKOFF,
            },
        };
        slots.insert(index, next);
    }

    /// Clear transient-degraded backoff so the provider is
    /// eligible on the next call. Does NOT clear
    /// `HardDisabled` — that requires the explicit
    /// `force_healthy` path (operator-triggered retest).
    pub fn clear_transient(&self, index: usize) {
        let mut slots = self.slots.lock().expect("health tracker poisoned");
        match slots.get(&index).copied() {
            Some(Internal::Hard) => {}
            _ => {
                slots.insert(index, Internal::Healthy);
            }
        }
    }

    /// Unconditionally move the slot to `Healthy`. Used by
    /// the operator-triggered retest flow per ROADMAP:
    /// "Hard-disabled stays until a retest hits it." The
    /// retest then sends a probe; normal success/failure
    /// recording reclassifies the slot from the probe result.
    pub fn force_healthy(&self, index: usize) {
        self.slots
            .lock()
            .expect("health tracker poisoned")
            .insert(index, Internal::Healthy);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex as StdMutex;

    use super::*;

    struct FakeClock {
        now: StdMutex<Instant>,
    }

    impl FakeClock {
        fn new() -> Self {
            Self {
                now: StdMutex::new(Instant::now()),
            }
        }

        fn advance(&self, d: Duration) {
            let mut g = self.now.lock().unwrap();
            *g += d;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            *self.now.lock().unwrap()
        }
    }

    fn timeout() -> AdapterError {
        AdapterError::Timeout {
            family: "anthropic",
            ms: 30_000,
        }
    }

    fn auth() -> AdapterError {
        AdapterError::Auth {
            family: "anthropic",
            detail: "401".into(),
        }
    }

    #[test]
    fn unknown_slot_is_healthy() {
        let t = HealthTracker::<SystemClock>::new();
        assert_eq!(t.state(0), HealthState::Healthy);
        assert!(!t.should_skip(0));
    }

    #[test]
    fn transient_failure_enters_degraded_with_base_backoff() {
        let clock = FakeClock::new();
        let t = HealthTracker::with_clock(clock);
        t.record_failure(0, &timeout());
        match t.state(0) {
            HealthState::TransientDegraded { retry_after_secs } => {
                assert_eq!(retry_after_secs, BASE_BACKOFF.as_secs());
            }
            other => panic!("expected degraded, got {other:?}"),
        }
        assert!(t.should_skip(0));
    }

    #[test]
    fn repeated_transient_failures_double_the_backoff_up_to_cap() {
        let clock = FakeClock::new();
        let t = HealthTracker::with_clock(clock);
        t.record_failure(0, &timeout());
        // Second failure while degraded → doubles to 60s.
        t.record_failure(0, &timeout());
        match t.state(0) {
            HealthState::TransientDegraded { retry_after_secs } => {
                assert_eq!(retry_after_secs, BASE_BACKOFF.as_secs() * 2);
            }
            other => panic!("expected degraded, got {other:?}"),
        }
        // Keep doubling until cap. 30, 60, 120, 240, 480, 960, 1800, 1800, ...
        for _ in 0..20 {
            t.record_failure(0, &timeout());
        }
        match t.state(0) {
            HealthState::TransientDegraded { retry_after_secs } => {
                assert_eq!(retry_after_secs, MAX_BACKOFF.as_secs());
            }
            other => panic!("expected degraded, got {other:?}"),
        }
    }

    #[test]
    fn transient_window_elapses_then_slot_eligible_again() {
        let clock = FakeClock::new();
        let t = HealthTracker::with_clock(clock);
        t.record_failure(0, &timeout());
        assert!(t.should_skip(0));
        // Jump clock past the backoff.
        t.clock.advance(BASE_BACKOFF + Duration::from_secs(1));
        assert!(!t.should_skip(0), "window elapsed → eligible");
        // But state() still reports degraded (zero remaining)
        // until a success clears it. This is intentional —
        // it tells operators the provider is on probation.
        match t.state(0) {
            HealthState::TransientDegraded { retry_after_secs } => {
                assert_eq!(retry_after_secs, 0);
            }
            other => panic!("expected degraded (0), got {other:?}"),
        }
    }

    #[test]
    fn permanent_failure_hard_disables_immediately() {
        let t = HealthTracker::<SystemClock>::new();
        t.record_failure(0, &auth());
        assert_eq!(t.state(0), HealthState::HardDisabled);
        assert!(t.should_skip(0));
    }

    #[test]
    fn hard_disabled_is_sticky_across_subsequent_calls() {
        let t = HealthTracker::<SystemClock>::new();
        t.record_failure(0, &auth());
        // A later "success" must not un-stick this.
        t.record_success(0);
        assert_eq!(t.state(0), HealthState::HardDisabled);
        // Even a retest-clear must not un-stick it.
        t.clear_transient(0);
        assert_eq!(t.state(0), HealthState::HardDisabled);
    }

    #[test]
    fn success_clears_transient_degraded_back_to_healthy() {
        let t = HealthTracker::<SystemClock>::new();
        t.record_failure(0, &timeout());
        t.record_success(0);
        assert_eq!(t.state(0), HealthState::Healthy);
    }

    #[test]
    fn clear_transient_clears_degraded_but_not_hard() {
        let t = HealthTracker::<SystemClock>::new();
        t.record_failure(0, &timeout());
        t.clear_transient(0);
        assert_eq!(t.state(0), HealthState::Healthy);

        t.record_failure(1, &auth());
        t.clear_transient(1);
        assert_eq!(t.state(1), HealthState::HardDisabled);
    }

    #[test]
    fn slots_are_independent() {
        let t = HealthTracker::<SystemClock>::new();
        t.record_failure(0, &timeout());
        t.record_failure(2, &auth());
        assert!(matches!(t.state(0), HealthState::TransientDegraded { .. }));
        assert_eq!(t.state(1), HealthState::Healthy);
        assert_eq!(t.state(2), HealthState::HardDisabled);
    }
}
