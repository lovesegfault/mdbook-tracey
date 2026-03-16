use std::path::PathBuf;

use mdbook_preprocessor::PreprocessorContext;
use serde::Deserialize;

/// `[preprocessor.tracey]` table from `book.toml`.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Path to `.config/tracey/config.styx`, resolved relative to the book
    /// root. `None` means anchor-only mode (no coverage badges).
    pub tracey_config: Option<PathBuf>,
    /// URL template for linking ref popover entries to source, e.g.
    /// `https://github.com/foo/bar/blob/main/{file}#L{line}`. `{file}` and
    /// `{line}` are substituted. When unset, derived from `source_url` in
    /// the tracey config if it looks like a GitHub repo.
    pub repo_url: Option<String>,
    /// Inject the built-in `<style>` block. Default true.
    pub style: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tracey_config: None,
            repo_url: None,
            style: true,
        }
    }
}

impl Config {
    pub fn from_context(ctx: &PreprocessorContext) -> anyhow::Result<Self> {
        let mut cfg: Self = ctx.config.get("preprocessor.tracey")?.unwrap_or_default();
        // tracey_config path is resolved relative to book.toml, not cwd.
        if let Some(p) = &mut cfg.tracey_config {
            *p = ctx.root.join(&*p);
        }
        Ok(cfg)
    }
}
