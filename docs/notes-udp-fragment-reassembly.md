# UDP large datagram stream reassembly coverage

## Findings and scope

The production UDP relay is a length-prefixed request carried over the existing
TCP/yamux tunnel. Its request length field is 16 bits, and `read_tunnel_request`
uses `read_exact`, so transport read boundaries must not be treated as UDP
datagram boundaries. TUN's smoltcp stack has IP fragmentation disabled; this
change does not add IP fragmentation or alter the tunnel protocol.

The UDP underlay codec is experimental and has a separate MTU-sized packet
limit. This task exercises the application UDP relay request codec instead.
This narrow test approach follows the bounded framing and explicit packet-size
checks used by shadowsocks-rust's UDP relay, without adding a new protocol or
dependency (reference: https://github.com/shadowsocks/shadowsocks-rust).

## Change and expected result

Added a regression test that sends the maximum 65,535-byte tunnel UDP payload
through a duplex stream with a 31-byte buffer, forcing many partial reads and
writes, then checks authority, priority, and every payload byte after decoding.
This validates reassembly at the existing stream-framing layer; it makes no
throughput claim and is expected to have no runtime impact.

## Verification

`$HOME/.cargo/bin/cargo test --offline -p espejismo-core
protocol::request::tests` passed: 4 passed, 0 failed. This includes the new
maximum-size fragmentation test plus wire-format and oversize-rejection tests.

`$HOME/.cargo/bin/cargo test --offline --workspace` compiled all workspace
crates and passed the client (46), core (192 passed, 1 ignored), core
integration tests (6), server (43 passed, 1 ignored), and yamux unit tests (45).
The final `tokio-yamux` integration test `one_way_bulk_transfer_exceeding_window`
could not run because the sandbox returned `PermissionDenied` while binding its
loopback socket. Therefore the workspace command exited unsuccessfully due to
the environment restriction; this is not a passing full-suite result. No
performance measurement applies because this is test-only correctness coverage.
