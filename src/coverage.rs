//! Loader for tracey's `/api/forward` JSON dump.
//!
//! Serde mirror structs cover only the fields we read. Depending on
//! `tracey-api` directly would pull in `tracey-core` → `marq[all-handlers]`
//! → arborium (the full tree-sitter grammar set) via tracey's workspace
//! feature pins — the exact transitive tree we avoided elsewhere.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Default)]
pub struct Coverage {
    pub impl_count: usize,
    pub verify_count: usize,
}

/// `rule_id.base → (impl_count, verify_count)`
pub type CoverageMap = HashMap<String, Coverage>;

pub fn load(path: &Path) -> Result<CoverageMap> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("reading coverage file {}", path.display()))?;
    parse(&json).with_context(|| format!("parsing coverage file {}", path.display()))
}

pub fn parse(json: &str) -> Result<CoverageMap> {
    let data: ForwardData = serde_json::from_str(json)?;
    let mut map = HashMap::new();
    for spec in data.specs {
        for rule in spec.rules {
            // Same base ID across two specs → last-write-wins. In practice
            // tracey specs use disjoint prefixes so this shouldn't collide.
            map.insert(
                rule.id.base,
                Coverage {
                    impl_count: rule.impl_refs.len(),
                    verify_count: rule.verify_refs.len(),
                },
            );
        }
    }
    Ok(map)
}

// --- wire-format mirrors ---------------------------------------------------
// See tracey-api/src/lib.rs: ApiForwardData / ApiSpecForward / ApiRule.
// Only ApiRule carries #[facet(rename_all = "camelCase")].

#[derive(Deserialize)]
struct ForwardData {
    specs: Vec<SpecForward>,
}

#[derive(Deserialize)]
struct SpecForward {
    rules: Vec<Rule>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Rule {
    id: RuleIdJson,
    #[serde(default)]
    impl_refs: Vec<serde_json::Value>,
    #[serde(default)]
    verify_refs: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct RuleIdJson {
    base: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_forward_dump() {
        let json = r#"{
            "specs": [{
                "name": "rix",
                "rules": [
                    {
                        "id": {"base": "obs.log.batch", "version": 1},
                        "raw": "...",
                        "html": "...",
                        "implRefs": [{"file": "a.rs", "line": 1}, {"file": "b.rs", "line": 2}],
                        "verifyRefs": [{"file": "t.rs", "line": 5}],
                        "dependsRefs": [],
                        "isStale": false,
                        "staleRefs": []
                    },
                    {
                        "id": {"base": "obs.log.flush", "version": 2},
                        "raw": "...",
                        "html": "...",
                        "implRefs": [{"file": "c.rs", "line": 3}],
                        "verifyRefs": [],
                        "dependsRefs": [],
                        "isStale": false,
                        "staleRefs": []
                    }
                ]
            }]
        }"#;
        let map = parse(json).unwrap();
        assert_eq!(map.len(), 2);
        let a = map.get("obs.log.batch").unwrap();
        assert_eq!(a.impl_count, 2);
        assert_eq!(a.verify_count, 1);
        let b = map.get("obs.log.flush").unwrap();
        assert_eq!(b.impl_count, 1);
        assert_eq!(b.verify_count, 0);
    }

    #[test]
    fn tolerates_missing_ref_arrays() {
        // serde default → empty vec → count 0
        let json = r#"{"specs":[{"rules":[{"id":{"base":"x.y","version":1}}]}]}"#;
        let map = parse(json).unwrap();
        let c = map.get("x.y").unwrap();
        assert_eq!(c.impl_count, 0);
        assert_eq!(c.verify_count, 0);
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse("not json").is_err());
        assert!(parse("{}").is_err()); // missing `specs`
    }
}
