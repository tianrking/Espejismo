# On-Call Response

Use this guide to move from an alert or user report to a verified recovery.
It assumes the operator manages the process with systemd; adapt service and
host commands for Docker or another process manager. Espejismo does not
provide an alert dispatcher or define deployment-wide severity targets.

## 1. Confirm impact and scope

Record when the issue began, affected users and hosts, whether the local or
remote side is affected, the last known-good version, and any recent deploy,
config, credential, firewall, DNS, or route changes. Check whether one peer or
all users are affected. Preserve the alert and a short log window for later
review.

Do not include PSKs, admin tokens, proxy credentials, or unredacted configs in
incident notes or support reports. Restrict access to collected logs and
configuration snapshots according to your secret-handling policy.

## 2. Check process and tunnel health

On the affected host, check the service and recent logs. If enabled, check the
admin endpoint and authenticated runtime state. On a client host, run the
diagnostics and handshake probe:

```bash
sudo systemctl --no-pager --full status espejismo-remote
sudo journalctl -u espejismo-remote -n 100 --no-pager
espejismo-local --config /etc/espejismo/espejismo.toml --doctor
espejismo-local --config /etc/espejismo/espejismo.toml --probe-server
```

Use the matching binary's `--check-config` on each side when startup or config
is suspect. `/healthz` confirms only that the admin HTTP handler answers; it
does not establish peer connectivity or working proxy traffic. `/probe-server`
checks reachability and completes the Espejismo handshake, but does not test a
local proxy listener. After it succeeds, test one representative SOCKS5 or
HTTP request through the local proxy.

For Prometheus alerts, first distinguish a failed scrape (`up == 0`) from a
missing scrape job. Then inspect handshake and stream failure counters on both
peers; metrics are process-local. See [Monitoring And Alerting](MONITORING-ALERTS.md)
for alert scope and [Admin and Metrics](ADMIN.md) for runtime state.

## 3. Mitigate and recover

Follow the symptom-specific checks in [Troubleshooting](TROUBLESHOOTING.md).
Check listener/firewall reachability, shared credentials and compatible
versions, egress policy, and recent configuration changes as indicated by the
evidence. Prefer the smallest reversible action that addresses the confirmed
cause. Avoid rotating credentials or changing firewall policy speculatively;
coordinate PSK changes across the remote and every affected client.

If a recent release or config change is the likely cause, use the
[rollback procedure](RUNBOOK.md#rollback) and restore a compatible client and
remote pair where required. Validate the restored config before starting.
For suspected host compromise, isolate the host and use
[Disaster Recovery](BACKUP.md#disaster-recovery-procedure); do not trust or
restore secrets from a compromised machine.

For TUN incidents, treat OS routes and DNS as separate state. Inspect them and
use `--tun-route-cleanup` when appropriate; restarting or restoring TOML does
not itself undo route changes. See [TUN mode](TUN.md).

## 4. Verify recovery

After mitigation, confirm service state and clean startup logs on affected
hosts. From an authorized client, complete `--probe-server` and a
representative proxy request. Check authenticated `/status` or `/connections`
and confirm relevant Prometheus scrapes and failure rates return to expected
levels for the deployment. A passing `/healthz` alone is insufficient.

Watch through the operator's normal observation window before closing the
incident. If recovery is incomplete, retain the incident and continue with
the troubleshooting guide rather than repeatedly restarting without a
hypothesis.

## 5. Record and hand off

Record the impact and timeline, evidence collected, changes made, versions
involved, verification performed, and any follow-up action. Redact secrets.
Hand off the current state and next check explicitly if the incident remains
open. Espejismo has no durable application data to replay; host, config, secret,
and network-state recovery details are in [Backup And Recovery](BACKUP.md).
