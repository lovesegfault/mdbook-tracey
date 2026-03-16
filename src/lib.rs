//! mdbook preprocessor for [tracey](https://github.com/bearcove/tracey)
//! requirement annotations.
//!
//! Tracey defines requirements in spec markdown with `r[req.id]` markers.
//! mdbook renders those as raw text. This preprocessor turns each marker into
//! a styled anchor block (so you can link to `#r-req.id`), and — when pointed
//! at a `.config/tracey/config.styx` — scans the source tree at preprocess
//! time to decorate each anchor with impl/verify badges. Hover a badge to see
//! where the refs live; click through to GitHub.

mod config;
pub mod coverage;
pub mod marker;
pub mod render;

use std::fs;

use anyhow::{Context, Result, anyhow};
use mdbook_preprocessor::book::{Book, BookItem};
use mdbook_preprocessor::{Preprocessor, PreprocessorContext};

use config::Config;
use coverage::CoverageMap;
use marker::find_markers;
use render::{STYLE, render_marker};

pub struct Tracey;

impl Preprocessor for Tracey {
    fn name(&self) -> &str {
        "tracey"
    }

    fn supports_renderer(&self, renderer: &str) -> Result<bool> {
        Ok(renderer == "html")
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let cfg = Config::from_context(ctx)?;

        // Misconfigured coverage is a hard error — the user asked for
        // badges, and silently falling back to anchor-only would hide the
        // problem until someone noticed the badges were missing.
        let coverage = match &cfg.tracey_config {
            Some(styx_path) => {
                let styx = fs::read_to_string(styx_path)
                    .with_context(|| format!("reading tracey config {}", styx_path.display()))?;
                let tracey_cfg: tracey_config::Config = facet_styx::from_str(&styx)
                    .map_err(|e| anyhow!("{e}"))
                    .with_context(|| format!("parsing {}", styx_path.display()))?;

                // Tracey config lives at .config/tracey/config.styx relative
                // to project root; three ../ gets us there.
                let project_root = styx_path
                    .parent()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .context("tracey config must live at .config/tracey/config.styx")?;

                let repo_url = cfg
                    .repo_url
                    .clone()
                    .or_else(|| derive_repo_url(&tracey_cfg));
                let map = coverage::scan(project_root, &tracey_cfg)?;
                Some((map, repo_url))
            }
            None => None,
        };

        let (cov_map, repo_url) = match &coverage {
            Some((m, u)) => (Some(m), u.as_deref()),
            None => (None, None),
        };

        let mut misses: Vec<String> = Vec::new();
        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item
                && let Some(new) =
                    process_chapter(&ch.content, cov_map, repo_url, cfg.style, &mut misses)
            {
                ch.content = new;
            }
        });

        if !misses.is_empty() {
            misses.sort();
            misses.dedup();
            eprintln!(
                "mdbook-tracey: warning: {} rule(s) not found in coverage scan: {}",
                misses.len(),
                misses.join(", ")
            );
        }

        Ok(book)
    }
}

/// Derive a `{file}`/`{line}` URL template from `SpecConfig.source_url` if
/// it looks like a GitHub repo. Returns `None` otherwise — refs render as
/// plain `<span>` in the popover.
fn derive_repo_url(cfg: &tracey_config::Config) -> Option<String> {
    let source = cfg.specs.iter().find_map(|s| s.source_url.as_deref())?;
    let source = source.trim_end_matches('/');
    if source.starts_with("https://github.com/") {
        // /blob/HEAD/ resolves to the default branch regardless of whether
        // it's called main, master, trunk, etc.
        Some(format!("{source}/blob/HEAD/{{file}}#L{{line}}"))
    } else {
        None
    }
}

