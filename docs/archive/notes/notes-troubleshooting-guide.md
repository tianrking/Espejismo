# Troubleshooting Guide

## Scope and rationale

Added a root-level `TROUBLESHOOTING.md` as a quick diagnostic path for users
who need to distinguish config, server connectivity/handshake, local proxy,
remote egress, service logging, and TUN cleanup failures. The repository
already has a detailed deployment troubleshooting table, so the new entry guide
links to it rather than duplicating its platform-specific cases.

The guide uses existing CLI diagnostics (`--check-config`, `--doctor`, and
`--probe-server`), established SOCKS5 test settings, existing runbook commands,
and documented known issues. It retains the project's single-client/single-
server operational model and makes no protocol, security, or product-position
claims beyond the existing documentation.

## Expected impact

This is a documentation-only change. It should reduce the time needed to
identify the failing layer by directing users through checks in dependency
order, while keeping the detailed support material in one maintained location.
No runtime or throughput improvement is expected or claimed.

## Validation

- Reviewed the referenced command examples against `docs/deployment/CLI.md`,
  `docs/deployment/RUNBOOK.md`, and the deployment troubleshooting guide.
- Confirmed linked files exist in the repository.
- No Rust code or executable behavior changed; cargo tests and performance
  benchmarks are not applicable to this documentation-only change.
