use std::cmp;
use std::time::Duration;

use anyhow::{bail, ensure, Result};
use rand::rngs::OsRng;
use rand::Rng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{sleep, Instant};

use crate::crypto::{decrypt, encrypt, length_mask, SessionKeys};

const MAX_FRAME: usize = 64 * 1024;
const DATA_CHUNK: usize = 16 * 1024;
const AEAD_TAG_LEN: usize = 16;
const STEALTH_HEADER_LEN: usize = 3;
pub const DEFAULT_STEALTH_FRAME_SIZE: usize = 4096;
pub const DEFAULT_STEALTH_TICK_MS: u64 = 50;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FrameType {
    Target = 1,
    Data = 2,
    Close = 3,
    Padding = 4,
}

impl TryFrom<u8> for FrameType {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Target),
            2 => Ok(Self::Data),
            3 => Ok(Self::Close),
            4 => Ok(Self::Padding),
            _ => bail!("unknown frame type {value}"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    pub ty: FrameType,
    pub payload: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObfuscationProfile {
    LowLatency,
    #[default]
    Balanced,
    HighEntropy,
    Stealth,
}

impl ObfuscationProfile {
    pub fn is_stealth(self) -> bool {
        self == Self::Stealth
    }
}

#[derive(Clone, Debug)]
pub struct FrameOptions {
    pub max_padding: usize,
    pub jitter_ms: u64,
    pub padding_chance_percent: u8,
    pub backpressure_threshold_ms: u64,
    pub backpressure_cooldown_ms: u64,
    pub obfuscation_profile: ObfuscationProfile,
    pub randomize_chunks: bool,
    pub min_chunk: usize,
    pub max_chunk: usize,
    pub stealth_frame_size: usize,
    pub stealth_tick_ms: u64,
    pub pacing_enabled: bool,
    pub pacing_max_bytes_per_sec: u64,
    pub pacing_burst_bytes: usize,
    pub pacing_min_write_bytes: usize,
    pub heartbeat_secs: u64,
}

impl Default for FrameOptions {
    fn default() -> Self {
        Self {
            max_padding: 64,
            jitter_ms: 0,
            padding_chance_percent: 35,
            backpressure_threshold_ms: 40,
            backpressure_cooldown_ms: 1000,
            obfuscation_profile: ObfuscationProfile::Balanced,
            randomize_chunks: true,
            min_chunk: 1024,
            max_chunk: DATA_CHUNK,
            stealth_frame_size: DEFAULT_STEALTH_FRAME_SIZE,
            stealth_tick_ms: DEFAULT_STEALTH_TICK_MS,
            pacing_enabled: true,
            pacing_max_bytes_per_sec: 0,
            pacing_burst_bytes: 64 * 1024,
            pacing_min_write_bytes: 1024,
            heartbeat_secs: 30,
        }
    }
}

impl FrameOptions {
    pub fn is_stealth(&self) -> bool {
        self.obfuscation_profile.is_stealth()
    }

    pub fn normalized_chunk_bounds(&self) -> (usize, usize) {
        if self.is_stealth() {
            let capacity = self.stealth_payload_capacity().unwrap_or(1);
            return (capacity, capacity);
        }
        if !self.randomize_chunks {
            return (DATA_CHUNK, DATA_CHUNK);
        }
        let requested_min = if self.pacing_enabled {
            self.min_chunk.max(self.pacing_min_write_bytes)
        } else {
            self.min_chunk
        };
        let min = requested_min.clamp(1, DATA_CHUNK);
        let max = self.max_chunk.clamp(min, DATA_CHUNK);
        (min, max)
    }

    pub fn next_chunk_size(&self) -> usize {
        let (min, max) = self.normalized_chunk_bounds();
        if min == max {
            max
        } else {
            rand::thread_rng().gen_range(min..=max)
        }
    }

    pub fn validate_stealth(&self) -> Result<()> {
        ensure!(
            self.stealth_frame_size > AEAD_TAG_LEN + STEALTH_HEADER_LEN,
            "shared.stealth.frame_size must leave room for frame metadata"
        );
        ensure!(
            self.stealth_frame_size <= MAX_FRAME,
            "shared.stealth.frame_size must be <= {MAX_FRAME}"
        );
        ensure!(
            self.stealth_tick_ms > 0,
            "shared.stealth.tick_ms must be greater than 0"
        );
        Ok(())
    }

    pub fn stealth_payload_capacity(&self) -> Result<usize> {
        self.validate_stealth()?;
        Ok(self.stealth_frame_size - AEAD_TAG_LEN - STEALTH_HEADER_LEN)
    }
}

pub struct FrameCodec<S> {
    stream: S,
    tx_seq: u64,
    rx_seq: u64,
    keys: SessionKeys,
    options: FrameOptions,
    padding_disabled_until: Option<Instant>,
    next_pace_at: Option<Instant>,
}

pub struct FrameWriter<W> {
    writer: W,
    tx_seq: u64,
    keys: SessionKeys,
    options: FrameOptions,
    padding_disabled_until: Option<Instant>,
    next_pace_at: Option<Instant>,
}

pub struct FrameReader<R> {
    reader: R,
    rx_seq: u64,
    keys: SessionKeys,
    options: FrameOptions,
}

impl<S> FrameCodec<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    pub fn new(stream: S, keys: SessionKeys, options: FrameOptions) -> Self {
        Self {
            stream,
            tx_seq: 0,
            rx_seq: 0,
            keys,
            options,
            padding_disabled_until: None,
            next_pace_at: None,
        }
    }

    pub async fn send(&mut self, frame: Frame) -> Result<()> {
        if !self.options.is_stealth() && frame.ty != FrameType::Padding {
            self.maybe_send_padding().await?;
        }
        let elapsed = write_one(
            &mut self.stream,
            &self.keys,
            &mut self.tx_seq,
            &self.options,
            &mut self.next_pace_at,
            frame,
        )
        .await?;
        self.observe_backpressure(elapsed);
        Ok(())
    }

    pub fn options(&self) -> &FrameOptions {
        &self.options
    }

    pub async fn recv(&mut self) -> Result<Frame> {
        loop {
            let frame = read_one(
                &mut self.stream,
                &self.keys,
                &mut self.rx_seq,
                &self.options,
            )
            .await?;
            if frame.ty != FrameType::Padding {
                return Ok(frame);
            }
        }
    }

    pub fn into_inner(self) -> S {
        self.stream
    }

    async fn maybe_send_padding(&mut self) -> Result<()> {
        if !should_send_padding(&self.options, self.padding_disabled_until) {
            return Ok(());
        }
        let len = rand::thread_rng().gen_range(1..=self.options.max_padding);
        let payload = random_padding(len);
        let elapsed = write_one(
            &mut self.stream,
            &self.keys,
            &mut self.tx_seq,
            &self.options,
            &mut self.next_pace_at,
            Frame {
                ty: FrameType::Padding,
                payload,
            },
        )
        .await?;
        self.observe_backpressure(elapsed);
        Ok(())
    }

    fn observe_backpressure(&mut self, elapsed: Duration) {
        if let Some(until) = observe_backpressure(&self.options, elapsed) {
            self.padding_disabled_until = Some(until);
        }
    }
}

