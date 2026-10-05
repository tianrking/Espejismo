# Resource planning documentation

## Findings and approach

Reviewed `docs/POSITIONING.md`, `docs/research/REFERENCES.md`, architecture and
configuration documentation, and the config defaults/runtime paths. Resource
controls already exist: client lanes (default 4 maximum), 1 MiB tunnel buffer,
server global limits (256 streams and 1024 physical connections), a 60-second
replay TTL, tarpit capacity (1024), and native mux flow-control/queue settings.
The 8 MiB native initial window is credit rather than a preallocated buffer.
Adaptive throughput can raise the tunnel buffer as high as 32 MiB. Per-stream
memory and kernel socket buffers have no stable code-level byte bound.

Added `docs/deployment/RESOURCES.md` to describe memory budget calculations,
CPU cost centers, descriptor scaling, and practical sizing. Estimates are
explicitly separated from RSS guarantees; this prevents operators from treating
buffer capacities or flow-control credit as exact resident memory. The advice
preserves the documented authenticated-encrypted-chaos design and single
configuration operational model.

## Expected impact and validation

This is documentation only. It adds no runtime allocation or CPU work, so
expected performance/resource-use change is 0%; correctness behavior is
unchanged. No benchmark or runtime tests are applicable. Values and formulas
were cross-checked against current config defaults and the server/client setup
paths. The estimates intentionally omit allocator, task, socket-kernel, and
workload-dependent costs, so operators should measure the target deployment.
