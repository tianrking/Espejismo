# Support and Issue Reports

Use this guide to find help for Espejismo and to submit a report that includes
enough context to investigate. For deployment symptoms, first check the
[FAQ](deployment/FAQ.md) and [troubleshooting guide](deployment/TROUBLESHOOTING.md).

## Where to ask

- For reproducible bugs, documentation errors, and feature requests, open a
  [GitHub issue](https://github.com/tianrking/Espejismo/issues). Search existing
  issues first, then choose a concise title that describes the symptom or
  request.
- For suspected security vulnerabilities, do not open a public issue. Follow
  the private reporting process in the [security policy](../SECURITY.md).
- For setup and operating questions, include only the sanitized details listed
  below. Public issues are visible to everyone.

There is no guaranteed response time. Include a minimal reproduction and
separate observed behavior from expected behavior so maintainers can assess
the report.

## Information to include

Copy and fill in the sections that apply. Mark unavailable information as
unknown; do not guess.

```text
### Summary
Describe the problem or request in one or two sentences.

### Espejismo version
Client version:
Server version:
Install source (release archive, build from source, or other):

### Environment
Client OS and version:
Server OS and version:
Relevant network path (for example, client -> VPS; redact hostnames/IPs):

### Configuration and command
Relevant settings only, with secrets and private addresses replaced by
<redacted>:
Command used, with private paths or values redacted:

### Steps to reproduce
1.
2.

### Expected behavior

### Actual behavior
Include the exact error text and approximate time, with sensitive values removed.

### Diagnostics
Relevant sanitized client/server log lines:
Checks already run (for example, --check-config, --doctor, --probe-server):
Workaround or frequency, if known:
```

For a documentation issue, include the page and heading. For a feature request,
describe the use case and constraints; avoid prescribing a large redesign when
a small outcome would solve it.

## Remove sensitive data

Before posting, inspect the full text and attachments. Never include:

- Shared or per-user PSKs, proxy usernames/passwords, admin tokens, or private
  keys.
- An unredacted TOML file, environment dump, command history, or packet capture.
- Public or private IP addresses, hostnames, usernames, or filesystem paths if
  you do not want them published. Replace them consistently with labels such as
  `<client-host>` and `<server-host>`.
- Logs that expose credentials or unrelated user traffic.

Keep enough context to reproduce the issue: retain setting names, relevant
versions, error messages, and the order of events while replacing secret
values. If sensitive material was already exposed, revoke or rotate it.

## Related guides

- [FAQ](deployment/FAQ.md) and
  [troubleshooting](deployment/TROUBLESHOOTING.md) for operator diagnosis.
- [Error reference](deployment/ERRORS.md) for common diagnostic messages.
- [Contributing guide](../CONTRIBUTING.md) for code changes and validation.
- [Security policy](../SECURITY.md) for private vulnerability reporting.
