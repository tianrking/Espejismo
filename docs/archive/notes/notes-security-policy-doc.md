# Security policy documentation

## Scope and approach

Add a root `SECURITY.md` describing a private vulnerability reporting path,
the information useful in a report, coordinated disclosure expectations,
supported-version guidance, and project security scope. The repository has no
published security email address, so the instructions point first to GitHub's
private vulnerability reporting feature when enabled and otherwise ask the
reporter to contact the maintainer through GitHub to obtain a private channel.
No response-time or remediation guarantee is introduced.

The policy links back to the existing positioning and protocol notes. It does
not claim invisibility, alter the protocol, or change the project's small
operating model.

## Expected benefit

The policy gives researchers a clear route to report security issues without
publishing exploit details and helps maintainers receive enough context to
reproduce and assess reports. As a documentation-only change, expected runtime,
performance, and protocol impact is zero; no quantitative security improvement
is claimed.

## Verification and outcome

Manually checked the report path wording against the repository's available
contact information and checked the internal links against the existing
documentation. `git diff --check` passed. No Rust build or tests were run
because this change contains documentation only.
