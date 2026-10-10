# CLI Reference

Espejismo ships two native binaries:

- `espejismo-local`: local SOCKS5 and HTTP proxy.
- `espejismo-remote`: remote authenticated tunnel endpoint.

Both binaries accept TOML config from a file or a one-line base64 string:

Process status behavior for successful commands, CLI errors, and runtime
failures is documented in [Process exit codes](EXIT-CODES.md).

Neither binary supports PID files. For process supervision and multiple
instances, use the service manager; see [PID files and multiple instances](PIDFILES.md).

Check the packaged binary version:

```bash
espejismo-local --version
espejismo-remote --version
```

The binaries have no subcommands. Their operations are selected with options;
`--help` lists the same options supported by the installed build. The tables
below cover both service binaries and the optional `espejismo-bench-http`
benchmark helper. Options marked as overrides use the config value when the
option is omitted; detailed config defaults are documented in [CONFIG.md](CONFIG.md).

## `espejismo-local` options

| Option | Purpose |
| --- | --- |
| `--config <PATH>` | Load TOML from a file. |
| `--config-base64 <STRING>` | Load TOML from base64. |
| `--print-example-config` | Print starter TOML and exit. |
| `--print-example-config-base64` | Print starter TOML as base64 and exit. |
| `--profile <NAME>` | Apply a built-in profile before CLI overrides. |
| `--print-config-base64` | Print the selected effective config as base64 and exit. |
| `--print-config` | Print the selected effective config as TOML and exit. |
| `--write-config <PATH>` | Write the selected effective config and exit. |
| `--decode-config-base64 <STRING>` | Decode base64 config to TOML and exit. |
| `--check-config` | Validate config and local prerequisites, then exit. |
| `--doctor` | Run deployment diagnostics and profile advice, then exit. |
| `--probe-server` | Connect to the configured server and complete a handshake, then exit. |
| `--check-update` | Check release metadata and exit. |
| `--update-url <URL>` | Override the release metadata URL used by `--check-update`. |
| `--print-client-profile` | Print a shareable client import URL and exit. |
| `--profile-name <NAME>` | Name embedded in an exported profile (default: `default`). |
| `--import-profile <URL>` | Import settings from an `espejismo://` client profile URL. |
| `--socks5-listen <ADDR>` | Override the local SOCKS5 listener. |
| `--http-listen <ADDR>` | Override the local HTTP proxy listener. |
| `--tun-enabled` | Enable the local TUN interface. |
| `--tun-name <NAME>` | Set the TUN interface name. |
| `--tun-address <IPv4>` | Set the TUN IPv4 address. |
| `--tun-destination <IPv4>` | Set the TUN IPv4 destination or peer address. |
| `--tun-prefix <0..32>` | Set the TUN IPv4 network prefix length. |
| `--tun-mtu <BYTES>` | Set the TUN interface MTU. |
| `--tun-auto-route` | Install system routes through TUN. |
| `--tun-auto-dns` | Configure system DNS to use the TUN DNS servers. |
| `--tun-route-cleanup` | Remove routes and DNS changes left by a previous TUN run, then exit. |
| `--tun-dns <IP[,IP...]>` | Set comma-separated DNS server IP addresses for TUN mode. |
| `--tun-disable-udp` | Disable UDP forwarding in TUN mode. |
| `--tun-udp-timeout-secs <SECONDS>` | Set the idle timeout for TUN UDP flows. |
| `--tun-udp-block-ports <PORT[,PORT...]>` | Block UDP destination ports in TUN mode. |
| `--server <HOST:PORT>` | Override the remote server. |
| `--psk <SECRET>` | Override the pre-shared key; also read from `ESPEJISMO_PSK`. |
| `--clock-skew-secs <SECONDS>` | Override the allowed peer clock difference. |
| `--max-padding <BYTES>` | Set maximum data-frame padding. |
| `--jitter-ms <MS>` | Set maximum random frame delay. |
| `--padding-chance-percent <0..100>` | Set the chance of adding padding. |
| `--backpressure-threshold-ms <MS>` | Set the backpressure detection threshold. |
| `--backpressure-cooldown-ms <MS>` | Set the retry delay after backpressure. |
| `--handshake-padding <BYTES>` | Set maximum handshake padding. |
| `--puzzle-bits <BITS>` | Set proof-of-work puzzle difficulty. |
| `--tunnel-buffer <BYTES>` | Set per-tunnel I/O buffer size. |
| `--tunnel-min-connections <COUNT>` | Set minimum available tunnel connections. |
| `--tunnel-max-connections <COUNT>` | Set maximum concurrent tunnel connections. |
| `--tunnel-interactive-lanes <COUNT>` | Set connections reserved for interactive traffic. |
| `--tunnel-bulk-lanes <COUNT>` | Set connections reserved for bulk traffic. |
| `--log-level <FILTER>` | Set the tracing filter, such as `info` or `espejismo=trace`. |
| `--log-format <FORMAT>` | Select human-readable or `json` logs. |
| `--log-file <PATH>` | Append logs to a file. |
| `--no-log-ansi` | Disable ANSI colors in terminal logs. |
| `--admin-listen <ADDR>` | Bind the local admin API. |
| `--admin-token <TOKEN>` | Set the bearer token required by the admin API. |

## `espejismo-remote` options

