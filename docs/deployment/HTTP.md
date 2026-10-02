# HTTP Proxy Ingress

The local client can expose an HTTP proxy for applications on the client
machine. It accepts HTTP `CONNECT` requests and absolute-form `http://` requests,
then forwards the connection through the encrypted tunnel to the remote server.
This is an application proxy ingress; it does not make the tunnel itself speak
HTTP or imitate an HTTP service.

## Listener address

Configure the listener under `[local]`:

```toml
[local]
server = "203.0.113.10:6690"
http_listen = "127.0.0.1:6681"
```

The default is `127.0.0.1:6681`. A loopback bind is suitable for applications
on the same machine. Setting `http_listen = "0.0.0.0:6681"` exposes it on all
local IPv4 interfaces; use that only when other machines need access. The
HTTP and SOCKS5 listeners must use different socket addresses. The remote
server's `[remote.egress]` policy still controls destination addresses and
ports; see [Egress Policy](EGRESS.md).

## Supported request forms

For HTTPS destinations, applications normally send `CONNECT host:443`; the
proxy replies that the tunnel is established, then relays the encrypted TLS
connection unchanged. For plain HTTP, applications send an absolute URL such
as `GET http://example.com/path HTTP/1.1`. The client converts it to origin
form before forwarding it (`GET /path HTTP/1.1`) and removes
`Proxy-Authorization` from the forwarded request.

Other request-target forms are not accepted for ordinary requests. The proxy
does not terminate TLS or inspect HTTPS contents. Destination resolution and
egress filtering happen on the remote side as part of opening the tunneled
connection.

The client reads request headers incrementally, with a 32 KiB maximum and a
15-second header-read timeout. An incomplete, invalid, oversized, or timed-out
header closes the proxy request with an error. Request bodies with a valid
`Content-Length` are copied for that declared length; requests without a
usable length use the configured idle-copy behavior.

## Local proxy authentication

HTTP proxy authentication is disabled by default. Add `[local.auth]` to require
HTTP Basic proxy credentials:

```toml
[local.auth]
username = "desktop"
password = "replace-with-a-local-proxy-secret"
```

With authentication enabled, requests must include a valid
`Proxy-Authorization: Basic ...` header. Missing or invalid credentials receive
`407 Proxy Authentication Required`. These credentials protect access to the
local proxy; they are separate from the PSK that authenticates the encrypted
tunnel. Basic credentials are not protected on the local hop by this proxy, so
keep the listener on loopback or protect any exposed network path.

## Bulk lane selection

`local.http_bulk_threshold_bytes` defaults to `1048576` (1 MiB). A request with
a `Content-Length` at or above this threshold is assigned to a bulk lane. Plain
HTTP `GET` requests whose path ends in a recognized download suffix (`.bin`,
`.zip`, `.tar.gz`, `.mp4`, `.iso`, and other common archive, package, and media
suffixes, case-insensitive) also use a bulk lane; this covers common downloads
that have no request body length. Other requests, including `CONNECT`, use
interactive lanes. Set the threshold to `0` to disable the upload-size rule;
download-path classification remains active.

The lane choice only affects scheduling within the client's tunnel pool. It
does not change HTTP semantics or the remote egress policy. See
[Configuration](CONFIG.md#local) for the field reference and
[Traffic Priority and QoS](QOS.md) for all stream classes and lane behavior,
and [Performance Tuning](PERFORMANCE.md) for pool guidance.

## Example

```toml
[shared]
psk = "replace-with-a-long-random-secret"

[local]
server = "203.0.113.10:6690"
http_listen = "127.0.0.1:6681"
http_bulk_threshold_bytes = 1048576

[local.auth]
username = "desktop"
password = "replace-with-a-local-proxy-secret"
```

Configure the application to use `127.0.0.1:6681` as its HTTP proxy and supply
the local proxy credentials when requested. The remote must have a matching
tunnel credential; remote egress rules determine which destinations are
reachable.
