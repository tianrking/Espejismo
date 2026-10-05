# Stream reset storm

## Findings and change

`tokio-yamux` kept each stream's sender in `Session::streams` after successfully
delivering a remote RST. Cleanup depended on the application polling or dropping
the corresponding `StreamHandle` and sending a later `Closed` event. Handles
that were not polled therefore left session map entries behind during a reset
storm.

After successfully queueing an RST to the stream, the session now removes its
sender immediately. The queued frame remains readable, so the handle still
reports `ConnectionReset`; failed or backpressured deliveries are not removed.
This follows yamux's stream-scoped reset behavior without changing the tunnel's
protocol or transport identity.

The approach is consistent with the stream lifecycle practices in the yamux
implementation referenced by `docs/research/REFERENCES.md`: release per-stream
state as soon as the protocol has definitively ended that stream.

## Expected effect and verification

The improvement is bounded to resource reclamation: after a burst of 512
successfully delivered resets, session-side stream tracking drops from 512
entries to zero immediately, even while all 512 handles remain alive. No
throughput claim is made.

Verification on 2026-10-05:

- `cargo test -p tokio-yamux --lib --offline -- --test-threads=1`: passed, 32
  tests. The reset storm test creates 512 streams, feeds RST to each, confirms
  the map is empty while handles remain alive, then confirms each handle reads
  `ConnectionReset`.
- `cargo test -p tokio-yamux --offline -- --test-threads=1`: all 32 unit tests
  passed; the integration test `one_way_bulk_transfer_exceeding_window` could
  not start because the sandbox denied its socket operation (`PermissionDenied`).
- Workspace `cargo fmt --all -- --check` reports pre-existing formatting
  differences in unrelated client, core, and server files. The changed Rust
  file was formatted directly with `rustfmt --edition 2024`.
