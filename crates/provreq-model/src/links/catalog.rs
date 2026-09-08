//! Built-in and effective link-type catalog.

use crate::schema::SystemLinkType;
use crate::system::LoadedSystem;

/// Whether a `LinkType` entry came from the hard-coded baseline or
/// from a System-declared extension (`TRACE-linkExtensibility`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkTypeSource {
    Builtin,
    System,
}

/// One link-type definition. The four metadata fields mirror
/// `SystemLinkType` so built-in and System-declared entries can be
/// handled uniformly downstream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkType {
    pub name: &'static str,
    pub inverse_name: &'static str,
    pub directed: bool,
    pub acyclic: bool,
    pub source: LinkTypeSource,
}

impl LinkType {
    const fn builtin(
        name: &'static str,
        inverse_name: &'static str,
        directed: bool,
        acyclic: bool,
    ) -> Self {
        Self {
            name,
            inverse_name,
            directed,
            acyclic,
            source: LinkTypeSource::Builtin,
        }
    }
}

/// The seven built-in link types from `TRACE-linkCatalog`.
pub const BUILTINS: &[LinkType] = &[
    LinkType::builtin("derives-from", "derived-into", true, true),
    LinkType::builtin("satisfies", "satisfied-by", true, false),
    LinkType::builtin("verifies", "verified-by", true, false),
    LinkType::builtin("supersedes", "superseded-by", true, true),
    LinkType::builtin("cites", "cited-by", true, false),
    LinkType::builtin("conflicts-with", "conflicts-with", false, false),
    LinkType::builtin("related-to", "related-to", false, false),
];

/// Borrowed view of the built-in catalog. Useful when the caller
/// only needs read access and doesn't want to allocate.
pub fn builtin_catalog() -> &'static [LinkType] {
    BUILTINS
}

/// Returns the combined built-in + System-declared link catalog.
///
/// Built-ins always come first and always win on name collision
/// (per `TRACE-linkExtensibility` — "Built-in link types … shall
/// not be overridden by System-level declarations"). System
/// entries whose name matches a built-in are dropped from the
/// output so the picker and validator see a single authoritative
/// definition per name.
pub fn effective_catalog(system: &LoadedSystem) -> Vec<LinkType> {
    let mut out: Vec<LinkType> = BUILTINS.to_vec();
    let Some(config) = system.config() else {
        return out;
    };
    for declared in &config.link_types {
        if BUILTINS.iter().any(|b| b.name == declared.name) {
            continue;
        }
        out.push(system_link_type_to_owned(declared));
    }
    out
}

