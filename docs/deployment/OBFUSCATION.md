# Traffic Shaping and Obfuscation

Espejismo does not impersonate TLS, HTTP, HTTP/2, QUIC, or another protocol.
It uses authenticated encryption, masked framing metadata, variable frame
sizes, and optional padding to avoid stable plaintext markers and fixed frame
length metadata. These mechanisms change observable traffic properties; they do
not make a connection invisible or guarantee resistance to traffic analysis.
See [Positioning](../POSITIONING.md) and the [protocol specification](../PROTOCOL.md)
for the security and wire format details.

## Choose a setting

`[shared.obfuscation].profile` controls frame shaping behavior:

| Profile | Behavior and trade-off |
| --- | --- |
| `low_latency` | Smaller chunks and less timing jitter for interactive exchanges; bulk throughput can be lower. |
| `balanced` | General-purpose variable chunks and normal padding behavior. This is the default. |
| `high_entropy` | More timing variation; added jitter can increase latency. |
| `bulk` | Larger chunks and reduced framing overhead; use when bulk throughput matters more than shape variation. |
| `stealth` | Fixed-size encrypted frames, with optional shaped idle padding and timing. This adds bandwidth and latency and can reduce bulk throughput. |

The associated `chunk_policy` accepts `low_latency`, `balanced`, `bulk`,
`stealth`, or `custom`. `min_chunk` and `max_chunk` set bounds for `custom`;
for `bulk`, `max_chunk` is the operator-selected ceiling. `randomize_chunks`
applies to normal frames; stealth frames use `[shared.stealth]` instead. Normal
non-stealth frames carry at most 262127 payload bytes.

Shared transport settings must agree on both peers. Configure the profile and
its related frame settings identically on client and server. Built-in command
line overlays such as `--profile stealth` are distinct from
`shared.obfuscation.profile`; see [Profiles](PROFILES.md) for overlay behavior.

## Stealth configuration example

This explicit TOML example enables fixed-size frames and budgeted idle shaping.
The same shared values belong in both peer configs:

```toml
[shared.obfuscation]
profile = "stealth"
chunk_policy = "stealth"
randomize_chunks = false

[shared.stealth]
frame_size = 4096
frame_size_candidates = [3328, 3584, 4096, 4608]
tick_ms = 20

[shared.stealth_shaper]
enabled = true
mode = "web"
idle_noise = "poisson"
padding_budget_bps = 16384
min_delay_ms = 20
max_delay_ms = 80
idle_max_delay_ms = 1000
```

`frame_size` is used for the stealth handshake wrapper and as the data-frame
fallback. A non-empty candidate list selects a data-frame size deterministically
for each authenticated session. The selected size determines the maximum data
payload that fits in a frame. Invalid sizes are rejected during configuration
validation.

The shaper is optional and disabled by default in ordinary configuration.
`web` increases idle delays over time, `stream` keeps a steadier cadence, and
`custom` uses the configured delay window. `idle_noise` can be `off`, `uniform`,
or `poisson`. `padding_budget_bps` caps idle padding bytes per second; zero
disables idle padding. Real data frames do not consume this budget. The delay
settings and padding can add latency and traffic, and do not rate-limit data.

## Operational guidance

- Start with `balanced` unless a measured workload or an intentional shaping
  requirement calls for another profile.
- Use `stealth` on both peers when fixed-size frames are desired. Apply the
  built-in `stealth` overlay on both ends only if its additional shaper defaults
  are wanted; inspect the resolved configuration.
- Keep stealth frame sizes modest when limiting cover-traffic cost matters.
  Increasing the padding budget can increase idle bandwidth consumption.
- Test real interactive and bulk workloads after changing profiles. Shaping
  can alter latency and throughput, so no profile guarantees a specific result.
- Do not describe these settings as TLS/QUIC camouflage, invisibility, or a
  guarantee against identification. Espejismo deliberately does not imitate
  another protocol.

For field defaults and validation limits, see [Configuration](CONFIG.md). For
the encrypted handshake and frame formats, see [Protocol](../PROTOCOL.md).
