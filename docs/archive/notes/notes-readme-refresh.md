# README Refresh

## Changes and rationale

- Reworked the English and Spanish README link bars to point readers to the
  quickstart, FAQ, operations index, configuration reference, protocol, and
  glossary.
- Added short in-page navigation to the main reader tasks and grouped the
  English operations links by setup, configuration, networking, upgrades,
  routine operation, monitoring, and design references. The operations index
  remains the complete guide catalog.
- Removed the duplicate troubleshooting entry and linked directly to the
  existing architecture, positioning, DNS, and version-compatibility guides.
- Clarified that the linked HK2/RK reports contain measurements rather than
  guaranteed throughput rates.

This is a documentation navigation and link-maintenance change. Expected
benefit: readers can reach common setup and operations guidance from either
README with fewer repeated links and less scanning. No performance or runtime
change is expected.

## Review / validation

- Checked every relative file link in `README.md` and `README_ES.md`; all local
  targets exist.
- Checked every in-page navigation anchor against its README heading; all
  anchors resolve.
- Confirmed scope and wording against `docs/deployment/INDEX.md` and
  `docs/POSITIONING.md`.
- Documentation-only change; build, tests, and throughput benchmarks do not
  apply. No behavioral or performance claims are made.
