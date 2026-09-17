# Four transport avenues

`ores-otel` follows the shared transport contract in `ORESoftware/ores-transport` and the fleet data-plane identity in `ORESoftware/k8s-libs-and-shared-defs`.

| Avenue | Owner | Use |
| --- | --- | --- |
| direct read | `ores-otel-web-server` + read-only `ores-lib-core` | latency-sensitive reads only |
| HTTP | `ores-otel-api-server` | default request/response and writes |
| stateful TCP/mTLS | `ores-otel-api-server` | chatty trusted internal request/response |
| JetStream | `ores-otel-api-server` | asynchronous work |

The policy/identity layer is `crate::web_api_plane`. The network serving layer is `crate::transport`; `transport::shared` adapts HTTP envelopes, TCP, and JetStream to one `ores_transport::OperationHandler` implementation. Do not create a second domain handler per protocol.

## Identity

- service slug: `ores-otel`
- environment prefix: `ORES_OTEL_`
- HTTP envelope route: `/v1/operations`
- app host: `app.ores-otel.dev`
- API host: `api.ores-otel.dev`

NATS subject/stream naming is derived from the pinned shared libraries rather than re-authored here. `web_api_plane` validates the fleet data-plane identity; `transport::shared` validates the `ores-transport` subject namespace.

## Safety invariants

Direct database access is a web-server read optimization, not an API-server transport. It must use a read-only role and a read-only library feature so writes require crossing the API boundary.

HTTP, TCP, and JetStream carry the same authenticated operation envelope. Authentication/authorization belongs to each envelope rather than only the lifetime of a TCP connection, because a held-open connection can outlive the credential that created it.

Missing optional transport configuration disables that avenue. Present-but-invalid transport configuration must fail closed; do not invent insecure fallback URLs or silently downgrade TLS.

For asynchronous delivery, publish the result before acknowledging the request so a crash cannot acknowledge work whose result was never emitted. Permanently malformed/expired/unauthorized requests should not be redelivered indefinitely; handler/transient failures may be retried according to the shared transport contract.

## Dependency pins

`ores-transport` is pinned to `c544e1a11a212dc43a5c1f87ea70a95939e80e2e`. Update that revision only after reviewing its API and rerunning the API/web/e2e transport parity tests.