fn system_link_type_to_owned(declared: &SystemLinkType) -> LinkType {
    // LinkType holds `&'static str` for the built-in case; for
    // System-declared types we leak the strings so the single
    // `LinkType` shape works for both paths. Catalog refreshes are
    // rare (only on discovery / refresh) and the number of System
    // types is tiny, so the leak is bounded and acceptable — an
    // alternative would be splitting into Owned/Borrowed variants,
    // which buys correctness at real ergonomic cost.
    LinkType {
        name: Box::leak(declared.name.clone().into_boxed_str()),
        inverse_name: Box::leak(declared.inverse_name.clone().into_boxed_str()),
        directed: declared.directed,
        acyclic: declared.acyclic,
        source: LinkTypeSource::System,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{SystemConfig, SystemLinkType};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn named_system(types: Vec<SystemLinkType>) -> LoadedSystem {
        LoadedSystem::Named {
            config: Box::new(SystemConfig {
                schema_version: 1,
                name: "test".to_owned(),
                projects: Vec::new(),
                link_types: types,
                languages: None,
                llm: None,
                overflow: BTreeMap::new(),
            }),
            source_path: PathBuf::from("/fake/system.json"),
        }
    }

    fn system_type(name: &str, inverse: &str, directed: bool, acyclic: bool) -> SystemLinkType {
        SystemLinkType {
            name: name.to_owned(),
            inverse_name: inverse.to_owned(),
            directed,
            acyclic,
            overflow: BTreeMap::new(),
        }
    }

    #[test]
    fn builtins_has_exactly_seven_types_with_spec_flags() {
        let builtins = builtin_catalog();
        assert_eq!(builtins.len(), 7, "spec pins the built-in count at seven");

        let by_name: std::collections::HashMap<_, _> =
            builtins.iter().map(|b| (b.name, b)).collect();

        assert_eq!(by_name["derives-from"].inverse_name, "derived-into");
        assert!(by_name["derives-from"].directed);
        assert!(by_name["derives-from"].acyclic);

        assert_eq!(by_name["satisfies"].inverse_name, "satisfied-by");
        assert!(by_name["satisfies"].directed);
        assert!(!by_name["satisfies"].acyclic);

        assert_eq!(by_name["verifies"].inverse_name, "verified-by");
        assert!(by_name["verifies"].directed);
        assert!(!by_name["verifies"].acyclic);

        assert_eq!(by_name["supersedes"].inverse_name, "superseded-by");
        assert!(by_name["supersedes"].directed);
        assert!(by_name["supersedes"].acyclic);

        assert_eq!(by_name["cites"].inverse_name, "cited-by");
        assert!(by_name["cites"].directed);
        assert!(!by_name["cites"].acyclic);

        assert_eq!(by_name["conflicts-with"].inverse_name, "conflicts-with");
        assert!(!by_name["conflicts-with"].directed);
        assert!(!by_name["conflicts-with"].acyclic);

        assert_eq!(by_name["related-to"].inverse_name, "related-to");
        assert!(!by_name["related-to"].directed);
        assert!(!by_name["related-to"].acyclic);

        assert!(builtins.iter().all(|b| b.source == LinkTypeSource::Builtin));
    }

    #[test]
    fn effective_catalog_is_just_builtins_for_unnamed_system() {
        let catalog = effective_catalog(&LoadedSystem::Unnamed);
        assert_eq!(catalog.len(), 7);
        assert!(catalog.iter().all(|e| e.source == LinkTypeSource::Builtin));
    }

    #[test]
    fn effective_catalog_is_just_builtins_when_system_has_no_link_types() {
        let catalog = effective_catalog(&named_system(Vec::new()));
        assert_eq!(catalog.len(), 7);
        assert!(catalog.iter().all(|e| e.source == LinkTypeSource::Builtin));
    }

    #[test]
    fn effective_catalog_appends_system_declared_types() {
        let catalog = effective_catalog(&named_system(vec![
            system_type("mitigates", "mitigated-by", true, false),
            system_type("informs", "informed-by", true, false),
        ]));
        assert_eq!(catalog.len(), 9);
        let names: Vec<&str> = catalog.iter().map(|e| e.name).collect();
        assert!(names.contains(&"mitigates"));
        assert!(names.contains(&"informs"));

        let mitigates = catalog.iter().find(|e| e.name == "mitigates").unwrap();
        assert_eq!(mitigates.source, LinkTypeSource::System);
        assert_eq!(mitigates.inverse_name, "mitigated-by");
        assert!(mitigates.directed);
        assert!(!mitigates.acyclic);
    }

    #[test]
    fn system_declared_type_cannot_override_a_builtin() {
        // System tries to change `satisfies` into something
        // symmetric — the built-in definition must survive.
        let catalog = effective_catalog(&named_system(vec![system_type(
            "satisfies",
            "junk-inverse",
            false,
            true,
        )]));
        assert_eq!(
            catalog.len(),
            7,
            "colliding system type is dropped, keeping the builtin"
        );
        let satisfies = catalog.iter().find(|e| e.name == "satisfies").unwrap();
        assert_eq!(satisfies.inverse_name, "satisfied-by");
        assert!(satisfies.directed);
        assert_eq!(satisfies.source, LinkTypeSource::Builtin);
    }
}
