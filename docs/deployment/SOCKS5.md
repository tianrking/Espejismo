# SOCKS5 Ingress

The local client exposes a SOCKS5 listener for applications on the client
machine. It accepts SOCKS5 `CONNECT` for TCP and `UDP ASSOCIATE` for UDP, then
relays the requested traffic through the encrypted tunnel to the remote
server. It is an ordinary proxy ingress; it does not imitate another protocol
or change Espejismo's transport identity.

## Listener address

Configure the listener under `[local]`:

```toml
[local]
server = "203.0.113.10:6690"
socks5_listen = "127.0.0.1:6680"
```

The default is `127.0.0.1:6680`. Set `socks5_listen = "0.0.0.0:6680"` only
when other machines need access; that exposes the proxy on all local IPv4
interfaces. Prefer a loopback bind for a desktop client. SOCKS5 and HTTP
listeners must use different socket addresses.

The listener is on the client machine, not the remote server. Applications
connect to the configured address, while the remote server's
`[remote.egress]` policy controls where relayed connections may go. See
[Egress Policy](EGRESS.md).

## Local proxy authentication

SOCKS5 uses no authentication by default. Configure `[local.auth]` to require
SOCKS5 username/password authentication; this same credential pair is used by
the local HTTP proxy when enabled:

```toml
[local.auth]
username = "local-user"
password = "replace-with-a-secret"
```

When credentials are configured, the listener requires RFC 1929
username/password authentication. When omitted, it requires no-auth. This
protects access to the local proxy only; it is separate from the PSK that
authenticates the encrypted tunnel. Bind to loopback or protect the network
path when allowing remote clients to reach this listener.

## TCP, UDP, and DNS

TCP uses SOCKS5 `CONNECT`. UDP uses `UDP ASSOCIATE`; the client opens a
loopback-only UDP relay socket for the association. SOCKS UDP fragmentation is
not supported. The UDP association uses the same local proxy authentication
setting as CONNECT.

For hostname requests, configure applications to use remote name resolution
(often shown as `socks5h://`); an application using `socks5://` may resolve the
name locally before sending the request. The remote resolves destination names
for relayed traffic. See [DNS Behavior](DNS.md) for the resolver path and
limits. SOCKS5 domain requests must contain a non-empty UTF-8 name without NUL
bytes for both TCP and UDP requests; malformed TCP names receive reply `0x08`
(address type unsupported), and malformed UDP names are rejected before relay.

UDP relay still passes through the remote egress policy. If an upstream proxy
is configured in `remote.egress`, UDP is available only when that proxy is
SOCKS5 and supports UDP ASSOCIATE; TCP-only proxy chains cannot carry UDP.

## Example client configuration

```toml
[shared]
psk = "replace-with-a-long-random-secret"

[local]
server = "203.0.113.10:6690"
socks5_listen = "127.0.0.1:6680"

[local.auth]
username = "desktop"
password = "replace-with-a-local-proxy-secret"
```

Point the application at `127.0.0.1:6680` and provide the local username and
password if it prompts for proxy authentication. The client and server must
also have matching tunnel credentials. The remote's egress rules, rather than
the local SOCKS listener, determine permitted destinations and ports.
