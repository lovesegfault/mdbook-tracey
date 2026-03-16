//! Integration tests: feed fixture markdown through the marker/render
//! pipeline and snapshot the output.

use std::collections::HashMap;

use mdbook_tracey::coverage::{self, Coverage};
use mdbook_tracey::marker::find_markers;
use mdbook_tracey::render::render_marker;

/// End-to-end chapter transform (same splice as `lib.rs::process_chapter`,
/// minus the style-inject toggle which is tested separately).
fn transform(content: &str, cov: Option<&HashMap<String, Coverage>>) -> String {
    let markers = find_markers(content);
    let mut out = String::new();
    let mut cursor = 0;
    for m in &markers {
        out.push_str(&content[cursor..m.line_span.start]);
        let c = cov.and_then(|map| map.get(&m.id.base)).copied();
        out.push_str(&render_marker(m, c));
        cursor = m.line_span.end;
    }
    out.push_str(&content[cursor..]);
    out
}

#[test]
fn standalone_anchor_only() {
    let md = include_str!("fixtures/standalone.md");
    insta::assert_snapshot!(transform(md, None));
}

#[test]
fn standalone_with_coverage() {
    let md = include_str!("fixtures/standalone.md");
    let json = include_str!("fixtures/forward.json");
    let cov = coverage::parse(json).unwrap();
    insta::assert_snapshot!(transform(md, Some(&cov)));
}

#[test]
fn blockquote_form() {
    let md = include_str!("fixtures/blockquote.md");
    insta::assert_snapshot!(transform(md, None));
}

#[test]
fn code_fences_left_alone() {
    let md = include_str!("fixtures/code-fence.md");
    let out = transform(md, None);
    // Only one real marker (r[real.requirement]); the fenced and inline
    // ones survive as literal text.
    assert_eq!(out.matches(r#"class="tracey-req""#).count(), 1);
    assert!(out.contains("r[example.one]"));
    assert!(out.contains("`r[example.two]`"));
    insta::assert_snapshot!(out);
}

#[test]
fn inline_mentions_survive() {
    let md = include_str!("fixtures/inline.md");
    let out = transform(md, None);
    // Exactly one definition (the one at column 0); the table cell,
    // inline-code, and mid-sentence mentions are prose.
    assert_eq!(out.matches(r#"id="r-sec.drv.validate""#).count(), 1);
    // Prose mentions are still there verbatim.
    assert!(out.contains("`r[sec.drv.validate]`"));
    assert!(out.contains("When implementing r[sec.drv.validate]"));
    insta::assert_snapshot!(out);
}

#[test]
fn html_blocks_left_alone() {
    let md = include_str!("fixtures/html-block.md");
    let out = transform(md, None);
    // Only r[real.marker] gets an anchor; the <pre> example and the
    // <!-- --> commented marker survive as literal text.
    assert_eq!(out.matches(r#"class="tracey-req""#).count(), 1);
    assert!(out.contains("r[section.rule-name]"));
    assert!(out.contains("r[commented.out]"));
    assert!(out.contains(r#"id="r-real.marker""#));
    insta::assert_snapshot!(out);
}
