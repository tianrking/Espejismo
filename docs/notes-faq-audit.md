# FAQ implementation audit

## Scope and findings

Audited `docs/deployment/FAQ.md` against the current CLI, config defaults and
parser, protocol/version documentation, TUN guide, quickstart, installer
documentation, and known-issues notes. The operational statements checked
against implementation include the default proxy ports (`127.0.0.1:6680` and
`:6681`), TCP listener port example (`6690`), `--probe-server` behavior,
wire-version matching, TUN UDP port 443 blocking, and route cleanup behavior.
These were consistent with the referenced docs and code.

One answer was too easy to misread: the handshake answer named the server's
fallback PSK or selected user's PSK without explaining which applies when user
authentication is configured. The quickstart shows that its `default` user
must have a PSK matching `[shared].psk`. The FAQ now states this distinction
and ties it to the example config.

## Change and expected benefit

Updated the handshake troubleshooting answer in `docs/deployment/FAQ.md` to
differentiate fallback-PSK and per-user authentication, and to call out the
two matching PSK fields used by the default example. This should prevent
operators from changing only `[shared].psk` and then debugging a predictable
handshake rejection. Expected benefit is fewer setup errors; this is a
qualitative documentation improvement with no numeric claim.

No runtime behavior, protocol, or project positioning changed. No performance
or correctness improvement is expected from this documentation-only change.

## Review and validation

Compared the edited answer with `docs/deployment/QUICKSTART.md`,
`docs/deployment/CONFIG.md`, `docs/deployment/AUTHENTICATION.md`,
`docs/deployment/CLI.md`, `docs/deployment/TUN.md`,
`docs/deployment/VERSION-COMPATIBILITY.md`, `docs/KNOWN_ISSUES.md`, and the
relevant client/server config and handshake code. Confirmed the links in the
FAQ point to existing repository paths and reviewed `git status --short`.
No cargo tests or throughput benchmark apply to this docs-only audit.
