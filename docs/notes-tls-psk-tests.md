# PSK key exchange boundary tests

## Findings and approach

The topic's PSK is the tunnel PSK, not a TLS-PSK cipher suite: per
`docs/POSITIONING.md`, the core protocol is an encrypted tunnel and does not
impersonate or depend on TLS. `docs/research/REFERENCES.md` points to
shadowsocks-rust for readable Rust AEAD patterns, but this change needs no new
protocol or dependency. The existing core uses HMAC for PSK authentication and
HKDF over a PSK-and-nonce salt plus the ephemeral X25519 shared secret for
directional frame keys. Server tests already cover replacing the runtime PSK
snapshot, but core tests did not directly assert key binding and direction.

Added deterministic crypto boundary coverage for matching client transmit and
server receive keys, independent directional keys, and changed outputs after
either PSK rotation or changing the ephemeral shared secret. Added parser
checks at the 16-byte decoded PSK floor for raw, hex, and base64 forms. Updated
the authentication guide to state the protocol boundary and forward-secrecy
condition accurately. This is correctness coverage only; expected performance
change is none.

## Verification

- `cargo test --offline -p espejismo-core` — passed: 331 unit tests passed, 1
  ignored, plus the config example, HTTP proxy integration tests, and doctest.
  The two new tests cover key binding/directions and decoded PSK length
  boundaries. No loopback sockets are used by the new tests.
- `cargo test --offline -p espejismo-server
  runtime_psk_rotation_rejects_old_key_and_accepts_new_key` — passed. Confirms
  the retired key fails and the replacement key completes a handshake after
  settings replacement.
- No benchmark applies: the change adds tests and documentation and does not
  alter runtime behavior.
