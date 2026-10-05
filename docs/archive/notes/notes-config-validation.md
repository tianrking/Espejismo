# Config validation clarity

## Analysis and changes

`parse_config` already rejected many out-of-range values, but its messages often
only named a comparison (for example, `must be <= 65535`). Operators then had
to search the config reference to find the full interval and a valid TOML value.
This change keeps the existing validation rules and adds explicit intervals
and copyable examples for common boundary settings: TUN prefix and MTU,
handshake-window counts, stream/physical-connection limits, obfuscation chunk
ordering, HTTP/2 frame size, and stealth frame size. Tests exercise accepted
boundary values and verify that representative failures include field, range,
and example. No dependency, protocol, or runtime behavior changes; the expected
benefit is faster diagnosis of invalid configs rather than a performance gain.

## Verification

- `cargo fmt --check`: pass after applying `cargo fmt`.
- `cargo test -p espejismo-core config::tests`: 23 passed, 0 failed.
- `cargo test -p espejismo-core`: 110 passed, 0 failed.
- Correctness result: new rejection-message and boundary regression cases pass;
  no regression in the core crate suite. No performance claim applies to this
  validation-only change.
