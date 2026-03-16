//! HTML generation for a single marker and the injected stylesheet.

use std::fmt::Write;

use crate::coverage::Coverage;
use crate::marker::Marker;

/// Built-in styles, prepended once to the first chapter that has markers.
/// Kept on one line so mdbook's markdown parser sees it as a single raw-HTML
/// block; a trailing blank line cleanly separates it from chapter content.
pub const STYLE: &str = concat!(
    "<style>",
    ".tracey-req{",
    "display:flex;align-items:center;gap:.6em;",
    "margin:1.2em 0 .4em 0;padding:.35em .6em;",
    "border-left:3px solid var(--links,#4183c4);",
    "background:var(--quote-bg,rgba(0,0,0,.03));",
    "border-radius:0 4px 4px 0;",
    "font-family:var(--mono-font,ui-monospace,SFMono-Regular,Menlo,monospace);",
    "font-size:.9em",
    "}",
    ".tracey-req-anchor{",
    "color:var(--links,#4183c4);text-decoration:none;font-weight:600",
    "}",
    ".tracey-req-anchor:hover{text-decoration:underline}",
    ".tracey-req-badges{margin-left:auto;display:flex;gap:.4em}",
    ".tracey-badge{",
    "padding:.1em .5em;border-radius:3px;font-size:.85em;font-weight:600",
    "}",
    ".tracey-badge.impl{background:#2ea04326;color:#2ea043}",
    ".tracey-badge.verify{background:#8250df26;color:#8250df}",
    ".tracey-badge.zero{background:#6e77811a;color:#6e7781}",
    "</style>\n\n",
);

/// Render one marker to an HTML block. Always emits an anchor; emits badges
/// only when coverage data was loaded (so anchor-only mode stays clean).
///
/// The trailing blank line is load-bearing: mdbook re-parses chapter content
/// with pulldown-cmark, and raw HTML must be followed by a blank line to
/// close the HTML block before markdown resumes. Note that a `<div>` at
/// column 0 is a type-6 HTML block — it interrupts paragraphs and
/// blockquotes per CommonMark, so a marker on line 2 of a blockquote pops
/// the anchor *out* of the quote (the snapshot tests accept this).
pub fn render_marker(m: &Marker, cov: Option<Coverage>) -> String {
    // marq uses `{prefix}-{id}` for anchor IDs (render.rs:1225); we match.
    let anchor = html_escape(&format!("{}-{}", m.prefix, m.id));
    let label = html_escape(&m.id.to_string());

    let mut s = String::with_capacity(256);
    write!(
        s,
        r##"<div class="tracey-req" id="{anchor}"><a class="tracey-req-anchor" href="#{anchor}">{label}</a>"##
    )
    .unwrap();

    if let Some(c) = cov {
        s.push_str(r#"<span class="tracey-req-badges">"#);
        badge(&mut s, "impl", c.impl_count);
        badge(&mut s, "verify", c.verify_count);
        s.push_str("</span>");
    }

    s.push_str("</div>\n\n");
    s
}

fn badge(s: &mut String, kind: &str, count: usize) {
    let zero = if count == 0 { " zero" } else { "" };
    write!(
        s,
        r#"<span class="tracey-badge {kind}{zero}">{kind} {count}</span>"#
    )
    .unwrap();
}

/// marq's `parse_rule_id` is permissive — it only rejects empty strings and
/// bad `+N` suffixes, so the charset is whatever the spec author typed.
/// In practice that's `[a-zA-Z0-9._-]`, but we escape defensively.
fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marker::find_markers;

    fn first(md: &str) -> Marker {
        find_markers(md).into_iter().next().unwrap()
    }

    #[test]
    fn anchor_only() {
        let m = first("r[obs.log.batch]\n");
        let html = render_marker(&m, None);
        assert!(html.contains(r#"id="r-obs.log.batch""#));
        assert!(html.contains(r##"href="#r-obs.log.batch""##));
        assert!(!html.contains("tracey-badge"));
        assert!(html.ends_with("\n\n"));
    }

    #[test]
    fn with_coverage() {
        let m = first("r[obs.log.batch]\n");
        let html = render_marker(
            &m,
            Some(Coverage {
                impl_count: 2,
                verify_count: 1,
            }),
        );
        assert!(html.contains(">impl 2<"));
        assert!(html.contains(">verify 1<"));
        assert!(!html.contains("zero"));
    }

    #[test]
    fn zero_count_styling() {
        let m = first("r[obs.log.batch]\n");
        let html = render_marker(
            &m,
            Some(Coverage {
                impl_count: 1,
                verify_count: 0,
            }),
        );
        assert!(html.contains(r#"class="tracey-badge verify zero""#));
        assert!(!html.contains(r#"class="tracey-badge impl zero""#));
    }

    #[test]
    fn version_in_label_and_anchor() {
        let m = first("r[auth.login+3]\n");
        let html = render_marker(&m, None);
        // Display impl for RuleId includes +N when version > 1.
        assert!(html.contains(">auth.login+3<"));
        assert!(html.contains(r#"id="r-auth.login+3""#));
    }
}
