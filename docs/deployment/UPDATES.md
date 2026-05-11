# Update Checks

Both binaries can check release metadata and print a human-readable update
notice. In `v0.0.2`, this is an explicit check command rather than an automatic
self-replacing updater.

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
  "tag_name": "v0.1.0",
  "html_url": "https://example/releases/v0.1.0"
}
```

`latest_version`, `version`, or `tag_name` may carry the version. The command
does not replace binaries automatically; it only reports availability and the
release URL so package managers, service managers, or deployment scripts can
decide how to roll forward.