| Option | Purpose |
| --- | --- |
| `--config <PATH>` | Load TOML from a file. |
| `--config-base64 <STRING>` | Load TOML from base64. |
| `--print-example-config` | Print starter TOML and exit. |
| `--print-example-config-base64` | Print starter TOML as base64 and exit. |
| `--profile <NAME>` | Apply a built-in profile before CLI overrides. |
| `--print-config-base64` | Print the selected effective config as base64 and exit. |
| `--print-config` | Print the selected effective config as TOML and exit. |
| `--write-config <PATH>` | Write the selected effective config and exit. |
| `--decode-config-base64 <STRING>` | Decode base64 config to TOML and exit. |
| `--check-config` | Validate config and server prerequisites, then exit. |
| `--doctor` | Run deployment diagnostics and profile advice, then exit. |
| `--check-update` | Check release metadata and exit. |
| `--update-url <URL>` | Override the release metadata URL used by `--check-update`. |
| `--listen <ADDR>` | Override the tunnel listener address. |
| `--psk <SECRET>` | Override the pre-shared key; also read from `ESPEJISMO_PSK`. |
| `--clock-skew-secs <SECONDS>` | Override the allowed peer clock difference. |
| `--max-padding <BYTES>` | Set maximum data-frame padding. |
| `--jitter-ms <MS>` | Set maximum random frame delay. |
| `--padding-chance-percent <0..100>` | Set the chance of adding padding. |
| `--backpressure-threshold-ms <MS>` | Set the backpressure detection threshold. |
| `--backpressure-cooldown-ms <MS>` | Set the retry delay after backpressure. |
| `--handshake-timeout-ms <MS>` | Set the handshake deadline. |
| `--reject-delay-ms <MS>` | Set the delay before rejecting an unsuccessful handshake. |
| `--max-handshake-padding <BYTES>` | Set maximum handshake padding. |
| `--replay-window-secs <SECONDS>` | Set how long completed handshakes stay in the replay cache. |
| `--puzzle-bits <BITS>` | Set proof-of-work puzzle difficulty. |
| `--tunnel-buffer <BYTES>` | Set per-tunnel I/O buffer size. |
| `--cold-start-delay-ms <MS>` | Delay service startup after launch. |
| `--tarpit-max <COUNT>` | Set maximum connections held in the tarpit. |
| `--tarpit-hold-secs <SECONDS>` | Set how long tarpit connections are held. |
| `--log-level <FILTER>` | Set the tracing filter, such as `info` or `espejismo=trace`. |
| `--log-format <FORMAT>` | Select human-readable or `json` logs. |
| `--log-file <PATH>` | Append logs to a file. |
| `--no-log-ansi` | Disable ANSI colors in terminal logs. |
| `--admin-listen <ADDR>` | Bind the admin API. |
| `--admin-token <TOKEN>` | Set the bearer token required by the admin API. |

## `espejismo-bench-http` options

This optional binary provides an HTTP source/sink for throughput experiments.

| Option | Purpose |
| --- | --- |
| `--listen <ADDR>` | Benchmark HTTP listener (default: `0.0.0.0:18082`). |
| `--default-download-mib <MIB>` | Default download size when the URL omits a size (default: `256`). |
| `--max-upload-mib <MIB>` | Maximum accepted upload size (default: `4096`). |
| `--chunk-bytes <BYTES>` | Transfer buffer size (default: `262144`; effective range: 1 KiB–1 MiB). |

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
`auto-throughput`, and `server-safe`. Profiles are ordinary config overlays;
explicit CLI options can still override individual fields.

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

Run doctor mode for deployment-oriented checks and low-feature profile advice:

```bash
espejismo-local --config espejismo.toml --doctor
espejismo-remote --config espejismo.toml --doctor
```

Local doctor additionally probes remote TCP reachability, checks TUN IPv4
auto-route requirements when TUN route takeover is requested, validates DNS
inputs, and warns when a config is not aligned with the no-impersonation,
low-feature transport profile.

For stealth profile rollouts, configure `[shared.stealth].frame_size_candidates`
with identical values on both local and remote. The handshake wrapper still
uses `frame_size`, while data frames select one fixed size per authenticated
session from the candidate set.

Probe the remote handshake from the local side:

```bash
espejismo-local --config espejismo.toml --probe-server
```

`--probe-server` opens TCP to `local.server` and completes the Espejismo
handshake. It does not start SOCKS5, HTTP, or TUN listeners.

## Platform Command Recipes

Linux/macOS local proxy mode:

```bash
./bin/espejismo-local --import-profile "espejismo://import/..."
```

Linux/macOS local TUN mode:

```bash
sudo ./bin/espejismo-local \
  --config ./client.toml \
  --tun-enabled \
  --tun-auto-route \
  --tun-auto-dns
```

Linux/macOS remote mode:

```bash
./bin/espejismo-remote --config ./server.toml
```

Windows local proxy mode (PowerShell):

```powershell
.\bin\espejismo-local.exe --import-profile "espejismo://import/..."
```

Windows local profile -> TOML -> TUN mode (PowerShell as Administrator):

```powershell
.\bin\espejismo-local.exe --import-profile "espejismo://import/..." --write-config .\client.toml
.\bin\espejismo-local.exe --config .\client.toml --tun-enabled --tun-auto-route --tun-auto-dns --check-config
.\bin\espejismo-local.exe --config .\client.toml --tun-enabled --tun-auto-route --tun-auto-dns
```

Windows remote mode (PowerShell):

```powershell
.\bin\espejismo-remote.exe --config .\server.toml
```

Windows cleanup after TUN crash:

```powershell
.\bin\espejismo-local.exe --config .\client.toml --tun-route-cleanup
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
sudo espejismo-local --config espejismo.toml --tun-enabled --tun-udp-block-ports 443
sudo espejismo-local --config espejismo.toml --tun-enabled --tun-disable-udp
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

`admin.token` is required when `admin.listen` is not a loopback address.

Check release metadata:

```bash
espejismo-local --check-update
espejismo-remote --check-update
```

Use a custom update metadata endpoint:

```bash
espejismo-local --check-update --update-url https://updates.example/espejismo/latest.json
```
