# Crypto key rotation boundary tests

## Scope and design

The code has two separate rotation mechanisms: time-window derived handshake authentication keys (with configurable previous/future slots), and per-session traffic keys advanced by an authenticated `KeyUpdate` frame. This change targets traffic-key epoch exhaustion and frame update boundaries. A session rotates in strict stream order: the update frame is authenticated under the old key, then both sequence number and key epoch advance. There is no implicit old traffic-key grace period; the authenticated update is the synchronization point. The handshake's previous-window grace remains independently configurable.

The implementation previously saturated the generation counter at `u64::MAX`. That would derive the same next epoch repeatedly, so updates now fail explicitly on exhaustion and only commit the generation after HKDF succeeds. Tests cover first/last valid generation boundaries, zero and exact frame-count triggers, excluded frame types, disabled rotation, and concurrently submitted writes spanning repeated rotations.

Reference review: `docs/research/REFERENCES.md` points to shadowsocks-rust for AEAD frame handling and quic-go for state-machine design. The applicable lesson is to keep key-state transitions explicit and ordered. Espejismo retains its native authenticated-encryption framing and does not adopt another protocol or camouflage behavior, consistent with `docs/POSITIONING.md`.

Expected impact: no throughput claim; normal rotations are unchanged. Only the practically unreachable exhausted-counter case changes from silently reusing an epoch to returning an error.

## Verification

`cargo test -p espejismo-core` passed: 273 unit tests, 1 ignored loopback test, 1 config-example integration test, 10 HTTP proxy integration tests, and 1 doc test. The ignored test is the existing `tcp::tests::listener_recovers_after_accept_queue_is_drained` loopback bind case.

The focused tests covered generation 0 / `u64::MAX - 1` / `u64::MAX`, the key-update threshold at sequence counts 0, 1, and 2, excluded Padding/KeyUpdate frames, disabled rotation, an ordinary key update round trip, and 12 concurrently submitted frames spanning repeated updates. All passed. The pre-existing `server_accepts_previous_handshake_window` and `server_rejects_expired_handshake_window` tests also ran in the full suite and passed, confirming the distinct old-handshake-key grace boundary.

Conclusion: no performance change is claimed or expected. Normal key rotation remains unchanged; exhaustion now errors instead of saturating the generation and reusing an epoch. No loopback-dependent test was added.
