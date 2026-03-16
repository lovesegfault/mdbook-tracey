//! In-process coverage scan via `tracey-core`.
//!
//! Previously this module parsed a JSON dump of tracey's `/api/forward`
//! endpoint. Now it drives `WalkSources` directly against the source tree,
//! so coverage is always current — no dump to sync, no curl step in CI.
//!
//! What we lose vs. the dump: `isStale` (computed by tracey's full analysis
//! pipeline, not the raw scan). Stale alarm is out of scope for now.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result, anyhow};
use tracey_core::{ExtractionResult, RefVerb, Sources, WalkSources};

/// One `// r[verb req.id]` reference found in source.
#[derive(Debug, Clone)]
pub struct Ref {
    /// Path relative to project root (what `repo_url` templates expect).
    pub file: String,
    /// 1-indexed line number.
    pub line: usize,
}

/// Implementation and verification references for one rule.
#[derive(Debug, Clone, Default)]
pub struct Coverage {
    pub impl_refs: Vec<Ref>,
    pub verify_refs: Vec<Ref>,
}

/// `rule_id.base → refs`
pub type CoverageMap = HashMap<String, Coverage>;

/// Scan `project_root` for requirement references using the include/exclude
/// globs from each `Impl` block in the tracey config. Groups by
/// `req_id.base` — version suffixes in source annotations don't split
/// coverage across buckets.
pub fn scan(project_root: &Path, cfg: &tracey_config::Config) -> Result<CoverageMap> {
    let mut map: CoverageMap = HashMap::new();

    for spec in &cfg.specs {
        for imp in &spec.impls {
            let includes: Vec<String> = imp
                .include
                .iter()
                .chain(imp.test_include.iter())
                .cloned()
                .collect();

            let result = WalkSources::new(project_root)
                .include(includes)
                .exclude(imp.exclude.clone())
                .extract()
                .map_err(|e| anyhow!("{e:?}"))
                .with_context(|| format!("scanning impl '{}'", imp.name))?;

            collect_refs(&mut map, result, project_root);
        }
    }

    Ok(map)
}

/// Fold an extraction result into the coverage map. Pulled out so tests can
/// drive it with `MemorySources` instead of walking a real tree.
fn collect_refs(map: &mut CoverageMap, result: ExtractionResult, project_root: &Path) {
    for r in result.reqs.references {
        let slot = match r.verb {
            RefVerb::Impl => &mut map.entry(r.req_id.base).or_default().impl_refs,
            RefVerb::Verify => &mut map.entry(r.req_id.base).or_default().verify_refs,
            // Define/Depends/Related — not badge-worthy.
            _ => continue,
        };
        let file = r
            .file
            .strip_prefix(project_root)
            .unwrap_or(&r.file)
            .to_string_lossy()
            .into_owned();
        slot.push(Ref { file, line: r.line });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracey_core::MemorySources;

    fn extract(sources: MemorySources) -> CoverageMap {
        let mut map = CoverageMap::new();
        let result = sources.extract().unwrap();
        collect_refs(&mut map, result, Path::new(""));
        map
    }

    #[test]
    fn groups_by_base_across_files() {
        let map = extract(
            MemorySources::new()
                .add("crates/a/src/foo.rs", "// r[impl obs.log.batch]")
                .add("crates/b/src/bar.rs", "// r[impl obs.log.batch]")
                .add("crates/a/tests/t.rs", "// r[verify obs.log.batch]"),
        );
        let c = map.get("obs.log.batch").unwrap();
        assert_eq!(c.impl_refs.len(), 2);
        assert_eq!(c.verify_refs.len(), 1);
        assert_eq!(c.verify_refs[0].file, "crates/a/tests/t.rs");
    }

    #[test]
    fn version_suffix_folds_into_base() {
        let map = extract(
            MemorySources::new()
                .add("a.rs", "// r[impl foo.bar]")
                .add("b.rs", "// r[impl foo.bar+2]"),
        );
        assert_eq!(map.get("foo.bar").unwrap().impl_refs.len(), 2);
    }

    #[test]
    fn ignores_non_badge_verbs() {
        let map = extract(
            MemorySources::new()
                .add("a.rs", "// r[depends foo.bar]")
                .add("b.rs", "// r[related foo.bar]"),
        );
        assert!(map.is_empty());
    }

    #[test]
    fn preserves_line_numbers() {
        let map = extract(
            MemorySources::new().add("src/sched.rs", "fn f() {}\n\n// r[impl obs.log.flush]\n"),
        );
        let c = map.get("obs.log.flush").unwrap();
        assert_eq!(c.impl_refs[0].line, 3);
    }

    #[test]
    fn strips_project_root() {
        let mut map = CoverageMap::new();
        let result = MemorySources::new()
            .add("/project/crates/foo/src/lib.rs", "// r[impl x.y]")
            .extract()
            .unwrap();
        collect_refs(&mut map, result, Path::new("/project"));
        assert_eq!(
            map.get("x.y").unwrap().impl_refs[0].file,
            "crates/foo/src/lib.rs"
        );
    }
}
