# Admin timeout and concurrency tests

## Analysis and change

The admin endpoint already bounded incomplete header and body reads to 15
seconds, but an authenticated `/reload` or `/apply` callback could remain
pending forever. Accepted connections also each spawned an unconstrained task,
so clients that stalled during request parsing could grow task and socket use
without bound.

Added a 30-second timeout around each admin action. Expired commands receive
HTTP 504. The listener now admits at most 32 concurrent client handlers and
returns HTTP 503 immediately when full; this cap applies to the whole request
lifetime, including slow header/body reads. These limits affect only the local
management plane and leave proxy traffic and protocol behavior unchanged.

Expected improvement: stalled management clients and callbacks now have finite
resource lifetimes, and concurrent handler/socket usage is capped at 32. This
is a bounded-resource correctness change, so no throughput claim applies.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo test --offline -p espejismo-core admin::tests`: 9 passed, 0 failed.
- `cargo test --offline -p espejismo-core`: 169 unit tests, 5 integration
  tests, and 1 doctest passed; 0 failed.
- The timeout test confirms that a pending admin action is cancelled at its
  deadline. The admission test holds a one-slot semaphore and confirms the
  excess client receives HTTP 503. Existing admin tests continue to cover
  protected routes, auth rejection before side effects, and response metrics.
- No performance benchmark applies: this change bounds management-plane
  resource use and does not change data-plane behavior.