/// Rewrite one chapter's markdown. Returns `None` if no markers were found
/// (leaves chapters without tracey annotations byte-identical). When
/// `coverage` is `Some` but a marker's ID is absent from the map, the ID is
/// pushed onto `misses` so the caller can warn.
fn process_chapter(
    content: &str,
    coverage: Option<&CoverageMap>,
    repo_url: Option<&str>,
    inject_style: bool,
    misses: &mut Vec<String>,
) -> Option<String> {
    let markers = find_markers(content);
    if markers.is_empty() {
        return None;
    }

    // Walk the source once, copying unmodified spans between markers and
    // splicing rendered HTML in place of each marker line. Marker spans are
    // non-overlapping and in document order.
    let mut out = String::with_capacity(content.len() + markers.len() * 256);
    if inject_style {
        out.push_str(STYLE);
    }

    let mut cursor = 0;
    for m in &markers {
        out.push_str(&content[cursor..m.line_span.start]);
        let cov = match coverage {
            Some(map) => match map.get(&m.id.base) {
                Some(c) => Some(c),
                None => {
                    misses.push(m.id.base.clone());
                    None
                }
            },
            None => None,
        };
        out.push_str(&render_marker(m, cov, repo_url));
        cursor = m.line_span.end;
    }
    out.push_str(&content[cursor..]);

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coverage::{Coverage, Ref};
    use pretty_assertions::assert_eq;

    #[test]
    fn chapter_without_markers_is_untouched() {
        let md = "# Title\n\nJust prose.\n";
        assert_eq!(process_chapter(md, None, None, true, &mut Vec::new()), None);
    }

    #[test]
    fn marker_replaced_prose_preserved() {
        let md = "# Heading\n\nr[foo.bar]\nThe requirement text.\n\nAnother paragraph.\n";
        let out = process_chapter(md, None, None, false, &mut Vec::new()).unwrap();
        assert!(out.contains(r#"id="r-foo.bar""#));
        assert!(out.contains("The requirement text."));
        assert!(out.contains("Another paragraph."));
        assert!(!out.contains("r[foo.bar]"));
    }

    #[test]
    fn style_injected_when_enabled() {
        let out = process_chapter("r[x.y]\n", None, None, true, &mut Vec::new()).unwrap();
        assert!(out.starts_with("<style>"));
        let out = process_chapter("r[x.y]\n", None, None, false, &mut Vec::new()).unwrap();
        assert!(!out.starts_with("<style>"));
    }

    #[test]
    fn coverage_lookup_by_base() {
        let mut map = CoverageMap::new();
        fn rf(file: &str, line: usize) -> Ref {
            Ref {
                file: file.into(),
                line,
            }
        }
        map.insert(
            "foo.bar".into(),
            Coverage {
                impl_refs: vec![rf("a.rs", 1), rf("b.rs", 2), rf("c.rs", 3)],
                verify_refs: vec![rf("t.rs", 5)],
            },
        );
        // Coverage is keyed by base ID; version suffix in the marker
        // shouldn't defeat the lookup.
        let out =
            process_chapter("r[foo.bar+2]\n", Some(&map), None, false, &mut Vec::new()).unwrap();
        assert!(out.contains("impl 3"));
        assert!(out.contains("verify 1"));
    }

    #[test]
    fn coverage_miss_recorded() {
        let map = CoverageMap::new();
        let mut misses = Vec::new();
        let out = process_chapter("r[not.in.map]\n", Some(&map), None, false, &mut misses).unwrap();
        assert_eq!(misses, ["not.in.map"]);
        assert!(!out.contains("tracey-badge"));
    }

    #[test]
    fn no_miss_without_coverage() {
        let mut misses = Vec::new();
        process_chapter("r[anything]\n", None, None, false, &mut misses).unwrap();
        assert!(misses.is_empty());
    }

    #[test]
    fn derive_repo_url_github() {
        let mut cfg = tracey_config::Config::default();
        cfg.specs.push(tracey_config::SpecConfig {
            name: "rix".into(),
            prefix: None,
            source_url: Some("https://github.com/lovesegfault/rix".into()),
            include: vec![],
            impls: vec![],
        });
        assert_eq!(
            derive_repo_url(&cfg),
            Some("https://github.com/lovesegfault/rix/blob/HEAD/{file}#L{line}".into())
        );
    }

    #[test]
    fn derive_repo_url_trailing_slash() {
        let mut cfg = tracey_config::Config::default();
        cfg.specs.push(tracey_config::SpecConfig {
            name: "x".into(),
            prefix: None,
            source_url: Some("https://github.com/foo/bar/".into()),
            include: vec![],
            impls: vec![],
        });
        assert_eq!(
            derive_repo_url(&cfg),
            Some("https://github.com/foo/bar/blob/HEAD/{file}#L{line}".into())
        );
    }

    #[test]
    fn derive_repo_url_non_github() {
        let mut cfg = tracey_config::Config::default();
        cfg.specs.push(tracey_config::SpecConfig {
            name: "x".into(),
            prefix: None,
            source_url: Some("https://gitlab.com/foo/bar".into()),
            include: vec![],
            impls: vec![],
        });
        assert_eq!(derive_repo_url(&cfg), None);
    }

    #[test]
    fn derive_repo_url_none_when_unset() {
        assert_eq!(derive_repo_url(&tracey_config::Config::default()), None);
    }
}
