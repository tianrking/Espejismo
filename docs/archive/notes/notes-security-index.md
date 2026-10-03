# Security documentation index

## Scope and approach

Added `docs/SECURITY.md` as a navigation page for existing security material.
It groups protocol and authentication details, deployment controls, and
credential/incident guidance, and links to the existing root `SECURITY.md` for
private vulnerability reporting. Added entry points from the README and the
operations documentation index so readers can find the security material
without knowing individual guide names.

The index summarizes documented behavior; it adds no protocol or security
claims. Its scope follows `docs/POSITIONING.md`: authenticated encrypted
chaos, no protocol impersonation, and no claim of invisibility.

## Expected benefit

This makes the existing security guidance easier to discover and gives users
a single starting point to find credential, access-control, egress, and
reporting instructions. The change is documentation-only, so expected runtime,
performance, and protocol impact is zero. No quantitative security improvement
is claimed.

## Verification and outcome

Manually checked the new relative links against the files in the repository
and reviewed the index descriptions against their linked guides. `git diff
--check` passed. No Rust build or tests were run because this change contains
documentation only.
