# CLI Reference

Espejismo ships two native binaries:

- `espejismo-local`: local SOCKS5 and HTTP proxy.
- `espejismo-remote`: remote authenticated tunnel endpoint.

Both binaries accept TOML config from a file or a one-line base64 string:

Check the packaged binary version:

```bash
espejismo-local --version
espejismo-remote --version
```

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

Print or write the effective TOML after config/profile/CLI overrides:

```bash
espejismo-local --config espejismo.toml --server remote.example.com:6690 --print-config
espejismo-local --config espejismo.toml --write-config client.toml
espejismo-remote --config espejismo.toml --listen 0.0.0.0:6690 --write-config server.toml
```

Print starter config:

```bash
espejismo-local --print-example-config
espejismo-local --print-example-config-base64
```

Print a starter config with an official tuning profile applied:

```bash
espejismo-local --profile balanced --print-example-config > espejismo.toml
espejismo-remote --profile server-safe --print-example-config > espejismo-server.toml
```

Available built-in profiles are `fast`, `balanced`, `low-latency`, `stealth`,
and `server-safe`. Profiles are ordinary config overlays; explicit CLI options
can still override individual fields.

## Config Diagnostics

Validate a config before running a long-lived service:

```bash
espejismo-local --config espejismo.toml --check-config
espejismo-remote --config espejismo.toml --check-config
```

The local check verifies `local.server` DNS resolution, listener bindability,
PSK length, admin token exposure, and pacing bounds. The remote check verifies
`remote.listen`, admin bindability, users or fallback PSK, broad egress policy
warnings, SOCKS5 chain DNS, quotas, bandwidth, and shared TCP/pacing options.

## Client Profiles

Export a local-client import URL:

```bash
espejismo-local --config espejismo.toml --print-client-profile --profile-name laptop
```

Import that URL:

```bash
espejismo-local --import-profile "espejismo://import/..." --socks5-listen 127.0.0.1:6680
```

Convert between local TOML and a client profile URL:

```bash
# TOML config -> one-line client import URL
espejismo-local --config client.toml --print-client-profile --profile-name laptop

# client import URL -> TOML config on stdout
espejismo-local --import-profile "espejismo://import/..." --print-config > client.toml

# client import URL -> TOML config file directly
espejismo-local --import-profile "espejismo://import/..." --write-config client.toml
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

Optional native TUN ingress:

```bash
sudo espejismo-local --config espejismo.toml --tun-enabled --tun-name esptun0
sudo espejismo-local --config espejismo.toml --tun-enabled --tun-auto-route --tun-auto-dns
```

Recover TUN routes/DNS after a crash or service-manager stop hook:

```bash
sudo espejismo-local --config espejismo.toml --tun-route-cleanup
sudo espejismo-local --tun-name esptun0 --tun-route-cleanup
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
