# Media kit documentation

## Findings and plan

The repository had a positioning guide and brand usage rules, but no
publication-ready media kit. The README and positioning guide provide the
canonical project description; they establish that Espejismo is an encrypted
tunnel, not protocol camouflage. The repository also has no approved graphic
logo or press artwork, and the support guide identifies GitHub issues as the
public contact route.

Added `docs/PRESS-KIT.md` with concise English boilerplate, a Chinese
introduction, a fact sheet, safe technical framing, explicit claim boundaries,
asset guidance, and links for press follow-up. The content points readers to
existing canonical documentation for current release details and avoids
duplicating operational instructions. Expected benefit: less inconsistent
project description and fewer unsupported claims when community members write
about Espejismo. This is a documentation-only change; no runtime or measurable
performance improvement is expected.

## Validation and outcome

- Compared product description and technical boundaries with
  `README.md`, `docs/POSITIONING.md`, and `docs/BRANDING.md`.
- Confirmed public contact and security-reporting paths against
  `docs/SUPPORT.md` and `SECURITY.md`.
- Checked all repository-relative links in the new page against existing files.
- Documentation-only change; build, runtime, and performance experiments do
  not apply. Outcome: media guidance is available in one page and preserves the
  established product positioning.
