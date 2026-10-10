//! Wire-level framing, requests, replay defense, puzzles, and UDP datagrams.
//!
//! These primitives define Espejismo's encrypted tunnel protocol; transport
//! adapters carry them without changing their meaning.

pub mod framing;
pub mod puzzle;
pub mod replay;
pub mod request;
pub mod udp;
