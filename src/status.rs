//! Coverage funnel (R-cov-1): `discovered → triaged → formalized → verified`,
//! keyed by item id, with the honest states kept distinct — un-triaged is not
//! stays-prose is not formalizable-but-not-yet-formalized. Extends the A4
//! traceability model on the triage axis.
//!
//! Implements: REQ011 (report requirement coverage as an honest funnel)

use crate::source::{Classification, Item};
use crate::triage::TriageState;

/// A snapshot of where every discovered item sits in the funnel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub discovered: usize,
    pub untriaged: usize,
    pub formalizable_now: usize,
    pub falsifiable_only: usize,
    pub stays_prose: usize,
    /// Step 3 — not built yet, honestly reported as 0 rather than hidden.
    pub formalized: usize,
    /// Step 4 — not built yet, honestly reported as 0.
    pub verified: usize,
}

/// Compute the funnel for `items` given the current triage `state`.
pub fn coverage(items: &[Item], state: &TriageState) -> Coverage {
    let mut cov = Coverage {
        discovered: items.len(),
        untriaged: 0,
        formalizable_now: 0,
        falsifiable_only: 0,
        stays_prose: 0,
        formalized: 0,
        verified: 0,
    };
    for item in items {
        match state.items.get(&item.id).map(|e| e.classification) {
            None => cov.untriaged += 1,
            Some(Classification::FormalizableNow) => cov.formalizable_now += 1,
            Some(Classification::FalsifiableOnly) => cov.falsifiable_only += 1,
            Some(Classification::StaysProse) => cov.stays_prose += 1,
        }
    }
    cov
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triage::{seed, set, ProseFloorClassifier, TriageState};

    fn item(id: &str) -> Item {
        Item {
            id: id.into(),
            text: id.into(),
            revision: id.into(),
            title: None,
            verification_hint: None,
        }
    }

    // Verifies: REQ011 — untriaged, stays-prose, and formalizable are distinct
    // funnel states, and unbuilt stages report an honest zero.
    #[tokio::test]
    async fn funnel_keeps_states_distinct() {
        let items = [item("A"), item("B"), item("C")];

        // Nothing triaged yet.
        let empty = coverage(&items, &TriageState::new());
        assert_eq!(empty.discovered, 3);
        assert_eq!(empty.untriaged, 3);

        // Seed all to prose, then promote A.
        let seeded = seed(&TriageState::new(), &items, &ProseFloorClassifier)
            .await
            .unwrap();
        let promoted = set(&seeded, &items[0], Classification::FormalizableNow);
        let cov = coverage(&items, &promoted);
        assert_eq!(cov.untriaged, 0);
        assert_eq!(cov.formalizable_now, 1);
        assert_eq!(cov.stays_prose, 2);
        assert_eq!(cov.formalized, 0);
        assert_eq!(cov.verified, 0);
    }
}
