# Traffic Shaping Documentation

## Research and findings

Reviewed `docs/research/REFERENCES.md` and `docs/POSITIONING.md` first. The
project's defining constraint is authenticated encrypted chaos without protocol
impersonation, paired with a small operational model. This documentation task
does not borrow other projects' wire formats or change that positioning.

Compared `docs/PROTOCOL.md`, `docs/deployment/CONFIG.md`,
`docs/deployment/PROFILES.md`, the framing/configuration implementation, and the
stealth profile overlay. Existing descriptions covered fields but spread the
mechanism, selection guidance, costs, and non-camouflage boundary across
several files.

## Change and expected benefit

- Added `docs/deployment/OBFUSCATION.md` to explain normal variable framing,
  stealth fixed-size encrypted frames, the optional idle shaper, a peer-shared
  TOML example, trade-offs, and operational selection guidance.
- Added links from the configuration guide and README so operators can find the
  walkthrough.
- Reiterated that traffic shaping changes observable properties but makes no
  invisibility or protocol-camouflage claim.

Expected runtime performance change: **0%**. No executable code, defaults, or
wire behavior changed. Expected benefit is clearer configuration choices and
fewer mistaken assumptions about stealth mode, peer settings, or protection
guarantees.

## Validation and evidence

This is a pure documentation change. A before/after throughput benchmark would
run identical binaries and cannot measure the documentation benefit, so no
benchmark or runtime test was run and no runtime gain is claimed. Cross-checked
the documented profile names, frame selection, shaper behavior, and config
validation against the implementation and protocol/config references. Reviewed
Markdown links and the final changed-file list. Runtime regression risk: none
from documentation-only edits.
