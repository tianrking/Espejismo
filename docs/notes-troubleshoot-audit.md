# Troubleshooting Guide Audit

## Scope and findings

Audited the root `TROUBLESHOOTING.md` and `docs/deployment/TROUBLESHOOTING.md`
against the CLI reference, client/server CLI implementation, config model, and
the Admin, Logging, and Configuration deployment guides.

- The listed `--check-config`, `--doctor`, `--probe-server`, and
  `--tun-route-cleanup` options exist. Local `--doctor` additionally probes
  remote TCP reachability; `--probe-server` performs the authenticated
  Espejismo handshake and does not start proxy listeners. The guide now
  distinguishes those checks so operators can choose the useful diagnostic.
- The handshake advice now reflects remote authentication: the client's PSK
  must match either the remote fallback PSK or a configured remote user PSK.
  Clock allowance applies when the handshake window feature is enabled.
- Admin `/status` and `/connections` are protected endpoints. The deployment
  symptom table now tells operators to query the bound admin listener with the
  bearer token and links to the guide with concrete curl examples.
- The `info,espejismo_core=debug` logging filter and external file rotation
  advice were checked against `docs/deployment/LOGGING.md`; no correction was
  needed. TUN cleanup advice and Windows variation match `docs/deployment/CLI.md`.

## Change and expected result

Clarified diagnostic command roles, credential matching, and admin API access
in the two troubleshooting guides. The expected improvement is fewer wasted
diagnostic steps and fewer failures caused by omitting endpoint authentication
or checking the wrong PSK. No runtime behavior or performance is changed.

## Verification

Documentation-only audit. Cross-checked each command and behavior against the
current source and linked deployment references; reviewed the rendered Markdown
structure and relative links. No cargo tests or throughput benchmark apply to
this documentation-only change. `git diff --check` is the whitespace check.
