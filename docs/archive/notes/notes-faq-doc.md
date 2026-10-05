# FAQ documentation notes

## Change and rationale

Added `docs/deployment/FAQ.md` as a short entry point for common questions that
were previously answered across the README, CLI, TUN, egress, and
troubleshooting documents. It covers product behavior, installation topology,
ports, handshake diagnostics, UDP/TUN constraints, remote egress, operational
setup, secret handling, and known limitations. README and deployment quickstart
link to the FAQ so new users can find it early.

The answers follow `docs/POSITIONING.md`: Espejismo is an authenticated encrypted
tunnel that does not impersonate another protocol and does not claim
invisibility. The FAQ does not change runtime behavior.

## Expected improvement

No performance or correctness change is expected. Consolidating repeated
first-line answers should reduce the number of pages a new operator needs to
consult for basic setup and triage; this is a qualitative documentation benefit
and no numeric improvement is claimed.

## Review and validation

Manually checked the statements against `README.md`,
`docs/deployment/QUICKSTART.md`, `CLI.md`, `TUN.md`, `EGRESS.md`,
`TROUBLESHOOTING.md`, `KNOWN_ISSUES.md`, and `POSITIONING.md`. Verified the new
relative links resolve to files in the repository and reviewed `git status
--short`. Since this is a documentation-only change, no cargo tests or
performance benchmark apply.
