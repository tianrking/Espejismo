# NAT deployment documentation

## Review and plan

The deployment quickstart and FAQ already require a reachable server TCP
listener and mention host/cloud firewall rules, but do not explain consumer
router forwarding, clients behind outbound NAT, or the inbound limitation of
CGNAT. `--probe-server` is the available end-to-end check: it tests TCP
reachability and completes the Espejismo handshake.

Document the two directions separately: a client normally initiates an
outbound TCP connection and needs no inbound mapping; a NAT-hosted remote
server needs an inbound TCP forward to its configured listener. Add practical
notes for fixed LAN addressing, differing external ports, CGNAT, changing
public IPs, hairpin NAT, and IPv6. This is documentation-only; expected
improvement is fewer deployment errors and faster diagnosis, with no runtime,
protocol, or throughput change expected. The guidance preserves the native
TCP tunnel and the project's small operator-managed setup.

## Changes

- Added `docs/deployment/NAT.md` with topology, port-forwarding example,
  limitations, and a verification procedure.
- Linked it from the deployment quickstart, FAQ, and troubleshooting table.

## Validation and outcome

- Reviewed the new advice against `remote.listen`, `local.server`, the TCP
  underlay, `--probe-server` behavior, and existing IPv6 and troubleshooting
  docs.
- Checked the new relative documentation links and Markdown changes by review.
- No build, cargo tests, or network experiment applies to this documentation
  change. Performance gain: not applicable; runtime regression: none expected.
