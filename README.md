# mdbook-tracey

An [mdbook](https://rust-lang.github.io/mdBook/) preprocessor for
[tracey](https://github.com/bearcove/tracey) requirement annotations.

Tracey defines requirements in spec markdown with `r[req.id]` markers and
links them to `// r[impl req.id]` annotations in source code. Without this
preprocessor, mdbook renders those markers as raw text. With it, each marker
becomes a styled anchor you can link to — and if you point it at a dump of
tracey's coverage data, each anchor also shows impl/verify badges.

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

Optionally point the preprocessor at a JSON dump of tracey's `/api/forward`
endpoint and each requirement badge will show how many `impl` and `verify`
references tracey found for it:

```toml
[preprocessor.tracey]
coverage = "tracey-forward.json"
```

To produce the dump, run `tracey web` and fetch the forward-traceability API:

```sh
curl -s http://localhost:8000/api/forward > docs/tracey-forward.json
```

The path is resolved relative to `book.toml`. A missing or malformed
coverage file is a build error — if you've configured a path, silently
falling back to anchor-only mode would just hide the misconfiguration.

## Configuration

| Key | Type | Default | Description |
|---|---|---|---|
| `coverage` | string | *(none)* | Path to an `/api/forward` JSON dump, relative to `book.toml`. Omit for anchor-only mode. |
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
