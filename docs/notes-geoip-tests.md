# GeoIP and IP route boundary tests

## Findings and scope

The repository has no GeoIP database, country matcher, geographic route rules, or
GeoIP configuration. Adding those would create a new routing feature and data
maintenance model, beyond this test task and the project's small operations
model. The closest existing behavior is Linux TUN's remote endpoint protection:
resolved server addresses must become IPv4 host routes so the tunnel does not
capture its own transport connection.

## Change and expected benefit

Extracted the address filtering/deduplication step from Linux endpoint
resolution into a pure helper and added boundaries for duplicate IPv4 answers,
port mismatch, IPv6 answers, and an IPv6-only result. This does not add GeoIP
routing; it makes the existing endpoint route selection deterministic and
regression-tested without binding sockets or invoking system route commands.
Expected performance impact is none; the helper retains the existing linear
deduplication behavior.

## Verification

`cargo test -p espejismo-client --bin espejismo-local route::linux::tests`
passed all 4 Linux route tests. `cargo test -p espejismo-client` passed all 52
client tests (0 failed, 0 ignored). The new cases exercise duplicate IPv4
answers, another port, IPv6 filtering, and an empty IPv4 selection; no loopback
or route command is used.
