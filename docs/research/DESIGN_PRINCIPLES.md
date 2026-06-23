# Espejismo Design Principles

## Native First

The current implementation is a native Rust transport built around Tokio TCP.
Native local and remote binaries are the stability target. Browser and WASM
support should be designed as a separate transport layer rather than forcing the
native runtime into a browser shape.

## Protocol Core Isolation

Cryptography, replay protection, framing, padding, puzzles, ingress parsing, and
transport bridging live in separate modules. Binaries compose these modules but
do not own protocol details.

## Fail Fast Below the Multiplexer

Encrypted frame authentication failure means the physical connection is broken.
The protocol does not scan for a new byte boundary after corruption. Higher
layers can reconnect or migrate, but byte-stream resynchronization is deliberately
out of scope.

## Mask Metadata Without Sacrificing Stability

The standard handshake uses a variable-length masked envelope instead of placing
the HMAC, timestamp, ephemeral public key, and server reply at fixed cleartext
offsets. The standard encrypted transport still uses length-based super-frames
for robust async I/O, but the 4-byte length field is masked with an HKDF-derived
per-direction sequence mask. This removes plaintext length and handshake-layout
signals while preserving stable parsing. Stealth mode intentionally trades that
variable-length framing for fixed-size encrypted blocks and paced padding when
the operator chooses the stealth profile.

## Bounded Public-Side Resource Use

Invalid handshakes never receive application data. The remote can hold invalid
sockets in a bounded silent pool, but every public-side resource has a hard cap
and expiry. The implementation favors service survival over unbounded peer
retention.

## Test Real Paths

Every major feature should be backed by an executable check. Unit tests cover
the protocol pieces today; end-to-end and chaos tests should be rebuilt as
dedicated CI jobs or external harnesses instead of living as ad hoc release
scripts.
