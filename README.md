# mdbook-tracey

An [mdbook](https://rust-lang.github.io/mdBook/) preprocessor for
[tracey](https://github.com/bearcove/tracey) requirement annotations.

Tracey defines requirements in spec markdown with `r[req.id]` markers and
links them to `// r[impl req.id]` annotations in source code. Without this
preprocessor, mdbook renders those markers as raw text. With it, each marker
becomes a styled anchor you can link to — and if you point it at your
`.config/tracey/config.styx`, it scans the source tree at build time and
decorates each anchor with impl/verify badges. Hover a badge to see every
reference; click through to GitHub.

## Install

```sh
cargo install --git https://github.com/lovesegfault/mdbook-tracey
```

Or with Nix:

```sh
nix run github:lovesegfault/mdbook-tracey
```

## Usage

Add to your `book.toml`:

```toml
[preprocessor.tracey]
```

Then build as usual:

```sh
mdbook build
```

Markers like this in your chapter markdown:

```markdown
r[obs.log.batch-64-100ms]
Log lines are batched (up to 64 lines or 100ms, whichever first).
```

render as a styled badge with an anchor at `#r-obs.log.batch-64-100ms`.

## Coverage badges

Point the preprocessor at your tracey config and it runs the source scan at
build time. Each requirement badge shows how many `impl` and `verify`
references tracey found; hovering a badge lists every `file:line`, linked to
your repo.

```toml
[preprocessor.tracey]
tracey_config = "../.config/tracey/config.styx"
```

The path is resolved relative to `book.toml`. The scan reuses the
`include`/`exclude` globs from each `impls` block in the tracey config — no
duplication in `book.toml`. A missing or malformed config is a build error.

Repo links in the popover are derived from `source_url` in the tracey config
when it looks like a GitHub repo (`.../blob/main/{file}#L{line}`). Override
with an explicit template:

```toml
[preprocessor.tracey]
tracey_config = "../.config/tracey/config.styx"
repo_url = "https://github.com/foo/bar/blob/trunk/{file}#L{line}"
```

## Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `tracey_config` | string | *(none)* | Path to `.config/tracey/config.styx`, relative to `book.toml`. Omit for anchor-only mode. |
| `repo_url` | string | *(derived)* | URL template for ref links. `{file}` and `{line}` are substituted. Derived from `source_url` if unset. |
| `style` | bool | `true` | Inject the built-in `<style>` block. Set to `false` if you ship your own CSS. |

## What counts as a marker

Following tracey's spec: a marker is a definition only when it opens a
paragraph at column 0, or opens a blockquote line (`> r[...]`). Inline
mentions, table cells, and anything inside code fences or backtick spans are
prose and are left alone.

```markdown
r[foo.bar]           ← definition (column 0)
> r[foo.bar]         ← definition (blockquote)
  r[foo.bar]         ← prose (indented)
See r[foo.bar] for…  ← prose (inline)
`r[foo.bar]`         ← prose (code span)
```

## License

BSD-3-Clause