impl<W> FrameWriter<W>
where
    W: AsyncWrite + Unpin,
{
    pub fn new(writer: W, keys: SessionKeys, options: FrameOptions) -> Self {
        Self {
            writer,
            tx_seq: 0,
            keys,
            options,
            padding_disabled_until: None,
            next_pace_at: None,
        }
    }

    pub async fn send(&mut self, frame: Frame) -> Result<()> {
        if !self.options.is_stealth() && frame.ty != FrameType::Padding {
            self.maybe_send_padding().await?;
        }
        let elapsed = write_one(
            &mut self.writer,
            &self.keys,
            &mut self.tx_seq,
            &self.options,
            &mut self.next_pace_at,
            frame,
        )
        .await?;
        self.observe_backpressure(elapsed);
        Ok(())
    }

    pub fn options(&self) -> &FrameOptions {
        &self.options
    }

    async fn maybe_send_padding(&mut self) -> Result<()> {
        if !should_send_padding(&self.options, self.padding_disabled_until) {
            return Ok(());
        }
        let len = rand::thread_rng().gen_range(1..=self.options.max_padding);
        let elapsed = write_one(
            &mut self.writer,
            &self.keys,
            &mut self.tx_seq,
            &self.options,
            &mut self.next_pace_at,
            Frame {
                ty: FrameType::Padding,
                payload: random_padding(len),
            },
        )
        .await?;
        self.observe_backpressure(elapsed);
        Ok(())
    }

    fn observe_backpressure(&mut self, elapsed: Duration) {
        if let Some(until) = observe_backpressure(&self.options, elapsed) {
            self.padding_disabled_until = Some(until);
        }
    }
}

