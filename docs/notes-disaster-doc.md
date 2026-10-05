# Disaster recovery documentation

## Findings and scope

`docs/deployment/BACKUP.md` already described recovery after an outage or
compromise, but did not give operators a pre-incident inventory of access,
artifacts, endpoint ownership, or recovery authority. During host loss those
dependencies can be unavailable along with the host itself. Existing recovery
steps remain authoritative for clean-host rebuild, config validation, staged
verification, credential rotation, and TUN route/DNS handling.

## Plan and expected result

- Add a preparation checklist to the disaster recovery procedure covering
  independent recovery records, provider and endpoint access, protected
  backup retrieval, compatible release artifacts, host networking steps, and
  assigned recovery/credential authority.
- Require a clean-host rehearsal and recording observed recovery time and gaps;
  keep secrets and decryption keys out of the inventory.
- Clarify the operations index entry so responders can find the procedure.

Expected benefit: fewer recovery delays caused by missing access or artifacts,
and clearer operator ownership. This is a documentation-only change; no runtime
or performance change is expected or claimed.

## Validation

- Cross-checked preparation items against the existing restore procedure and
  systemd, Docker, TUN, and high-availability deployment guides.
- Reviewed links and Markdown structure in the edited documentation.
- No application tests or throughput benchmark apply to this docs-only change.
