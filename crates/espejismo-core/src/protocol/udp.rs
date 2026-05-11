use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Result};

const MAGIC: &[u8; 4] = b"ESPU";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 4 + 1 + 1 + 8 + 4 + 4 + 2;
const MAX_PAYLOAD: usize = 65_507 - HEADER_LEN;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum UdpPacketKind {
    Data = 1,
    Ack = 2,
    Close = 3,
}

impl TryFrom<u8> for UdpPacketKind {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Data),
            2 => Ok(Self::Ack),
            3 => Ok(Self::Close),
            _ => bail!("unknown UDP underlay packet kind {value}"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UdpPacket {
    pub kind: UdpPacketKind,
    pub session_id: u64,
    pub seq: u32,
    pub ack: u32,
    pub payload: Vec<u8>,
}

impl UdpPacket {
    pub fn data(session_id: u64, seq: u32, ack: u32, payload: Vec<u8>) -> Self {
        Self {
            kind: UdpPacketKind::Data,
            session_id,
            seq,
            ack,
            payload,
        }
    }

    pub fn ack(session_id: u64, ack: u32) -> Self {
        Self {
            kind: UdpPacketKind::Ack,
            session_id,
            seq: 0,
            ack,
            payload: Vec::new(),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>> {
        ensure!(
            self.payload.len() <= MAX_PAYLOAD,
            "UDP underlay payload too large"
        );
        let mut out = Vec::with_capacity(HEADER_LEN + self.payload.len());
        out.extend_from_slice(MAGIC);
        out.push(VERSION);
        out.push(self.kind as u8);
        out.extend_from_slice(&self.session_id.to_be_bytes());
        out.extend_from_slice(&self.seq.to_be_bytes());
        out.extend_from_slice(&self.ack.to_be_bytes());
        out.extend_from_slice(&(self.payload.len() as u16).to_be_bytes());
        out.extend_from_slice(&self.payload);
        Ok(out)
    }

    pub fn decode(input: &[u8]) -> Result<Self> {
        ensure!(input.len() >= HEADER_LEN, "UDP underlay packet too short");
        ensure!(&input[..4] == MAGIC, "bad UDP underlay magic");
        ensure!(input[4] == VERSION, "unsupported UDP underlay version");
        let kind = UdpPacketKind::try_from(input[5])?;
        let session_id = u64::from_be_bytes(input[6..14].try_into()?);
        let seq = u32::from_be_bytes(input[14..18].try_into()?);
        let ack = u32::from_be_bytes(input[18..22].try_into()?);
        let payload_len = u16::from_be_bytes(input[22..24].try_into()?) as usize;
        ensure!(
            input.len() == HEADER_LEN + payload_len,
            "UDP underlay packet length mismatch"
        );
        Ok(Self {
            kind,
            session_id,
            seq,
            ack,
            payload: input[HEADER_LEN..].to_vec(),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveredDatagram {
    pub seq: u32,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug)]
struct PendingPacket {
    packet: UdpPacket,
    last_sent: Instant,
}

#[derive(Debug)]
pub struct UdpReliability {
    session_id: u64,
    next_seq: u32,
    highest_contiguous_rx: u32,
    received_out_of_order: BTreeMap<u32, Vec<u8>>,
    pending: BTreeMap<u32, PendingPacket>,
    retransmit_after: Duration,
}

#[derive(Clone, Debug)]
pub struct UdpCongestionController {
    cwnd_bytes: usize,
    ssthresh_bytes: usize,
    inflight_bytes: usize,
    min_cwnd_bytes: usize,
    max_datagram_bytes: usize,
}

impl UdpCongestionController {
    pub fn new(initial_cwnd_bytes: usize, max_datagram_bytes: usize) -> Self {
        let max_datagram_bytes = max_datagram_bytes.max(1);
        let min_cwnd_bytes = max_datagram_bytes * 2;
        Self {
            cwnd_bytes: initial_cwnd_bytes.max(min_cwnd_bytes),
            ssthresh_bytes: usize::MAX / 2,
            inflight_bytes: 0,
            min_cwnd_bytes,
            max_datagram_bytes,
        }
    }

    pub fn can_send(&self, bytes: usize) -> bool {
        self.inflight_bytes.saturating_add(bytes) <= self.cwnd_bytes
    }

    pub fn on_send(&mut self, bytes: usize) {
        self.inflight_bytes = self.inflight_bytes.saturating_add(bytes);
    }

    pub fn on_ack(&mut self, bytes: usize) {
        self.inflight_bytes = self.inflight_bytes.saturating_sub(bytes);
        if self.cwnd_bytes < self.ssthresh_bytes {
            self.cwnd_bytes = self.cwnd_bytes.saturating_add(bytes.max(1));
        } else {
            let increment =
                (self.max_datagram_bytes * bytes.max(1) / self.cwnd_bytes.max(1)).max(1);
            self.cwnd_bytes = self.cwnd_bytes.saturating_add(increment);
        }
    }

    pub fn on_loss(&mut self) {
        self.ssthresh_bytes = (self.cwnd_bytes / 2).max(self.min_cwnd_bytes);
        self.cwnd_bytes = self.ssthresh_bytes;
        self.inflight_bytes = 0;
    }

    pub fn cwnd_bytes(&self) -> usize {
        self.cwnd_bytes
    }

    pub fn inflight_bytes(&self) -> usize {
        self.inflight_bytes
    }
}

impl UdpReliability {
    pub fn new(session_id: u64, retransmit_after: Duration) -> Self {
        Self {
            session_id,
            next_seq: 1,
            highest_contiguous_rx: 0,
            received_out_of_order: BTreeMap::new(),
            pending: BTreeMap::new(),
            retransmit_after,
        }
    }

    pub fn next_data(&mut self, payload: Vec<u8>, now: Instant) -> Result<UdpPacket> {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1).max(1);
        let packet = UdpPacket::data(self.session_id, seq, self.highest_contiguous_rx, payload);
        self.pending.insert(
            seq,
            PendingPacket {
                packet: packet.clone(),
                last_sent: now,
            },
        );
        Ok(packet)
    }

    pub fn handle_packet(
        &mut self,
        packet: UdpPacket,
    ) -> Result<(Vec<DeliveredDatagram>, UdpPacket)> {
        ensure!(
            packet.session_id == self.session_id,
            "UDP underlay session id mismatch"
        );
        self.mark_acked(packet.ack);
        let mut delivered = Vec::new();
        if packet.kind == UdpPacketKind::Data && packet.seq > self.highest_contiguous_rx {
            self.received_out_of_order
                .entry(packet.seq)
                .or_insert(packet.payload);
            while let Some(payload) = self
                .received_out_of_order
                .remove(&(self.highest_contiguous_rx + 1))
            {
                self.highest_contiguous_rx += 1;
                delivered.push(DeliveredDatagram {
                    seq: self.highest_contiguous_rx,
                    payload,
                });
            }
        }
        Ok((
            delivered,
            UdpPacket::ack(self.session_id, self.highest_contiguous_rx),
        ))
    }

    pub fn due_retransmissions(&mut self, now: Instant) -> Vec<UdpPacket> {
        let mut due = Vec::new();
        for pending in self.pending.values_mut() {
            if now.duration_since(pending.last_sent) >= self.retransmit_after {
                pending.last_sent = now;
                due.push(pending.packet.clone());
            }
        }
        due
    }

    pub fn mark_acked(&mut self, ack: u32) {
        let acked: Vec<u32> = self.pending.range(..=ack).map(|(seq, _)| *seq).collect();
        for seq in acked {
            self.pending.remove(&seq);
        }
    }

    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packet_codec_roundtrips() {
        let packet = UdpPacket::data(42, 7, 6, b"hello".to_vec());
        let encoded = packet.encode().unwrap();
        assert_eq!(UdpPacket::decode(&encoded).unwrap(), packet);
    }

    #[test]
    fn retransmits_until_acked() {
        let start = Instant::now();
        let mut tx = UdpReliability::new(7, Duration::from_millis(100));
        let packet = tx.next_data(b"one".to_vec(), start).unwrap();
        assert_eq!(packet.seq, 1);
        assert!(tx
            .due_retransmissions(start + Duration::from_millis(99))
            .is_empty());
        assert_eq!(
            tx.due_retransmissions(start + Duration::from_millis(100)),
            vec![packet.clone()]
        );
        tx.mark_acked(1);
        assert!(tx
            .due_retransmissions(start + Duration::from_secs(1))
            .is_empty());
        assert_eq!(tx.pending_len(), 0);
    }

    #[test]
    fn receiver_returns_cumulative_ack() {
        let start = Instant::now();
        let mut tx = UdpReliability::new(9, Duration::from_secs(1));
        let mut rx = UdpReliability::new(9, Duration::from_secs(1));
        let one = tx.next_data(b"one".to_vec(), start).unwrap();
        let two = tx.next_data(b"two".to_vec(), start).unwrap();

        let (delivered, ack) = rx.handle_packet(two).unwrap();
        assert!(delivered.is_empty());
        assert_eq!(ack.ack, 0);

        let (delivered, ack) = rx.handle_packet(one).unwrap();
        assert_eq!(
            delivered,
            vec![
                DeliveredDatagram {
                    seq: 1,
                    payload: b"one".to_vec()
                },
                DeliveredDatagram {
                    seq: 2,
                    payload: b"two".to_vec()
                }
            ]
        );
        assert_eq!(ack.ack, 2);
    }

    #[test]
    fn congestion_grows_and_halves_on_loss() {
        let mut cc = UdpCongestionController::new(2400, 1200);
        assert!(cc.can_send(1200));
        cc.on_send(1200);
        assert_eq!(cc.inflight_bytes(), 1200);
        cc.on_ack(1200);
        assert_eq!(cc.inflight_bytes(), 0);
        assert!(cc.cwnd_bytes() > 2400);
        let grown = cc.cwnd_bytes();
        cc.on_loss();
        assert!(cc.cwnd_bytes() < grown);
        assert!(cc.cwnd_bytes() >= 2400);
    }
}
