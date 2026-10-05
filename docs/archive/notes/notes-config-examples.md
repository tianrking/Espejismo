# Configuration scenario examples

## Analysis and changes

`docs/deployment/CONFIG.md` already described individual fields, defaults, and
one short baseline, while the maintained TOML file exposed many options at
once. Operators still had to assemble common deployments themselves. Added
three complete single-file configurations for a one-user SOCKS5 setup, a
multi-user server with per-user quotas and bandwidth limits, and a TUN client
with route and DNS takeover.

Each example preserves the existing one-file model: the remote and local
binaries read their respective sections from the same file. The text calls
out which credentials must match and where platform-specific TUN privileges
and route protection matter. No runtime behavior or project positioning
changed.

Expected benefit: operators can copy a representative end-to-end config and
adapt only deployment-specific addresses and secrets, reducing configuration
assembly errors. This is a documentation and usability improvement; no runtime
or performance change is expected.

## Verification

- `cargo test -p espejismo-core --test doc_config_examples`: passed; the
  integration check parsed all fenced TOML examples in the scanned docs.
- No performance experiment applies to this documentation-only change.
