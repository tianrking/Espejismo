# UDP Response Timeout Boundaries

## Findings and approach

The TUN UDP relay sends independent datagram requests over the authenticated
TCP mux. `read_udp_response` reads a two-byte length prefix followed by the
response payload. Previously each stage received a separate full timeout, so a
slow response could consume nearly twice `udp_timeout_secs`. The timeout is
intended to bound one datagram response as a whole.

The references guide points to shadowsocks-rust for bounded UDP relay handling.
That is applicable at the datagram relay boundary; Espejismo keeps its existing
TCP-first transport and does not add a UDP underlay or NAT session table.

## Changes and expected effect

- Apply one timeout to the complete response read, covering both framing and
  payload. This caps total response waiting at the configured duration.
- Add regression coverage for zero timeout, a partial length prefix, and a
  partial payload. Keep the existing timeout-then-fresh-datagram test.
- Update the TUN guide to define the timeout as covering the complete response.

Expected behavior improvement: a stalled datagram response is bounded by one
configured timeout instead of up to two; no throughput change is expected.

## Verification

- `cargo test -p espejismo-client tun::tests::udp_response -- --nocapture`:
  5 passed, 0 failed. Covers queue behavior plus timeout expiry, fresh window,
  zero timeout, and partial header/payload expiry. These tests use in-memory
  duplex streams and do not require loopback sockets.
- `cargo test -p espejismo-client`: 48 passed, 0 failed, 0 ignored.
- Correctness improvement confirmed; no performance claim or throughput change
  is claimed for this timeout behavior change.
