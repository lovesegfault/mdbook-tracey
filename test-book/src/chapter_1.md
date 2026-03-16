# Observability

rio-build provides three pillars of observability: logs, metrics, and traces.

## Log Lifecycle

r[obs.log.batch-64-100ms]
Log lines are batched (up to 64 lines or 100ms, whichever first) in `BuildLogBatch` messages.

r[obs.log.periodic-flush]
The scheduler flushes buffers to S3 periodically (every 30s) during active builds, not only on completion — bounds log loss to at most 30s on failover.

## Gateway Metrics

r[obs.metric.gateway]

| Metric | Type | Description |
|--------|------|-------------|
| `rio_gateway_connections_total` | Counter | Total SSH connections |
| `rio_gateway_connections_active` | Gauge | Currently active connections |

## Blockquote form

> r[api.error-format]
> API errors must follow this format:
>
> ```json
> {"error": "message", "code": 400}
> ```

## Inline mentions are prose

When implementing r[obs.log.batch-64-100ms] you should consider the batching trade-off.
The inline `r[obs.log.periodic-flush]` reference above is also prose.

## Code fences are examples

```markdown
r[example.only]
This marker is inside a fence — not a definition.
```
