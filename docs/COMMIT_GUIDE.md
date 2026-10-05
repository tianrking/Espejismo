# Commit Message Guide

This guide defines the commit message format used in the Espejismo repository.
Clear messages make the project history easier to scan and explain why a
change was made.

## Subject

- Write a concise English subject in imperative form, describing the concrete
  change (for example, `Tune yamux window for high-RTT paths`).
- Capitalize the first word and omit the trailing period.
- Keep the subject focused on one change. Avoid vague subjects such as
  `Update files` or `Fix stuff`.
- Do not add tags, ticket numbers, or other prefixes unless the change context
  specifically calls for them.

## Body

For changes that need context, leave one blank line after the subject and add a
short body describing the reason and important implementation details. Mention
validation when it helps explain confidence in the change. Keep claims tied to
checks or measurements that were actually performed.

Documentation-only commits can state which guidance was added, how it helps
contributors, and which documentation checks were run. They do not need to
claim runtime or performance effects.

## Attribution and examples

Commit messages describe the change itself. Do not add AI-generation notices,
assistant names, or `Co-Authored-By` trailers.

```text
Clarify support contact expectations

Document the public issue channel and private security reporting path.
Reviewed the links and terminology against the support and security guides.
```

```text
Tune yamux window for high-RTT paths

Increase the stream receive window to reduce stalls on high-latency links.
Record the benchmark conditions and before-and-after throughput results.
```

## Review

Before finalizing a commit, check that its subject matches the diff, its body
does not overstate validation, and it contains no generated files or unrelated
changes. Follow the repository's [documentation style](DOCUMENTATION_STYLE.md)
when editing this guide.
