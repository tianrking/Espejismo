# Vendored `tokio-yamux`

## Provenance

- Upstream: `tokio-yamux 0.3.18` from crates.io (part of
  [nervosnetwork/tentacle](https://github.com/nervosnetwork/tentacle)).
- Vendored on 2026-09-29 because the crate carries a **deterministic
  writer-stall bug** (see below) that upstream has not fixed as of
  `0.3.20` (verified by diffing 0.3.18 against 0.3.20: the
  `poll_write` / `recv_frames_wake` logic is unchanged there).
- The vendored copy is otherwise byte-identical to the crates.io
  0.3.18 release, except for the fix documented below.

## Root cause (fixed here)

`StreamHandle::poll_write` drained inbound frames with the **non-blocking**
`try_recv_frames()`, which never registers the task's waker on the
stream's `frame_receiver` channel. When `send_window` hit 0, the writer
task parked with only `writeable_wake` set. A `WindowUpdate` arriving
afterwards was delivered by the session into `frame_receiver`, but
**nothing woke the writer task**, so `handle_window_update` never ran and
`send_window` stayed 0 forever: a deterministic deadlock on any one-way
bulk transfer larger than the stream window (e.g. 64 MiB with a 1 MiB
window).

Why non-blocking was there in the first place: 0.3.18 deliberately
avoided the blocking drain in `poll_write` because it would overwrite the
**reader's** waker stored in the single-waker `frame_receiver` channel
(regression test `test_write_side_does_not_overwrite_read_waker`). That
fix assumed the read path would always process window updates — true only
when the application actually reads from the stream.

## The fix (`src/stream.rs`)

- Added `StreamHandle::recv_frames_wake_blocking(cx)`: same reader-wake
  side effects as `recv_frames_wake`, but uses the blocking
  `recv_frames(cx)` so the current task's waker is registered on
  `frame_receiver`.
- `poll_write` now picks the drain based on whether a read task is
  parked (`readable_wake.is_some()`):
  - read task parked → non-blocking drain (read path owns the channel
    waker; it will process the window update and wake the writer via
    `writeable_wake`) — preserves the 0.3.18 reader fix;
  - no read task → blocking drain (writer registers itself, so a later
    window update wakes it directly) — fixes the one-way stall.
- Invariant: the last task to park owns `frame_receiver`'s waker, and in
  every case at least one live task processes inbound window updates.

## Regression tests

- `tests/window_update_deadlock.rs`: one-way 8 MiB bulk transfer over a
  1 MiB window with no read task on the writer side. Stalls forever
  without the fix; completes without it.
- The crate's own unit tests, in particular
  `test_write_side_does_not_overwrite_read_waker` and
  `test_window_update_wakes_write_via_read_path`, still pass.

## Updating

To re-vendor a newer upstream release: copy the new release over this
directory, re-apply the `recv_frames_wake_blocking` change described
above, and run `cargo test -p tokio-yamux` plus the workspace test suite.
