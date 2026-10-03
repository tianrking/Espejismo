# Startup banner

## Findings and approach

The local and remote binaries previously emitted different readiness messages,
and neither included the binary version. Their existing structured fields
contained only a partial view of the effective runtime configuration.

Use one `service started` event in both roles, with stable `role` and `version`
fields plus non-secret runtime summary fields. The client reports remote,
mux/underlay, and enabled ingress addresses/modes; the server reports listener,
mux/underlay modes, and bound listener count. PSKs, admin tokens, and proxy
credentials are deliberately excluded. Existing per-listener client messages
remain useful for seeing when each proxy accept loop begins. This improves
first-log-line correlation and reduces config hunting during startup diagnosis;
no performance increase is expected and startup cost is negligible.

## Validation

`cargo test --offline -p espejismo-client -p espejismo-server` passed: 31
client tests and 17 server tests passed. The new regression test checks the
client ingress summary's enabled and disabled states. No failures or runtime
behavior regressions were observed. `cargo fmt --all -- --check` reports
pre-existing formatting differences in unrelated client TUN, core config, core
transport, and server test code; the touched client file was formatted directly
and those unrelated changes were left out.
