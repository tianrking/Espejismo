# Incident Postmortem

Copy this document for a significant incident or recurring operational issue.
Keep it blameless: explain how the system and operating conditions allowed
the incident, not who made a mistake. Record facts separately from hypotheses
and mark unknowns explicitly. Adapt the depth to the impact; a short, complete
review is more useful than an unfinished report.

Do not include PSKs, admin tokens, proxy credentials, private keys, or
unredacted configuration. Redact or restrict access to logs and host details
that could expose secrets or personal data. See [On-Call Response](ONCALL.md)
for triage and recovery steps.

## Summary

- **Incident ID / title:**
- **Status:** Open / monitoring / closed
- **Severity and rationale:** (Use the deployment's own severity scheme.)
- **Incident lead / participants:**
- **Start and end:** (Include timezone; distinguish customer impact from
  investigation and recovery.)
- **Services, peers, and versions affected:**
- **Customer or operator impact:** (Who was affected, what failed, and for how
  long? Include known scope and what remains unknown.)

## Detection

- **How was it detected?** (Alert, user report, operator check, or other.)
- **Detection time and alert/report:**
- **Could it have been detected earlier? What signal was missing or noisy?**

## Timeline

Use one timezone consistently. Include observations and actions, not only
decisions. Link evidence where access is controlled.

| Time (timezone) | Event, observation, or action | Result / evidence |
| --- | --- | --- |
| | | |

## Technical analysis

- **Trigger:** What event immediately preceded the impact?
- **Root cause:** Describe the failure mechanism. If unconfirmed, say so.
- **Contributing conditions:** Include deployment/configuration changes,
  dependencies, capacity, network conditions, or gaps in safeguards where
  supported by evidence.
- **Evidence:** Relevant redacted logs, metrics, versions, config keys, and
  reproduction or diagnostic results. Do not paste secrets or full configs.
- **Confidence and open questions:** Separate verified facts from hypotheses.

## Response and recovery

- **Mitigation:** What changed, by whom, and when? Note rejected or reverted
  mitigations when they affect follow-up.
- **Recovery verification:** Record checks performed and their results, such
  as service state, handshake probe, representative proxy traffic, and
  relevant metrics. Do not treat `/healthz` alone as proof of tunnel recovery.
- **Residual impact or risk:**
- **Rollback, route, or DNS cleanup needed:** (If applicable.)

## Follow-up actions

Prefer specific actions that reduce likelihood, impact, or time to detection.
Every action should have one accountable owner and a due date. Link a tracked
issue when one exists; do not record credentials in issue text.

| Action | Owner | Due date | Priority | Tracking link / status |
| --- | --- | --- | --- | --- |
| | | | | |

## Lessons and review

- **What worked well?**
- **Where did response or recovery take longer than expected?**
- **What assumptions or safeguards should change?**
- **Review date and reviewers:**
- **Next review trigger:** (For example, after an action completes or a repeat
  incident.)
