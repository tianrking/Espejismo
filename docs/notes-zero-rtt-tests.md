# TLS 1.3 early-data boundary tests

## Findings and approach

The HTTPS proxy transport reuses a rustls client configuration so TLS 1.3
session tickets can resume subsequent proxy connections. Resumption does not
require 0-RTT. The production config leaves `ClientConfig::enable_early_data`
disabled, and the TLS 1.3 test server leaves `ServerConfig::max_early_data_size`
at zero. Together these boundaries ensure proxy CONNECT credentials and tunnel
traffic are sent only after the authenticated TLS handshake completes.

The upstream `tokio-rustls` early-data tests demonstrate that early data requires
both client opt-in and server authorization. Espejismo intentionally keeps both
disabled: CONNECT is a stateful proxy operation, so replayable unauthenticated
early data would add risk without changing the project's transport model.
This retains normal TLS behavior and does not introduce TLS camouflage.

## Change and expected effect

- Assert that the production HTTPS proxy TLS config does not enable early data.
- Assert that the TLS 1.3 ticket-resumption fixture does not authorize early
  data, while preserving its full/resumed/replenished ticket coverage.
- Runtime and throughput are unchanged; the expected benefit is catching an
  accidental policy change that could allow requests before handshake
  authentication.

## Experiment

- `$HOME/.cargo/bin/cargo test -p espejismo-server https_proxy_` — passed (6
  filtered HTTPS proxy tests), including client early-data-disabled policy and
  zero server early-data allowance alongside TLS 1.3 full/resumed handshakes.
- `$HOME/.cargo/bin/cargo test -p espejismo-server` — passed (41 passed, 1
  ignored, 0 failed). The ignored SOCKS loopback test requires a loopback bind.

Conclusion: TLS 1.3 session resumption still works with 0-RTT disabled; no test
regressions. This is a correctness guardrail, so no performance claim applies.
