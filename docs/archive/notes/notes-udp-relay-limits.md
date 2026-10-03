# UDP relay limits

## Findings and scope

The production UDP relay is request/response traffic carried by the authenticated
TCP mux, not the experimental physical UDP underlay. Every mux stream consumes
both the configured per-session `max_streams` permit and the server-wide stream
permit. UDP requests therefore share the same concurrency ceiling as TCP; the
permit is held for the full datagram relay operation.

The UDP payload length uses a 16-bit wire field, and the writer rejects payloads
larger than `u16::MAX`. On the server, each UDP request payload and response
payload is charged to the same user's rolling quota and aggregate bandwidth
limiter. The response receive buffer is bounded to one maximum UDP-sized buffer.
These controls fit the existing small-operations model and need no new config
surface or protocol changes.

The references guide identifies `shadowsocks-rust` as a useful implementation
reference for async UDP relay patterns. This pass keeps the existing single
datagram request/response model and adds regression coverage at its wire-size
and aggregate-quota boundaries; it does not introduce a UDP physical transport.

## Changes and expected effect

- Reject and test oversized UDP payloads before writing any request bytes.
- Test that request and response bytes both consume the same user's quota.
- No runtime throughput change is expected. The change makes existing bounds
  explicit in tests and prevents regressions that could permit oversized wire
  payloads or omit one side of UDP accounting.

## Validation

Targeted tests passed:

- `cargo test -p espejismo-core protocol::request::tests`: 3 passed.
- `cargo test -p espejismo-server limits::tests`: 3 passed.

The oversized-payload regression initially exposed that the writer emitted the
command and authority before checking the payload size. Validation now happens
before writing any bytes, so a rejected request cannot leave a partial wire
message. No runtime benchmark was needed because this correctness change adds
no hot-path work for valid payloads beyond moving a length check earlier.
