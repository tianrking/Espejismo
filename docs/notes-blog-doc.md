# Blog Writing Guide

## Analysis and plan

The repository had a general documentation style guide and brand/positioning
references, but no focused guidance for technical blog posts. Added
`docs/BLOG_WRITING_GUIDE.md` to give contributors a reusable workflow for
choosing a bounded topic, structuring an article, supporting technical and
performance claims, handling security boundaries, and reviewing examples
before publication.

The guide links to the existing canonical references rather than duplicating
their full guidance. It explicitly preserves the product's positioning: the
core tunnel does not impersonate protocols, optional WebSocket and HTTP/2
underlays are real transports, and security language must not promise
invisibility or undetectability.

## Validation and expected impact

This is a documentation-only change. No runtime behavior or performance is
changed, so no benchmark or Rust test applies. Reviewed the new guidance against
`docs/DOCUMENTATION_STYLE.md`, `docs/BRANDING.md`, and `docs/POSITIONING.md`.
Expected impact is more consistent, verifiable, and appropriately bounded
technical blog content; no numerical performance gain is claimed.
