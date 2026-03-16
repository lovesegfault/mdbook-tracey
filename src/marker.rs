//! Requirement-marker detection in chapter markdown.
//!
//! A marker is `PREFIX[ID]` where PREFIX is `[a-z0-9]+`. Tracey recognizes a
//! marker as a definition only when it opens a paragraph at column 0 or opens
//! a blockquote line; inline occurrences and anything inside code
//! blocks/spans are prose.
//!
//! Detection here is line-based (we need to replace whole lines), with a
//! pulldown-cmark pass up front to mask out code regions.

use marq::{RuleId, parse_rule_id};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// A requirement marker found in chapter markdown.
#[derive(Debug, Clone)]
pub struct Marker {
    /// Byte range of the full line this marker occupies (including the
    /// trailing `\n` if present). Splice-replace against this.
    pub line_span: std::ops::Range<usize>,
    /// The prefix as written (e.g. `r`, `req`, `h2`).
    pub prefix: String,
    pub id: RuleId,
    /// The marker was on a `> ` blockquote line rather than at column 0.
    pub blockquote: bool,
}

/// Find all requirement-definition markers in a chapter's markdown.
pub fn find_markers(content: &str) -> Vec<Marker> {
    let code_mask = code_mask(content);
    let mut out = Vec::new();

    let mut offset = 0;
    for line in content.split_inclusive('\n') {
        let line_start = offset;
        offset += line.len();

        // Skip if this line starts inside a code block or span.
        if *code_mask.get(line_start).unwrap_or(&false) {
            continue;
        }

        let trimmed = line.trim_end_matches(['\n', '\r']);

        // Blockquote: strip a single leading `>` and one optional space.
        // No leading whitespace before `>` — we only recognize the
        // top-level blockquote form.
        let (body, blockquote) = if let Some(rest) = trimmed.strip_prefix('>') {
            (rest.strip_prefix(' ').unwrap_or(rest), true)
        } else {
            (trimmed, false)
        };

        let Some((prefix, inner)) = parse_leading_marker(body) else {
            continue;
        };

        // Inner content may carry attributes (`foo.bar status=draft`);
        // split on the first space and hand just the ID to marq.
        // On parse failure we leave the line alone — better to render the
        // raw marker than to silently drop a malformed spec line.
        let id_part = inner.split_once(' ').map(|(id, _)| id).unwrap_or(inner);
        let Some(id) = parse_rule_id(id_part) else {
            continue;
        };

        out.push(Marker {
            line_span: line_start..offset,
            prefix: prefix.to_owned(),
            id,
            blockquote,
        });
    }

    out
}

/// Recognize `PREFIX[CONTENT]` at the start of `text` where the closing `]`
/// ends the line (trailing whitespace tolerated). Returns
/// `(prefix, inner)` — `inner` is everything between the brackets.
///
/// The leading-marker split lives in marq (`render.rs::parse_req_leading_marker`)
/// but is private; this mirrors its prefix scan and adds the whole-line
/// constraint tracey uses for the standalone form.
fn parse_leading_marker(text: &str) -> Option<(&str, &str)> {
    let prefix_len = text
        .bytes()
        .take_while(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        .count();
    if prefix_len == 0 || text.as_bytes().get(prefix_len) != Some(&b'[') {
        return None;
    }
    let close = text.find(']')?;
    if close <= prefix_len + 1 {
        return None; // empty brackets
    }
    // Anything non-whitespace after `]` → inline mention, not a definition.
    if !text[close + 1..].trim().is_empty() {
        return None;
    }
    Some((&text[..prefix_len], &text[prefix_len + 1..close]))
}

/// Build a per-byte mask: `true` where the byte falls inside a fenced or
/// indented code block, or an inline backtick span. Markers starting at a
/// masked byte are examples, not definitions.
fn code_mask(content: &str) -> Vec<bool> {
    let mut mask = vec![false; content.len()];
    let mut depth = 0usize;

    let parser = Parser::new_ext(content, Options::all()).into_offset_iter();
    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                depth += 1;
                fill(&mut mask, range);
            }
            Event::End(TagEnd::CodeBlock) => {
                depth = depth.saturating_sub(1);
                fill(&mut mask, range);
            }
            Event::Code(_) => fill(&mut mask, range),
            _ if depth > 0 => fill(&mut mask, range),
            _ => {}
        }
    }
    mask
}

