# SOCKS5 authentication boundary tests

## Findings and change

`ProxyAuth::validate` already rejected an empty username and credentials over
255 bytes, while SOCKS5 RFC 1929 authentication rejects zero-length username
and password fields on the wire. It did not reject an empty configured
password, so an operator could supply a configuration that parses but can
never authenticate through the SOCKS5 ingress. Validation now rejects that
configuration with a field-specific error. The raw credential matcher still
has a unit test for empty configured passwords as a defensive primitive; valid
runtime configuration cannot reach that state.

The SOCKS5 exchange tests cover method negotiation (including no acceptable
method, password-only offers when auth is disabled, and refusing no-auth when
credentials are configured), unsupported RFC 1929 subnegotiation versions,
empty username/password, wrong username/password, and successful auth. They
use Tokio duplex streams, so they do not require loopback sockets.

The reference list points to sing-box and shadowsocks-rust as examples of
maintained proxy implementations. This change follows the same narrow protocol
boundary approach: enforce credentials at the ingress/configuration boundary;
it adds no protocol, dependency, or camouflage behavior and keeps the existing
minimal local proxy model.

## Expected impact

No throughput change is expected. Invalid empty-password configurations fail
early and report the setting that must be corrected; valid SOCKS5 authentication
and method selection remain unchanged.

## Verification

- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core ingress::socks5::tests`: 27 passed.
- `$HOME/.cargo/bin/cargo test --offline -p espejismo-core`: 279 passed, 1 ignored (the existing loopback-bind test); config example, HTTP proxy tests, and doc tests passed.
- `$HOME/.cargo/bin/cargo fmt --all -- --check` and the package-scoped format check report existing formatting diffs across the workspace/core files, including unrelated files. Direct `rustfmt --check` on the touched files also flags pre-existing import/layout style differences; no formatter rewrite was applied.

No ignored tests were added. The SOCKS5 auth regression cases all ran in the sandbox.
