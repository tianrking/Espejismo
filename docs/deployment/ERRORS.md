# Error and status reference

Espejismo does not expose a stable application-wide numeric error-code system.
Most CLI and runtime failures are contextual error messages, while proxy
protocols have their own status values and operating systems report their own
I/O errors. Treat message text as diagnostic context, not as a machine-readable
API. For an ordered troubleshooting flow, see [Troubleshooting](TROUBLESHOOTING.md).

## Common startup and connection errors

| What you see | Likely layer | Checks and next step |
| --- | --- | --- |
| `cannot resolve`, `resolved no addresses`, or `no addresses` | DNS / `local.server` or destination name | Check spelling, resolver configuration, and whether the name resolves on the host running that binary. Use `--doctor` for local server checks. |
| `cannot bind` / `Address already in use` | Local listener or remote listener | Check the configured address and port, whether another process owns it, and whether the service user has permission to bind. Run `--check-config`. |
| `Connection refused` | TCP listener / firewall path | Confirm the remote service is running and listening on the configured interface and port; then check host and cloud firewall rules. |
| `Connection timed out` or handshake probe timeout | Network path or peer not responding | Check routing/firewall and remote logs. Run `--probe-server` to separate TCP/handshake reachability from local proxy listener setup. |
| `handshake ... failed`, `authentication failed`, or handshake rejected | Peer authentication / protocol settings | Compare the PSK or user credentials, shared protocol settings, and system clocks if handshake windows are enabled. Do not paste secrets into logs or support requests. |
| `unsupported ... version`, `capability`, or protocol error | Peer version or malformed/incompatible traffic | Confirm both binaries are the intended Espejismo versions and that both sides have compatible settings. See [Protocol](../PROTOCOL.md). |
| `egress target ... blocked` / `not in ... allow_hosts` | Remote egress policy | Review `remote.egress` host, port, and private-IP rules. A policy rejection is intentional until the policy is changed. |
| `user quota exceeded` | Per-user server limit | Review the matching `remote.users` limits and current usage. |
| `Permission denied` while opening a file, binding, or changing routes | OS permissions / service account | Identify the named resource and run with the documented service privileges; avoid running the whole service as root unless required by the deployment. |

These phrases are examples from current diagnostics, not exhaustive or stable
identifiers. The underlying OS may use different wording for the same failure.

## Proxy status values

### SOCKS5

| Value | Meaning in Espejismo | Operator action |
| --- | --- | --- |
| `0x00` | Local SOCKS5 request accepted | This confirms request parsing, not remote destination reachability. If the tunnel later closes, check client/server logs and remote egress. |
| `0x07` | Command or request version is unsupported | Use a SOCKS5 CONNECT client and check that the application is configured for the SOCKS5 listener. |
| `0x08` | Address type is unsupported | Retry with IPv4, IPv6, or a domain name supported by SOCKS5. |
| `0xff` (method negotiation) | No offered authentication method is acceptable | Match the client's offered methods with the listener's proxy-auth configuration. |
| `0x01` (username/password sub-negotiation) | Proxy username/password authentication failed | Check the local proxy credentials; this is separate from the tunnel PSK. |

The client supports both SOCKS5 CONNECT and UDP ASSOCIATE. CONNECT requests
receive `0x00` after local parsing, before remote egress is known; UDP
ASSOCIATE receives its success reply after the local relay is established.

### HTTP proxy

| Status | Meaning | Operator action |
| --- | --- | --- |
| `200 Connection Established` | HTTP CONNECT request accepted by the local proxy | Like SOCKS5 `0x00`, this is a local acceptance response, not proof that a remote destination will remain reachable. |
| `407 Proxy Authentication Required` | Configured local HTTP proxy authentication did not match | Check the local proxy username/password, independently of tunnel authentication. |

Other malformed or unsupported HTTP proxy requests may close with a diagnostic
in the local process log; there is no comprehensive HTTP error-status mapping.

## Where to look next

- Validate a config with `espejismo-local --check-config` or
  `espejismo-remote --check-config` on the corresponding host.
- Run `espejismo-local --doctor` for deployment checks and
  `espejismo-local --probe-server` for a TCP plus authenticated-handshake probe.
- Check recent service logs on both peers. See [Logging](LOGGING.md) for levels
  and event meanings, and [Troubleshooting](TROUBLESHOOTING.md) for systemd
  commands and symptom-based checks.
- Keep PSKs, proxy credentials, admin tokens, and unsanitized config dumps out
  of shared logs and bug reports.
