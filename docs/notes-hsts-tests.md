# HSTS header boundary tests

## Findings and change

The repository has no HSTS implementation, configuration, or
`Strict-Transport-Security` response header. Its only built-in HTTP response is
the optional plaintext fallback in `crates/espejismo-server/src/fallback.rs`.
HSTS user agents accept the policy only when received over a secure transport,
so adding the header to this HTTP response would be ineffective and would
create misleading behavior. No TLS listener or application response path in
this change owns a secure-origin policy.

Added a boundary assertion to the fallback response test: header names are
checked case-insensitively and the plaintext response must not contain
`Strict-Transport-Security`. This captures the applicable HSTS edge for the
existing response path without adding policy configuration or changing the
project's protocol behavior. The scope follows the HTTP boundary testing style
used in sing-box and shadowsocks-rust; no upstream HSTS implementation is
needed for this negative protocol assertion. The change does not affect the
project's positioning in `docs/POSITIONING.md`.

## Expected benefit

Prevents future changes from suggesting an HSTS policy over plaintext HTTP.
This is a correctness regression guard; no performance improvement is
expected or claimed.

## Verification

- `cargo test --offline -p espejismo-server fallback::tests` — 9 passed, 0
  failed. The response test exercises the HSTS absence check alongside the
  existing response status and header checks; remaining fallback tests retain
  coverage for HTTP method and TLS-preface routing boundaries.
- `cargo test --offline -p espejismo-server` — 55 passed, 0 failed, 1 ignored
  (`requires loopback bind`).
- `git diff --check` — passed.

Formatting checks report existing style differences in this file and many
unrelated workspace files; no broad formatting changes were made. No
regression was observed in the server package.
