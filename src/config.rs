use std::path::PathBuf;

use mdbook_preprocessor::PreprocessorContext;
use serde::Deserialize;

/// `[preprocessor.tracey]` table from `book.toml`.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Path to a tracey `/api/forward` JSON dump, resolved relative to the
    /// book root. `None` means anchor-only mode (no coverage badges).
    pub coverage: Option<PathBuf>,
    /// Inject the built-in `<style>` block. Default true.
    pub style: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            coverage: None,
            style: true,
        }
    }
}

impl Config {
    pub fn from_context(ctx: &PreprocessorContext) -> anyhow::Result<Self> {
        let mut cfg: Self = ctx.config.get("preprocessor.tracey")?.unwrap_or_default();
        // Coverage path is resolved relative to book.toml, not cwd.
        if let Some(p) = &mut cfg.coverage {
            *p = ctx.root.join(&*p);
        }
        Ok(cfg)
    }
}
