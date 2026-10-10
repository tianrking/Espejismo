# Update Checks

Both binaries can check release metadata and print a human-readable update
notice. This is an explicit check command; it does not replace binaries or
restart services.

Before replacing either binary, review the [client/server version compatibility
policy](VERSION-COMPATIBILITY.md). Binary release numbers do not by themselves
promise wire-protocol compatibility.

```bash
espejismo-local --check-update
espejismo-remote --check-update
```

By default, the check reads the latest GitHub release metadata for this project.
Operators can use their own metadata endpoint:

```bash
espejismo-local --check-update --update-url https://updates.example/espejismo/latest.json
```

Compatible JSON fields:

```json
{
  "tag_name": "v0.1.5",
  "html_url": "https://example/releases/v0.1.5"
}
```

`latest_version`, `version`, or `tag_name` may carry the version. The command
does not replace binaries automatically; it only reports availability and the
release URL so package managers, service managers, or deployment scripts can
decide how to roll forward.

Numeric dotted tags are compared by component, with trailing zero components
treated as equivalent (`1.2` and `1.2.0`). Tags that are not entirely numeric
use a string inequality fallback. This metadata check does not verify artifact
signatures or perform installation and rollback.
