# Observability Notes

## Findings and scope

The admin `GET /metrics` endpoint already exports cumulative tunnel byte
counters, so Prometheus can derive throughput with `rate()` without extra
sampling work in the data path. Its role, user, and failure-reason labels were
previously appended without complete Prometheus escaping. A quote, backslash,
or newline in a role or user label could produce invalid exposition.

I reviewed the local reference list and the project's positioning. The yamux
references highlight window and flow-control visibility as useful operational
signals. In this tree, however, UDP reliability/retransmission code has no
production call sites, and live mux window state is not connected to the
shared metrics object. Exporting either as a live metric now would be
misleading. This change therefore fixes the metrics format and documents the
use of existing byte counters; adding transport counters requires wiring the
real production paths first.

## Change and expected result

- Escape backslash, quote, and line feed in every Prometheus label value.
- Document PromQL `rate()` queries for both traffic directions.
- State that retransmission and live mux window occupancy are not currently
  exported.

Expected improvement: label values containing these characters remain a
single valid Prometheus sample instead of breaking scrape parsing. Operators
can derive bytes per second from the existing counters. This does not change
data-path operations or claim a throughput increase; the formatting helper
runs only while rendering a metrics scrape.

## Experiment

- `cargo test -p espejismo-core --offline metrics::tests::prometheus_labels_escape_backslash_quote_and_newline` — passed (1 passed).
- `cargo test -p espejismo-core --offline` — passed (107 passed; doc tests: 0).

This is a scrape-format correctness change, so a throughput benchmark is not
applicable. The renderer runs on metrics scrapes, not tunnel payload writes;
no throughput improvement is claimed.
