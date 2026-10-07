# Frame size boundary tests

## Scope and approach

The normal encrypted frame codec caps ciphertext at 256 KiB. Because ciphertext
contains a one-byte frame type and a 16-byte XChaCha20-Poly1305 tag, the
largest legal payload is 262127 bytes. The codec already rejects payloads
above this limit before encryption and rejects oversized received lengths
before allocating a buffer; the missing coverage was an end-to-end boundary
check for the exact maximum and first invalid size.

Add a duplex-stream regression test that sends and receives an exact-limit
payload, then checks that a payload one byte larger is rejected. Record those
limits in `docs/PROTOCOL.md` so implementations share the same boundary. This
follows bounded parsing practice from the protocol references (including
shadowsocks-rust's AEAD framing): validate lengths before allocating, with no
change to Espejismo's wire format or positioning.

## Expected benefit

No runtime performance change is intended. The test guards the inclusive
256 KiB ciphertext boundary and prevents accidental acceptance or rejection of
payloads at either edge.

## Verification

- `cargo fmt --all` completed.
- `cargo test -p espejismo-core protocol::framing::tests::normal_frame_payload_limit_roundtrips_and_rejects_one_byte_over --offline`: passed (1 test).
- `cargo test -p espejismo-core protocol::framing::tests --offline`: passed (6 tests, including stealth framing and key rotation).
- No benchmark applies; the change only adds regression coverage and specification text.
