# Security Policy

## Reporting a vulnerability

Please report suspected security vulnerabilities privately. Do not open a
public issue or discussion containing exploit details, proof-of-concept code,
or information that could help someone attack users.

Use GitHub's **Report a vulnerability** option on the repository's Security
tab when it is available. If private vulnerability reporting is unavailable,
contact the maintainer through the GitHub profile at
https://github.com/tianrking and ask for a private reporting channel before
sending sensitive details.

Include, when possible:

- The affected version, component, and configuration.
- The security impact and conditions needed to reproduce it.
- A minimal reproduction or proof of concept, with secrets and personal data
  removed.
- Any mitigation already identified.

Please allow the maintainer a reasonable opportunity to investigate and
prepare a fix before sharing the report publicly. There is no guaranteed
response or remediation timeframe. We will acknowledge reports when possible,
may ask for more information, and will coordinate any public disclosure with
the reporter.

## Supported versions

Security fixes are generally made on the latest development version. Older
releases may not receive backported fixes; users should plan to update to the
latest release when one is available.

## Scope

Reports are welcome for vulnerabilities in Espejismo's code, protocol
implementation, and official distribution artifacts. This policy does not
promise that the software is invisible or that it can defeat every form of
traffic analysis. See [the project positioning](docs/POSITIONING.md) and
[protocol security notes](docs/PROTOCOL.md#security-notes) for the stated
design and limitations.