impl<R> FrameReader<R>
where
    R: AsyncRead + Unpin,
{
    pub fn new(reader: R, keys: SessionKeys, options: FrameOptions) -> Self {
        Self {
            reader,
            rx_seq: 0,
            keys,
            options,
        }
    }

    pub async fn recv(&mut self) -> Result<Frame> {
        loop {
            let frame = read_one(
                &mut self.reader,
                &self.keys,
                &mut self.rx_seq,
                &self.options,
            )
            .await?;
            if frame.ty != FrameType::Padding {
                return Ok(frame);
            }
        }
    }
}

pub async fn send_frame<S>(codec: &mut FrameCodec<S>, ty: FrameType, payload: Vec<u8>) -> Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    codec.send(Frame { ty, payload }).await
}

pub async fn read_frame<S>(codec: &mut FrameCodec<S>) -> Result<Frame>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    codec.recv().await
}

pub async fn copy_encrypted<R, S>(mut reader: R, codec: &mut FrameCodec<S>) -> Result<u64>
where
    R: AsyncRead + Unpin,
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut buf = vec![0_u8; codec.options.normalized_chunk_bounds().1];
    let mut total = 0_u64;
    loop {
        let chunk_size = codec.options.next_chunk_size();
        let n = reader.read(&mut buf[..chunk_size]).await?;
        if n == 0 {
            codec
                .send(Frame {
                    ty: FrameType::Close,
                    payload: Vec::new(),
                })
                .await?;
            return Ok(total);
        }
        total += n as u64;
        codec
            .send(Frame {
                ty: FrameType::Data,
                payload: buf[..n].to_vec(),
            })
            .await?;
    }
}

async fn write_one<S>(
    stream: &mut S,
    keys: &SessionKeys,
    tx_seq: &mut u64,
    options: &FrameOptions,
    next_pace_at: &mut Option<Instant>,
    frame: Frame,
) -> Result<Duration>
where
    S: AsyncWrite + Unpin,
{
    maybe_jitter(options).await;
    if options.is_stealth() {
        return write_stealth_one(stream, keys, tx_seq, options, next_pace_at, frame).await;
    }
    let seq = *tx_seq;
    let mut plain = Vec::with_capacity(frame.payload.len() + 1);
    plain.push(frame.ty as u8);
    plain.extend_from_slice(&frame.payload);
    let encrypted = encrypt(&keys.tx, seq, &keys.nonce_tag, &plain)?;
    *tx_seq += 1;
    if encrypted.len() > MAX_FRAME {
        bail!("encrypted frame too large");
    }
    pace_before_write(options, encrypted.len() + 4, next_pace_at).await;
    let started = Instant::now();
    let masked_len = (encrypted.len() as u32) ^ length_mask(&keys.tx_len_mask, seq)?;
    stream.write_all(&masked_len.to_be_bytes()).await?;
    stream.write_all(&encrypted).await?;
    Ok(started.elapsed())
}

