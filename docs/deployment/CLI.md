# CLI Reference

Espejismo ships two native binaries:

- `espejismo-local`: local SOCKS5 and HTTP proxy.
- `espejismo-remote`: remote authenticated tunnel endpoint.

Both binaries accept TOML config from a file or a one-line base64 string:

```bash
espejismo-local --config espejismo.toml
espejismo-remote --config espejismo.toml

espejismo-local --config-base64 "$CONFIG_B64"
espejismo-remote --config-base64 "$CONFIG_B64"
```

## Config Conversion

Convert a selected TOML config into a portable one-line string:

```bash
espejismo-local --config espejismo.toml --print-config-base64
espejismo-remote --config espejismo.toml --print-config-base64
```

Decode it back to TOML:

```bash
espejismo-local --decode-config-base64 "$CONFIG_B64" > espejismo.toml
espejismo-remote --decode-config-base64 "$CONFIG_B64" > espejismo.toml
```

Print starter config:

```bash
espejismo-local --print-example-config
espejismo-local --print-example-config-base64
```

## Client Profiles

Export a local-client import URL:

```bash
espejismo-local --config espejismo.toml --print-client-profile --profile-name laptop
```

Import that URL:

```bash
espejismo-local --import-profile "espejismo://import/..." --socks5-listen 127.0.0.1:6680
```

## Running

Remote:

```bash
espejismo-remote --config espejismo.toml
```

Local:

```bash
espejismo-local --config espejismo.toml
```

Common direct overrides:

```bash
espejismo-remote --listen 0.0.0.0:6690 --psk "change-me-long-random-secret"

espejismo-local \
  --server remote.example.com:6690 \
  --socks5-listen 127.0.0.1:6680 \
  --http-listen 127.0.0.1:6681 \
  --psk "change-me-long-random-secret"
```

## Admin And Updates

Enable admin with `[admin]` in config or CLI overrides:

```bash
espejismo-remote --config espejismo.toml --admin-listen 127.0.0.1:9090 --admin-token "token"
```

Check release metadata:

```bash
espejismo-local --check-update
espejismo-remote --check-update
```

Use a custom update metadata endpoint:

```bash
espejismo-local --check-update --update-url https://updates.example/espejismo/latest.json
```
