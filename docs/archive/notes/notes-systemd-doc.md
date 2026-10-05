# systemd documentation

## Change and rationale

Added `docs/deployment/SYSTEMD.md` to explain the checked-in local and remote
units, first-time installation, config permissions, journald, optional
systemd hardening directives, and the special privilege requirements of TUN
mode. Linked it from packaging documentation so Linux operators can find it.
The existing unit files already enable several safe defaults; the guide makes
their effect clear and documents additional restrictions as operator choices.

This is documentation-only and preserves the project's two-binary, one-TOML
deployment model and non-impersonation positioning. Expected improvement is
less operator guesswork during Linux installation and safer handling of
credentials. There is no runtime or performance change, so no throughput gain
is expected.

## Validation

Manually checked the example service names, executable/config paths, account,
and enabled restrictions against `deployments/systemd/` and related logging,
shutdown, and TUN documentation. No tests or build were run because the change
is documentation-only; there is no performance claim to benchmark.
