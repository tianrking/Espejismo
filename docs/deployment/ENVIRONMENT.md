# Environment Variables

Espejismo uses environment variables in three places: the local and remote
binary command lines, the release installers, and the optional throughput
benchmark script. Runtime service settings otherwise belong in the TOML
configuration file.

## Runtime binaries

| Variable | Used by | Purpose |
| --- | --- | --- |
| `ESPEJISMO_PSK` | `espejismo-local`, `espejismo-remote` | Supplies the pre-shared key when `--psk` is not provided. It overrides the TOML key through the CLI argument's normal precedence. Use a secret manager or a protected service environment; avoid putting secrets in shell history. |

For other runtime settings, use the TOML file and CLI options documented in
[Configuration](CONFIG.md) and [CLI](CLI.md). The process also inherits normal
operating-system variables, but Espejismo does not interpret them as
configuration values.

`ESPEJISMO_AUTH_REQUEST` is set by Espejismo when it starts a configured
authentication extension. Its value is a JSON request; it is output to the
child process, not a setting to provide to the service.

## Release installers

`scripts/install.sh` and `scripts/install.ps1` accept the following variables.
The platform variables are optional overrides; otherwise the installer detects
the current OS and architecture.

| Variable | Default | Purpose |
| --- | --- | --- |
| `ESPEJISMO_REPO` | `tianrking/Espejismo` | GitHub repository in `owner/name` form. |
| `ESPEJISMO_VERSION` | `latest` | Release tag, such as `v0.1.5`, or `latest`. |
| `ESPEJISMO_PACKAGE` | `full` | `full` installs client and server; `server` installs only the remote server. |
| `ESPEJISMO_INSTALL_DIR` | `$HOME/.espejismo` (Unix); `$HOME/.espejismo` in Git Bash or the PowerShell user's home equivalent | Directory where the archive is extracted. |
| `ESPEJISMO_ARCHIVE_URL` | unset | Direct archive URL override, useful with a mirror. |
| `ESPEJISMO_OS` | detected | Artifact OS suffix: `linux`, `darwin`, or `windows`. |
| `ESPEJISMO_ARCH` | detected | Artifact architecture suffix, such as `amd64`, `386`, `arm64`, or `armv7`. |

The selected OS and architecture must match a published artifact. The installer
downloads and extracts files; it does not configure a service or firewall. See
[Quickstart](QUICKSTART.md) and [Windows deployment](WINDOWS.md).

## Throughput benchmark script

The optional `scripts/bench-throughput.sh` script uses these variables. URLs
must point to a reachable HTTP test source/sink; admin values are optional and
used only to collect status snapshots.

| Variable | Default | Purpose |
| --- | --- | --- |
| `ESPEJISMO_PROXY_URL` | `http://127.0.0.1:16681` | Local HTTP proxy URL. |
| `ESPEJISMO_DIRECT_DOWNLOAD_URL` | `http://127.0.0.1:18082/256m.bin` | Direct download test URL. |
| `ESPEJISMO_PROXY_DOWNLOAD_URL` | `http://127.0.0.1:18082/256m.bin` | Download URL requested through the proxy. |
| `ESPEJISMO_DIRECT_UPLOAD_URL` | `http://127.0.0.1:18082/upload` | Direct upload test endpoint. |
| `ESPEJISMO_PROXY_UPLOAD_URL` | `http://127.0.0.1:18082/upload` | Upload endpoint requested through the proxy. |
| `ESPEJISMO_UPLOAD_FILE` | `/tmp/espejismo-upload-128m.bin` | Upload payload path; created if missing. |
| `ESPEJISMO_UPLOAD_MIB` | `128` | Payload size in MiB when the script creates it; minimum `1`. |
| `ESPEJISMO_PARALLEL` | `4` | Concurrent transfers; minimum `1`. |
| `ESPEJISMO_ROUNDS` | `1` | Number of rounds; minimum `1`. |
| `ESPEJISMO_ROUND_DELAY_SECS` | `5` | Delay between rounds in seconds; minimum `0`. |
| `ESPEJISMO_CURL_MAX_TIME` | `600` | Maximum seconds allowed for each curl transfer; minimum `1`. |
| `ESPEJISMO_ADMIN_URL` | unset | Optional admin status URL. |
| `ESPEJISMO_ADMIN_TOKEN` | unset | Optional bearer token for the admin URL. Treat it as a secret. |
| `ESPEJISMO_OUTPUT_DIR` | `./bench-results` | Parent directory for benchmark output. |
| `ESPEJISMO_RUN_ID` | Current UTC timestamp (`YYYYMMDDTHHMMSSZ`) | Run directory name under the output directory. |
| `ESPEJISMO_LOCAL_LOG_FILE` | `/tmp/espejismo-local-bench.log` | Local client log file inspected for benchmark log-risk reporting. |
| `ESPEJISMO_LOG_SCAN_LINES` | `300` | Maximum recent log lines to inspect; minimum `1`. |
| `ESPEJISMO_MAX_LOG_LINE_BYTES` | `8192` | Maximum line size used by log-risk inspection; minimum `1`. |
| `ESPEJISMO_ALLOW_VERBOSE_LOGS` | `0` | Set to `1` to allow verbose logs during the benchmark. |
| `ESPEJISMO_PYTHON` | `python3` | Python executable used for aggregate statistics when available. |

Numeric settings are validated by the script. See
[Throughput tuning](PERFORMANCE.md) for test setup and interpretation.
