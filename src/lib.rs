//! mdbook preprocessor for [tracey](https://github.com/bearcove/tracey)
//! requirement annotations.
//!
//! Tracey defines requirements in spec markdown with `r[req.id]` markers.
//! mdbook renders those as raw text. This preprocessor turns each marker into
//! a styled anchor block (so you can link to `#r-req.id`), and optionally
//! decorates it with impl/verify coverage badges when pointed at a dump of
//! tracey's `/api/forward` endpoint.

mod config;
pub mod coverage;
pub mod marker;
pub mod render;

use anyhow::Result;
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
        let coverage = cfg.coverage.as_deref().map(coverage::load).transpose()?;

        let mut misses: Vec<String> = Vec::new();
        book.for_each_mut(|item| {
            if let BookItem::Chapter(ch) = item
                && let Some(new) =
                    process_chapter(&ch.content, coverage.as_ref(), cfg.style, &mut misses)
            {
                ch.content = new;
            }
        });

        if !misses.is_empty() {
            misses.sort();
            misses.dedup();
            eprintln!(
                "mdbook-tracey: warning: {} rule(s) not found in coverage dump: {}",
                misses.len(),
                misses.join(", ")
            );
        }

        Ok(book)
    }
}

/// Rewrite one chapter's markdown. Returns `None` if no markers were found
/// (leaves chapters without tracey annotations byte-identical). When
/// `coverage` is `Some` but a marker's ID is absent from the map, the ID is
/// pushed onto `misses` so the caller can warn.
fn process_chapter(
    content: &str,
    coverage: Option<&CoverageMap>,
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
                Some(c) => Some(*c),
                None => {
                    misses.push(m.id.base.clone());
                    None
                }
            },
            None => None,
        };
        out.push_str(&render_marker(m, cov));
        cursor = m.line_span.end;
    }
    out.push_str(&content[cursor..]);

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn chapter_without_markers_is_untouched() {
        let md = "# Title\n\nJust prose.\n";
        assert_eq!(process_chapter(md, None, true, &mut Vec::new()), None);
    }

    #[test]
    fn marker_replaced_prose_preserved() {
        let md = "# Heading\n\nr[foo.bar]\nThe requirement text.\n\nAnother paragraph.\n";
        let out = process_chapter(md, None, false, &mut Vec::new()).unwrap();
        assert!(out.contains(r#"id="r-foo.bar""#));
        assert!(out.contains("The requirement text."));
        assert!(out.contains("Another paragraph."));
        assert!(!out.contains("r[foo.bar]"));
    }

    #[test]
    fn style_injected_when_enabled() {
        let out = process_chapter("r[x.y]\n", None, true, &mut Vec::new()).unwrap();
        assert!(out.starts_with("<style>"));
        let out = process_chapter("r[x.y]\n", None, false, &mut Vec::new()).unwrap();
        assert!(!out.starts_with("<style>"));
    }

    #[test]
    fn coverage_lookup_by_base() {
        let mut map = CoverageMap::new();
        map.insert(
            "foo.bar".into(),
            coverage::Coverage {
                impl_count: 3,
                verify_count: 1,
            },
        );
        // Coverage is keyed by base ID; version suffix in the marker
        // shouldn't defeat the lookup.
        let out = process_chapter("r[foo.bar+2]\n", Some(&map), false, &mut Vec::new()).unwrap();
        assert!(out.contains("impl 3"));
        assert!(out.contains("verify 1"));
    }

    #[test]
    fn coverage_miss_recorded() {
        let map = CoverageMap::new();
        let mut misses = Vec::new();
        let out = process_chapter("r[not.in.map]\n", Some(&map), false, &mut misses).unwrap();
        assert_eq!(misses, ["not.in.map"]);
        assert!(!out.contains("tracey-badge"));
    }

    #[test]
    fn no_miss_without_coverage() {
        let mut misses = Vec::new();
        process_chapter("r[anything]\n", None, false, &mut misses).unwrap();
        assert!(misses.is_empty());
    }
}
