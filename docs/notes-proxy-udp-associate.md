# SOCKS5 UDP ASSOCIATE boundary coverage

## Findings and approach

The local SOCKS5 ingress already recognizes `UDP ASSOCIATE`, returns the
family-specific bound address, and parses/encodes UDP datagrams. The request
parser read the SOCKS request reserved byte but discarded it, allowing malformed
requests to proceed. Following the explicit field validation used by mature
SOCKS implementations such as shadowsocks-rust's UDP relay codec, this change
requires the reserved byte to be zero and sends the protocol's general failure
reply otherwise. This keeps the existing application-level relay over the
authenticated TCP tunnel and does not change the transport or product position.

## Changes and expected effect

- Reject SOCKS5 requests with a nonzero reserved byte before dispatching the
  command; valid CONNECT and UDP ASSOCIATE behavior is unchanged.
- Add tests for an unspecified IPv4 UDP ASSOCIATE client endpoint, IPv4 and IPv6
  relay-bound replies, and the malformed reserved-byte reply.
- Expected improvement: malformed request rejection becomes deterministic and
  explicit. No performance change is expected.

## Verification

`$HOME/.cargo/bin/cargo test -p espejismo-core`: 167 passed, 0 failed. The new
cases exercise valid UDP ASSOCIATE dispatch with the unspecified client
endpoint, both address-family reply encodings, and malformed RSV rejection;
existing UDP packet codec and bounded-random parser robustness tests also pass.
