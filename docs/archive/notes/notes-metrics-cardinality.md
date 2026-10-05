# Metrics Cardinality

## Findings and scope

The Prometheus exporter does not attach peer IP addresses to labels. Its
per-user series were keyed by every distinct user string ever observed, and
failure reasons were sanitized and truncated but still accepted arbitrary
distinct values. Both maps therefore grew for the lifetime of the process.
These labels appear in the admin `/metrics` output as well as the JSON
snapshot's per-user and per-reason collections.

The reference list points to established transport and proxy projects for
operational design, but this issue is about controlling labels at our existing
metrics boundary. The fix retains current aggregate counters and diagnostics
while imposing explicit bounds; it does not change transport behavior or the
project positioning in `docs/POSITIONING.md`.

## Change and expected result

- Cap per-user metric series at 128 and per-failure-reason series at 32.
- Route overflow observations into a stable `other` series, retaining their
  aggregate counts without admitting more label values.
- Add tests that send more distinct values than each limit and verify both
  the map bound and overflow count.

This bounds in-memory metric map entries and corresponding scrape series at
128 user series and 32 reason series per metrics instance, independent of the
number of distinct input values. The fixed aggregate metrics remain unchanged.
There is no data-path performance gain claimed; extra work occurs only when
recording a previously unseen label, and memory use is bounded under hostile
or misconfigured inputs.

## Experiment

- `cargo test -p espejismo-core --offline` — passed (119 unit tests and 1 doc
  test; 0 failed). Both cardinality regression tests passed.
- This is a cardinality and correctness change; a throughput benchmark is not
  applicable and no throughput improvement is claimed.
