# Certificate Rotation Boundary Tests

## Findings and approach

Espejismo has no tunnel TLS certificate: its core connection uses the native
PSK-authenticated protocol. The only TLS client in the remote process connects
to an HTTPS egress proxy, whose certificate and private key are owned by that
proxy operator. Runtime reload can change the egress proxy configuration used
for future streams; it cannot rotate the proxy's certificate or reconfigure an
already established TLS connection. Keep this ownership boundary intact and
test the reload behavior that Espejismo actually controls.

## Change and expected effect

- Extend the remote settings atomic-replacement regression test to change the
  HTTPS proxy endpoint alongside users, idle timeout, and stream limit, then
  assert the committed proxy selection matches the complete candidate.
- Extend invalid-candidate coverage to verify the live proxy selection remains
  unchanged when candidate settings cannot be built.
- Clarify certificate lifecycle and reload scope in the TLS deployment guide.
- Expected effect: detect partial application or accidental proxy change on a
  failed reload. This correctness-only change has no runtime or throughput
  impact and does not add tunnel TLS or certificate management.

## Experiment

- `cargo test --offline -p espejismo-server` passed: 27 tests passed, 0
  failed. The successful replacement test verifies that the HTTPS proxy endpoint
  is committed with the other candidate settings; the invalid-candidate test
  verifies the live proxy selection stays unchanged after settings construction
  fails.
- No throughput benchmark applies to this correctness-only change.
