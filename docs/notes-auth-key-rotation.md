# Runtime authentication key rotation

## Findings and approach

The remote builds a complete `RemoteSettings` candidate and swaps it into the
shared settings lock only after config parsing and validation succeed. Each new
physical tunnel takes a snapshot of the current settings before its handshake;
existing tunnels already have independent session traffic keys. There is no
old/new PSK overlap. This atomic-snapshot approach matches the small operational
model and avoids a protocol change or extra key-slot configuration.

The auth documentation now states that a successful remote reload immediately
applies the new PSK to new physical tunnels. A server regression test performs
real client/server handshakes against the current runtime settings: after
replacement it checks rejection of the retired key and acceptance of the new
key. This exercises the same snapshot and handshake path used by incoming
tunnels.

The references review found no reason to adopt another project's protocol or
key-slot model for this focused test. The task is correctness-focused; no
throughput gain is claimed. Expected operational effect: new-key connections
work immediately after reload, with no overlap grace period; existing sessions
remain active under their negotiated traffic keys.

## Verification

- `cargo test -p espejismo-server runtime_psk_rotation_rejects_old_key_and_accepts_new_key` — passed. Covers old-key rejection and new-key acceptance after runtime snapshot replacement.
- `cargo test -p espejismo-core -p espejismo-server -q` — passed: core 182/182; server 35 passed, 1 ignored, plus integration/binary test targets passed (4 and 1 tests).
