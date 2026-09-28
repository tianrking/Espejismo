---
name: Bug report
description: Report a reproducible problem
labels: [bug]
body:
  - type: input
    id: version
    attributes:
      label: Version
      description: Output of `espejismo-remote --version` / release tag
    validations:
      required: true
  - type: input
    id: platform
    attributes:
      label: Platform
      description: OS and arch, e.g. linux x86_64
    validations:
      required: true
  - type: textarea
    id: config
    attributes:
      label: Relevant config (redact keys and PSKs)
      render: toml
  - type: textarea
    id: repro
    attributes:
      label: Reproduction steps
    validations:
      required: true
  - type: textarea
    id: logs
    attributes:
      label: Logs (redact keys and PSKs)
      render: shell
