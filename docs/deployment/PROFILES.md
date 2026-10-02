# Profiles

## Built-In Config Profiles

Both binaries can apply an official config overlay with `--profile`:

```bash
espejismo-local --profile fast --print-example-config
espejismo-local --profile low-latency --config espejismo.toml
espejismo-remote --profile server-safe --config espejismo.toml
```

Available profiles:

- `balanced`: general-purpose starting point for mixed interactive and bulk
  proxy traffic. It retains the normal padding behavior, enables pacing, and
  uses a moderate tunnel pool. Use this unless measurements or a specific
  operating constraint point to another profile.
- `fast`: throughput-oriented TCP proxying on paths where bulk transfers matter
  more than padding or timing jitter. It uses bulk-sized chunks, an 8 MiB
  tunnel buffer, and four bulk lanes. Choose it for a simpler throughput
  overlay when the more memory-intensive `auto-throughput` settings are not
  needed; verify results on the target path.
- `auto-throughput`: measured, high-bandwidth-delay-product or long-haul bulk
  transfers. It uses maximum normal-frame payloads, at least 16 MiB of tunnel
  buffering and native-mux window, larger TCP socket buffers, a lower HTTP bulk
  threshold, and six bulk lanes. Expect higher memory use, and use benchmark
  results to establish whether the path benefits. It reduces padding and
  timing variation to favor bulk throughput.
- `low-latency`: interactive workloads with small exchanges, such as browsing
  or control requests, especially when queueing and memory use matter more than
  peak bulk throughput. It uses smaller chunks and pacing bursts, TCP_NODELAY,
  and a smaller tunnel pool; large transfers may be slower.
- `stealth`: use when the configured packet shaping pattern is a requirement.
  It selects stealth frames, frame-size candidates, jitter, Poisson idle noise,
  budgeted padding, and more frequent frame-key updates. Shaping can add traffic
  and latency and reduce bulk throughput; this profile is not protocol
  camouflage and makes no invisibility claim.
- `server-safe`: apply on the remote when its egress and resource policy should
  be conservative: deny private IP destinations, allow common web ports only,
  and cap streams, physical connections, and tarpit pressure. This changes
  server policy and may block destinations your clients require; it is not a
  client performance profile.

These are named config overlays, not a mutually exclusive set of full configs.
`fast`, `auto-throughput`, `low-latency`, and `stealth` are intended to be
selected consistently on both peers for shared transport/frame settings.
`server-safe` is a remote-side policy overlay. Existing explicit config values
may affect the final effective settings; inspect the resolved configuration
before comparing profiles. Profile names here refer to command-line overlays;
they are distinct from `shared.obfuscation.profile` values such as `bulk` and
`low_latency`.

For a first selection, use `balanced` for mixed everyday use, `low-latency` for
interactive traffic, `fast` for moderate bulk tuning, `auto-throughput` for
measured high-BDP bulk paths, `stealth` for intentional traffic shaping, and
`server-safe` to apply remote egress/resource limits. See
[`PERFORMANCE.md`](PERFORMANCE.md) for measurement steps and setting trade-offs.

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