async fn read_one<S>(
    stream: &mut S,
    keys: &SessionKeys,
    rx_seq: &mut u64,
    options: &FrameOptions,
) -> Result<Frame>
where
    S: AsyncRead + Unpin,
{
    if options.is_stealth() {
        return read_stealth_one(stream, keys, rx_seq, options).await;
    }
    let seq = *rx_seq;
    let masked_len = stream.read_u32().await?;
    let len = (masked_len ^ length_mask(&keys.rx_len_mask, seq)?) as usize;
    if len == 0 || len > MAX_FRAME {
        bail!("invalid frame length {len}");
    }
    let mut encrypted = vec![0_u8; len];
    stream.read_exact(&mut encrypted).await?;
    let plain = decrypt(&keys.rx, seq, &keys.nonce_tag, &encrypted)?;
    *rx_seq += 1;
    if plain.is_empty() {
        bail!("empty plaintext frame");
    }
    Ok(Frame {
        ty: FrameType::try_from(plain[0])?,
        payload: plain[1..].to_vec(),
    })
}

async fn write_stealth_one<S>(
    stream: &mut S,
    keys: &SessionKeys,
    tx_seq: &mut u64,
    options: &FrameOptions,
    next_pace_at: &mut Option<Instant>,
    frame: Frame,
) -> Result<Duration>
where
    S: AsyncWrite + Unpin,
{
    let capacity = options.stealth_payload_capacity()?;
    if frame.payload.len() > capacity {
        bail!("stealth frame payload too large");
    }

    let seq = *tx_seq;
    let plain_len = options.stealth_frame_size - AEAD_TAG_LEN;
    let mut plain = vec![0_u8; plain_len];
    OsRng.fill_bytes(&mut plain);
    plain[0] = frame.ty as u8;
    plain[1..3].copy_from_slice(&(frame.payload.len() as u16).to_be_bytes());
    plain[STEALTH_HEADER_LEN..STEALTH_HEADER_LEN + frame.payload.len()]
        .copy_from_slice(&frame.payload);

    let encrypted = encrypt(&keys.tx, seq, &keys.nonce_tag, &plain)?;
    ensure!(
        encrypted.len() == options.stealth_frame_size,
        "stealth encrypted frame size mismatch"
    );
    pace_before_write(options, encrypted.len(), next_pace_at).await;
    let started = Instant::now();
    *tx_seq += 1;
    stream.write_all(&encrypted).await?;
    Ok(started.elapsed())
}

async fn pace_before_write(
    options: &FrameOptions,
    bytes: usize,
    next_pace_at: &mut Option<Instant>,
) {
    if !options.pacing_enabled || options.pacing_max_bytes_per_sec == 0 {
        return;
    }
    let burst = options
        .pacing_burst_bytes
        .max(options.pacing_min_write_bytes)
        .max(1);
    let charge = bytes.saturating_sub(burst);
    if charge == 0 {
        return;
    }
    let delay = Duration::from_secs_f64(charge as f64 / options.pacing_max_bytes_per_sec as f64);
    if delay.is_zero() {
        return;
    }
    let now = Instant::now();
    let base = next_pace_at.filter(|when| *when > now).unwrap_or(now);
    let target = base + delay;
    if target > now {
        sleep(target - now).await;
    }
    *next_pace_at = Some(target);
}

async fn read_stealth_one<S>(
    stream: &mut S,
    keys: &SessionKeys,
    rx_seq: &mut u64,
    options: &FrameOptions,
) -> Result<Frame>
where
    S: AsyncRead + Unpin,
{
    options.validate_stealth()?;
    let seq = *rx_seq;
    let mut encrypted = vec![0_u8; options.stealth_frame_size];
    stream.read_exact(&mut encrypted).await?;
    let plain = decrypt(&keys.rx, seq, &keys.nonce_tag, &encrypted)?;
    *rx_seq += 1;
    if plain.len() < STEALTH_HEADER_LEN {
        bail!("short stealth plaintext frame");
    }
    let payload_len = u16::from_be_bytes([plain[1], plain[2]]) as usize;
    let capacity = options.stealth_payload_capacity()?;
    if payload_len > capacity || STEALTH_HEADER_LEN + payload_len > plain.len() {
        bail!("invalid stealth payload length {payload_len}");
    }
    Ok(Frame {
        ty: FrameType::try_from(plain[0])?,
        payload: plain[STEALTH_HEADER_LEN..STEALTH_HEADER_LEN + payload_len].to_vec(),
    })
}

