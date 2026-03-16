//! Integration tests: feed fixture markdown through the marker/render
//! pipeline and snapshot the output.

use std::collections::HashMap;

use mdbook_tracey::coverage::{Coverage, Ref};
use mdbook_tracey::marker::find_markers;
use mdbook_tracey::render::render_marker;

/// End-to-end chapter transform (same splice as `lib.rs::process_chapter`,
/// minus the style-inject toggle which is tested separately).
fn transform(
    content: &str,
    cov: Option<&HashMap<String, Coverage>>,
    repo_url: Option<&str>,
) -> String {
    let markers = find_markers(content);
    let mut out = String::new();
    let mut cursor = 0;
    for m in &markers {
        out.push_str(&content[cursor..m.line_span.start]);
        let c = cov.and_then(|map| map.get(&m.id.base));
        out.push_str(&render_marker(m, c, repo_url));
        cursor = m.line_span.end;
    }
    out.push_str(&content[cursor..]);
    out
}

#[test]
fn standalone_anchor_only() {
    let md = include_str!("fixtures/standalone.md");
    insta::assert_snapshot!(transform(md, None, None));
}

#[test]
fn standalone_with_coverage() {
    let md = include_str!("fixtures/standalone.md");
    let cov = fixture_coverage();
    insta::assert_snapshot!(transform(
        md,
        Some(&cov),
        Some("https://github.com/x/y/blob/main/{file}#L{line}")
    ));
}

#[test]
fn blockquote_form() {
    let md = include_str!("fixtures/blockquote.md");
    insta::assert_snapshot!(transform(md, None, None));
}

#[test]
fn code_fences_left_alone() {
    let md = include_str!("fixtures/code-fence.md");
    let out = transform(md, None, None);
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
    let out = transform(md, None, None);
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
    let out = transform(md, None, None);
    // Only r[real.marker] gets an anchor; the <pre> example and the
    // <!-- --> commented marker survive as literal text.
    assert_eq!(out.matches(r#"class="tracey-req""#).count(), 1);
    assert!(out.contains("r[section.rule-name]"));
    assert!(out.contains("r[commented.out]"));
    assert!(out.contains(r#"id="r-real.marker""#));
    insta::assert_snapshot!(out);
}

/// Hand-built coverage map mirroring what a scan over the old
/// `tests/fixtures/forward.json` would have produced.
fn fixture_coverage() -> HashMap<String, Coverage> {
    fn rf(file: &str, line: usize) -> Ref {
        Ref {
            file: file.into(),
            line,
        }
    }
    let mut m = HashMap::new();
    m.insert(
        "obs.log.batch-64-100ms".into(),
        Coverage {
            impl_refs: vec![rf("src/scheduler.rs", 42), rf("src/worker.rs", 128)],
            verify_refs: vec![rf("tests/log_test.rs", 15)],
        },
    );
    m.insert(
        "obs.log.periodic-flush".into(),
        Coverage {
            impl_refs: vec![rf("src/scheduler.rs", 89)],
            verify_refs: vec![],
        },
    );
    m.insert("obs.metric.gateway".into(), Coverage::default());
    m
}