fn fill(mask: &mut [bool], range: std::ops::Range<usize>) {
    if let Some(slice) = mask.get_mut(range) {
        slice.fill(true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn standalone_column_zero() {
        let md = "r[obs.log.batch]\nLog lines are batched...\n";
        let markers = find_markers(md);
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].prefix, "r");
        assert_eq!(markers[0].id.base, "obs.log.batch");
        assert_eq!(markers[0].id.version, 1);
        assert!(!markers[0].blockquote);
        // Span covers the marker line including its newline; replacing it
        // leaves the prose line as the start of the next paragraph.
        assert_eq!(&md[markers[0].line_span.clone()], "r[obs.log.batch]\n");
    }

    #[test]
    fn indented_is_not_a_definition() {
        assert!(find_markers("  r[obs.log.batch]\n").is_empty());
    }

    #[test]
    fn blockquote_form() {
        let markers = find_markers("> r[api.error-format]\n> API errors...\n");
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].id.base, "api.error-format");
        assert!(markers[0].blockquote);
    }

    #[test]
    fn blockquote_no_space_after_gt() {
        let markers = find_markers(">r[x.y]\n");
        assert_eq!(markers.len(), 1);
        assert!(markers[0].blockquote);
    }

    #[test]
    fn inline_mention_is_ignored() {
        assert!(find_markers("When implementing r[auth.login] you should...\n").is_empty());
    }

    #[test]
    fn trailing_text_after_bracket_is_ignored() {
        // The closing `]` must end the line (modulo whitespace).
        assert!(find_markers("r[foo.bar] extra\n").is_empty());
    }

    #[test]
    fn inside_fenced_code_is_ignored() {
        let md = "```markdown\nr[foo.bar]\n```\n";
        assert!(find_markers(md).is_empty());
    }

    #[test]
    fn inside_inline_code_is_ignored() {
        // Even though the backticked text starts at col 0, pulldown-cmark
        // marks those bytes as inline code.
        assert!(find_markers("`r[foo.bar]`\n").is_empty());
    }

    #[test]
    fn version_suffix() {
        let markers = find_markers("r[auth.login+3]\n");
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].id.base, "auth.login");
        assert_eq!(markers[0].id.version, 3);
    }

    #[test]
    fn attributes_dont_break_id_parse() {
        // Attributes after the ID (`status=draft` etc.) are valid tracey
        // syntax. We don't render them, but they mustn't prevent detection.
        let markers = find_markers("r[foo.bar status=draft level=must]\n");
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].id.base, "foo.bar");
    }

    #[test]
    fn alternate_prefix() {
        let markers = find_markers("h2[stream.window]\n");
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].prefix, "h2");
    }

    #[test]
    fn malformed_version_left_alone() {
        // marq::parse_rule_id is permissive about segment structure (it
        // accepts `foo..bar`, `nodot`, etc. — the "at least one dot" rule
        // is tracey-spec, not marq) but it does reject bad version
        // suffixes. Those lines render as-is.
        assert!(find_markers("r[foo.bar+]\n").is_empty());
        assert!(find_markers("r[foo.bar+0]\n").is_empty());
        assert!(find_markers("r[foo.bar+1+2]\n").is_empty());
    }

    #[test]
    fn multiple_markers() {
        let md = "\
# Heading\n\
\n\
r[first.req]\n\
Text.\n\
\n\
r[second.req]\n\
More text.\n\
";
        let markers = find_markers(md);
        assert_eq!(markers.len(), 2);
        assert_eq!(markers[0].id.base, "first.req");
        assert_eq!(markers[1].id.base, "second.req");
    }

    #[test]
    fn final_line_without_newline() {
        let markers = find_markers("r[end.of.file]");
        assert_eq!(markers.len(), 1);
        assert_eq!(markers[0].line_span, 0..14);
    }
}
