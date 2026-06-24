# Profiles

## Built-In Config Profiles

Both binaries can apply an official config overlay with `--profile`:

```bash
espejismo-local --profile fast --print-example-config
espejismo-local --profile low-latency --config espejismo.toml
espejismo-remote --profile server-safe --config espejismo.toml
```

Available profiles:

- `fast`: minimal padding/jitter, 256 KiB-capable bulk chunks, larger buffers,
  and a wider tunnel pool for throughput-oriented TCP proxying.
- `auto-throughput`: aggressive high-BDP TCP defaults for benchmarked long-haul
  paths: exact maximum normal-frame payloads, larger socket/transport buffers,
  a wider mux window, lower HTTP bulk threshold, and six bulk lanes.
- `balanced`: production default for general proxy use.
- `low-latency`: smaller chunks, TCP_NODELAY, tighter pacing, and fewer lanes
  for interactive requests.
- `stealth`: fixed-size stealth frames, per-session frame-size diversification
  candidates, modest jitter, budgeted idle padding, Poisson idle timing, and
  more frequent frame key updates.
- `server-safe`: conservative remote defaults with private-IP denial, common
  web ports, capped stream/connection limits, and bounded tarpit pressure.

Profiles are plain config overlays. They do not hide secrets and they do not
override explicit CLI flags such as `--server`, `--listen`, or `--psk`.

For cross-border VPS tuning, start with `--profile auto-throughput` on both
local and remote, run `scripts/bench-throughput.sh` for several rounds, and then
copy only the proven knobs into the real deployment config. Keep `stealth` as
the censorship-resistance default when packet shape matters more than raw bulk
throughput.

For stealth deployments, the built-in profile enables:

```toml
[shared.stealth_shaper]
enabled = true
mode = "web"
idle_noise = "poisson"
padding_budget_bps = 16384
min_delay_ms = 20
max_delay_ms = 80
idle_max_delay_ms = 1000
```

Increase `padding_budget_bps` only after measuring the extra traffic cost. Use
`mode = "stream"` for long-lived media-like flows where steadier timing is more
important than idle quietness.

## Client Import Profiles

`espejismo-local` can export and import compact client profiles.

Export from an existing TOML config:

```bash
espejismo-local --config espejismo.toml --print-client-profile --profile-name laptop
```

The output is an `espejismo://import/...` URL containing URL-safe base64 JSON.
It includes the PSK and optional local proxy credentials, so treat the profile
URL as secret key material. Base64 is only an encoding, not encryption.

Import:

```bash
espejismo-local --import-profile 'espejismo://import/...' --socks5-listen 127.0.0.1:6680
```

Import and materialize a normal TOML config:

```bash
espejismo-local --import-profile 'espejismo://import/...' --print-config > client.toml
espejismo-local --import-profile 'espejismo://import/...' --write-config client.toml
```

This makes the profile URL and the local TOML config reversible for the client
settings carried by the profile. You can also adjust local listeners while
materializing:

```bash
espejismo-local \
  --import-profile 'espejismo://import/...' \
  --socks5-listen 127.0.0.1:6680 \
  --http-listen 127.0.0.1:6681 \
  --write-config client.toml
```

Profiles currently carry the local client essentials: profile name, remote
server address, PSK, dynamic handshake-window settings, local proxy listeners,
and optional local proxy auth. Obfuscation settings, including
`profile = "stealth"` and `[shared.stealth]` (`frame_size`,
`frame_size_candidates`, `tick_ms`), remain in TOML/CLI config and are not
embedded in the import URL. Server egress/admin/logging policy also remains in
TOML because those are deployment-side operator settings.
