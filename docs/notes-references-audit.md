# References Link Audit

## Scope and findings

Audited the external project references in `docs/research/REFERENCES.md`. The entries previously used repository slugs as inline code rather than links, so readers could not navigate to the upstream sources or verify repository ownership. Replaced these with explicit GitHub links for the referenced projects and the official NUTTCP documentation page. `utls` now uses its current `refraction-networking` organization name, and the ambiguous `trojan-g` label is clarified as Trojan-GFW with its upstream repository.

## Change and rationale

Added links for 13 upstream repositories/documentation targets while preserving the existing learning notes and the positioning boundary in `docs/POSITIONING.md`. This is a documentation usability and traceability improvement; no code, protocol, dependencies, or runtime behavior changed. Expected performance improvement: none (not applicable).

## Verification

Checked each GitHub repository URL by opening its destination. All resolved to public repository pages; the old Hysteria URL redirects to the current `HyNetworks/hysteria` owner. NUTTCP's GitHub guess did not resolve, so linked its official `nuttcp.org` documentation page instead. No Rust build or tests were run because this change only edits documentation.

## Conclusion

All external references in `REFERENCES.md` now have explicit destinations, and the checked destinations are reachable. No performance claim is made; the project positioning is unchanged.
