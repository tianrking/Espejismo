# Auth replay tests

## Scope and rationale

The handshake already rejected byte-for-byte replays using the authenticated
first-packet digest and separately tracked the X25519 ephemeral public key.
The missing regression case was a captured authenticated hello carried in a
freshly randomized outer envelope: its wire digest changes, but its ephemeral
key does not. Replay admission also used two sequential cache inserts, allowing
the digest to be retained when the public-key check rejected the handshake.

The change makes replay admission check and insert both identifiers as one
operation. It adds a handshake-level test for the fresh-envelope replay and a
cache test proving failed admission leaves the other identifier unrecorded.
`docs/PROTOCOL.md` now records this behavior. This retains the project's
authenticated-encryption and non-camouflage protocol design; the transport
references in `docs/research/REFERENCES.md` do not offer a directly relevant
replay-cache mechanism, so no external transport design was imported.

## Expected effect

No throughput change is expected. The handshake now avoids a partial replay
cache insertion on rejection and has explicit regression coverage for reuse of
an authenticated hello under a different randomized envelope.

## Verification

`$HOME/.cargo/bin/cargo test --offline -p espejismo-core` passed: 161 unit
tests, 1 documented-config integration test, 4 HTTP proxy integration tests,
and 1 doctest (167 total). The replay coverage exercised exact packet replay,
fresh-envelope replay, failed atomic admission, and normal handshake paths.
`git diff --check` passed. Workspace `cargo fmt --all -- --check` reports
formatting differences in numerous unrelated existing files; no workspace-wide
formatting rewrite was applied.
