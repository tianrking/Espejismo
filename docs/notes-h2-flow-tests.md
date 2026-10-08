# H2 flow control window boundaries

## Findings and approach

The HTTP/2 underlay forwards bytes through h2's `SendStream` and `RecvStream`,
and returns receive capacity after the bytes have been copied into its bounded
duplex stream. Configuration already required the protocol minimum of 65,535
bytes and required the connection window to be at least the stream window, but
did not check the protocol's maximum. h2 0.4.15 defines `MAX_WINDOW_SIZE` as
`(1 << 31) - 1` and asserts that stream-window values do not exceed it; this
could turn a bad config into a panic during underlay setup.

HTTP/2 flow control uses independent stream and connection credits (RFC 9113,
sections 6.9 and 6.9.2). The existing Yamux documentation and tests also make
window boundaries explicit. This change applies that boundary-testing approach
to the optional real HTTP/2 transport without changing the tunnel protocol or
its positioning.

## Changes and expected effect

- Reject either configured HTTP/2 window above 2^31-1 during config parsing.
- Add parse tests for minimum and maximum accepted values and one-above-maximum
  rejection of both stream and connection windows.
- Document the HTTP/2 underlay bounds alongside the independent tunnel mux
  flow control.

This is a correctness and configuration-hardening change; expected throughput
change is 0%. Invalid configurations now fail early with a field-specific
validation error instead of reaching h2's assertion.

## Experiment

- `cargo test -p espejismo-core --offline --quiet`: passed 214 unit tests, with
  1 existing loopback-bind test ignored by its sandbox annotation; the
  integration suites also passed (1 + 8 + 1 tests). This includes
  `config::tests::validates_http2_flow_control_window_boundaries`, exercising
  min/max acceptance and overflow rejection for both window fields.
- `rustfmt --check crates/espejismo-core/src/config/mod.rs` and
  `git diff --check`: passed. Workspace-wide `cargo fmt --all -- --check`
  reports pre-existing formatting differences in unrelated files; no unrelated
  files were changed.
- No throughput benchmark was run because this change does not alter runtime
  flow-control behavior for valid settings.