async fn maybe_jitter(options: &FrameOptions) {
    if options.jitter_ms == 0 {
        return;
    }
    let upper = cmp::max(1, options.jitter_ms);
    let delay = match options.obfuscation_profile {
        ObfuscationProfile::LowLatency => rand::thread_rng().gen_range(0..=cmp::max(1, upper / 4)),
        ObfuscationProfile::Balanced => rand::thread_rng().gen_range(0..=upper),
        ObfuscationProfile::HighEntropy => {
            let first = rand::thread_rng().gen_range(0..=upper);
            let second = rand::thread_rng().gen_range(0..=upper);
            cmp::max(first, second)
        }
        ObfuscationProfile::Stealth => 0,
    };
    if delay > 0 {
        sleep(Duration::from_millis(delay)).await;
    }
}

fn random_padding(len: usize) -> Vec<u8> {
    let mut payload = vec![0_u8; len];
    OsRng.fill_bytes(&mut payload);
    payload
}

fn should_send_padding(options: &FrameOptions, disabled_until: Option<Instant>) -> bool {
    if options.is_stealth() {
        return false;
    }
    if options.max_padding == 0 || options.padding_chance_percent == 0 {
        return false;
    }
    if disabled_until.is_some_and(|until| Instant::now() < until) {
        return false;
    }
    let chance = f64::from(options.padding_chance_percent.min(100)) / 100.0;
    rand::thread_rng().gen_bool(chance)
}

fn observe_backpressure(options: &FrameOptions, elapsed: Duration) -> Option<Instant> {
    if options.backpressure_threshold_ms == 0 || options.backpressure_cooldown_ms == 0 {
        return None;
    }
    if elapsed >= Duration::from_millis(options.backpressure_threshold_ms) {
        return Some(Instant::now() + Duration::from_millis(options.backpressure_cooldown_ms));
    }
    None
}

#[cfg(test)]
mod tests {
    use tokio::io::duplex;

    use super::{Frame, FrameOptions, FrameReader, FrameType, FrameWriter, ObfuscationProfile};
    use crate::crypto::{accept_handshake, connect_handshake, HandshakeConfig};

    #[tokio::test]
    async fn stealth_frames_roundtrip_data_and_ignore_padding() {
        let cfg = HandshakeConfig::new(b"test-secret-that-is-long-enough".to_vec(), 30, 128, 4);
        let (mut client, mut server) = duplex(8192);
        let client_cfg = cfg.clone();
        let server_cfg = cfg.clone();
        let client_task = tokio::spawn(async move {
            let keys = connect_handshake(&mut client, &client_cfg).await?;
            anyhow::Ok((client, keys))
        });
        let server_task = tokio::spawn(async move {
            let keys = accept_handshake(&mut server, &server_cfg).await?;
            anyhow::Ok((server, keys))
        });
        let (client, client_keys) = client_task.await.unwrap().unwrap();
        let (server, server_keys) = server_task.await.unwrap().unwrap();

        let options = FrameOptions {
            obfuscation_profile: ObfuscationProfile::Stealth,
            stealth_frame_size: 512,
            stealth_tick_ms: 10,
            ..FrameOptions::default()
        };
        let mut writer = FrameWriter::new(client, client_keys, options.clone());
        let mut reader = FrameReader::new(server, server_keys, options);

        writer
            .send(Frame {
                ty: FrameType::Padding,
                payload: Vec::new(),
            })
            .await
            .unwrap();
        writer
            .send(Frame {
                ty: FrameType::Data,
                payload: b"hello stealth".to_vec(),
            })
            .await
            .unwrap();

        let frame = reader.recv().await.unwrap();
        assert_eq!(frame.ty, FrameType::Data);
        assert_eq!(frame.payload, b"hello stealth");
    }
}
