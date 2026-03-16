# Security

r[sec.drv.validate]
Derivations are validated before scheduling.

Additional validation checks (below) are enforced at other points.
These are **not** covered by `r[sec.drv.validate]` itself.

| Check | Component | Requirement |
|-------|-----------|-------------|
| NAR SHA-256 | Store | `r[sec.drv.validate]` |

When implementing r[sec.drv.validate] you should also consider the
threat model described above.
