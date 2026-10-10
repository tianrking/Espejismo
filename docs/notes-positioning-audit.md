# Positioning audit

## Findings and change

Compared `docs/POSITIONING.md` with `docs/PROTOCOL.md`, `docs/ARCHITECTURE.md`,
the README, and the configured underlay implementation. The project has a raw
TCP default and optional real WebSocket and HTTP/2 underlays. The core Espejismo
handshake and encrypted frames do not impersonate those protocols, but the
optional underlays naturally expose their own protocol characteristics.

The positioning page previously said it borrowed no protocol fingerprints and
suggested passive observers could not see stable indicators without qualifying
that statement by layer. This overstated the claim and conflicted with the
documented underlays. Updated it to distinguish the core encrypted protocol
from transport adapters and to state that masking/encryption reduces stable
cleartext features without promising invisibility. Also clarified that the
tunnel does not depend on a UDP underlay; application UDP relay still travels
over the TCP tunnel.

## Expected impact and verification

This is documentation-only; no runtime behavior or performance is changed, so
there is no throughput delta to report. Manual consistency review against the
documents and implementation references above found the revised statements
consistent with the documented behavior. No build or tests were run because no
code changed.
