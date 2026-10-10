# Technical Blog Writing Guide

This guide helps contributors write accurate, useful technical articles about
Espejismo. It supplements the [documentation style guide](DOCUMENTATION_STYLE.md)
and applies to project blog posts, guest articles, and long-form technical
announcements.

## Choose a useful, bounded topic

- State the intended reader and the question the article answers.
- Keep each article focused on one feature, operational task, experiment, or
  engineering decision. Link to canonical guides for setup and reference details.
- Distinguish released behavior from planned, experimental, or unsupported work.
- Explain why the topic matters to an operator or contributor, without turning
  the article into a general product pitch.

## Use a readable structure

- Use a descriptive title and an opening that states the problem and the
  article's main conclusion.
- Organize the body with clear headings. Introduce necessary context before
  implementation details, then explain implications and limitations.
- Prefer concrete examples, short paragraphs, and diagrams only when they
  clarify a mechanism or setup.
- End with practical next steps or links to the relevant canonical documents.

## Make technical claims verifiable

- Check behavior against the current source, configuration defaults, protocol
  specification, and deployment documentation. Link to repository references
  where readers can verify important details.
- For measurements, give the environment, versions, workload, method, and
  relevant baseline. Separate observed results from estimates and explain
  meaningful limitations; never present one run as a universal guarantee.
- For security claims, describe the mechanism and threat boundary. Do not claim
  that Espejismo is invisible, undetectable, or disguised as another protocol.
  The core tunnel does not impersonate TLS, HTTP, or QUIC. WebSocket and HTTP/2
  underlays are real transports with their normal observable characteristics;
  see [project positioning](POSITIONING.md).
- Do not publish credentials, private endpoints, personal data, or exploitable
  operational details. Use clearly marked placeholders in examples.
- Keep claims about supported platforms, compatibility, and maturity aligned
  with the current release and canonical documentation.

## Keep examples safe and maintainable

- Use copyable commands and complete, minimal configuration snippets. Label
  placeholders and platform-specific steps.
- Avoid embedding large reference sections that will drift; link to the
  canonical guide instead.
- Spell the project name **Espejismo** and component names consistently as
  specified in [brand usage](BRANDING.md). Preserve literal identifiers.
- Use descriptive link text and relative links for repository documents.

## Review before publication

- Confirm factual claims and examples against the current implementation.
- Check that measurements include reproducible conditions and are not framed
  as guarantees.
- Check that security wording respects the project's positioning and threat
  boundaries.
- Verify links, headings, code fences, spelling, and placeholder labels.
- Ask a project maintainer to review changes involving protocol behavior,
  security claims, or performance conclusions.
