# Glossary expansion

## Analysis and changes

Reviewed the existing glossary against the newer FAQ, configuration examples
index, TUN, CLI, profile, NAT, and version-compatibility documentation. Those
pages use several terms whose meanings are operationally important and were
not yet defined in the glossary: client import profile, config profile, DNS
takeover, route takeover, server probe, wire protocol version, and CGNAT.

Added concise definitions for those terms to `GLOSSARY.md`. The definitions
preserve the distinctions made in the source documentation: import URLs carry
secrets without encrypting them, DNS settings are distinct from DNS resolution,
route takeover is opt-in TUN behavior, probes do not start listeners, and
binary release numbers do not imply wire compatibility.

Expected benefit: easier navigation across setup and operations docs and less
ambiguity around configuration, diagnostics, networking, and compatibility.
This is documentation-only; expected runtime performance change is 0% and no
runtime correctness change is expected.

## Review and validation

Cross-checked each definition against its corresponding maintained docs:
`docs/deployment/PROFILES.md`, `TUN.md`, `CLI.md`, `NAT.md`, and
`VERSION-COMPATIBILITY.md`. The glossary additions introduce no behavior or
configuration claims beyond those sources. Runtime tests and benchmarks do
not apply to this documentation-only change.
