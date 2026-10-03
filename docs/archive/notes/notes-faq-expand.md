# FAQ expansion notes

## Findings and scope

The existing FAQ already covers first-run setup, ports, handshake checks, UDP,
egress policy, installer scope, and documented transfer limitations. The
recurring operator questions with direct answers elsewhere in the repository
that were missing from the FAQ are upgrade compatibility and recovery after a
TUN route takeover. This expansion turns those into short FAQ entries and links
to the detailed procedures.

I attempted to inspect the public GitHub issues page for
`tianrking/Espejismo`, but it was unavailable to the web lookup in this run and
repository text search found no issue export or issue archive. Therefore these
additions are grounded in the maintained version-compatibility and TUN
troubleshooting docs, rather than attributed to specific issue numbers. If
issue history is supplied or becomes accessible, the FAQ can be reconciled
against it.

## Change and rationale

- Added an FAQ answer clarifying that binary release numbers do not establish
  interoperability, protocol versions are matched exactly, and config keys may
  change across upgrades. This points operators to the compatibility checklist.
- Added an FAQ answer for restoring routes and DNS after TUN client exit, with
  the documented cleanup command and privilege requirement.
- Kept the language consistent with the project's small-operations model and
  did not imply protocol camouflage or alter runtime behavior.

Expected improvement: no performance or correctness change. The two answers
make common upgrade and networking recovery steps discoverable from the main
FAQ; no numeric benefit is claimed.

## Review and validation

Manually checked the statements and links against
`docs/deployment/VERSION-COMPATIBILITY.md`, `RUNBOOK.md`, `TUN.md`,
`CLI.md`, and `TROUBLESHOOTING.md`. Verified linked paths exist and reviewed
`git status --short`. This is documentation-only, so cargo tests and
performance benchmarks do not apply.
