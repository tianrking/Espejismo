use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::protocol::puzzle;
use crate::protocol::replay::ReplayCache;

type HmacSha256 = Hmac<Sha256>;

const CLIENT_HELLO_TAG_LEN: usize = 32;
const CLIENT_HELLO_FIXED_BODY_LEN: usize = 8 + 24 + 32 + 2 + 8 + 8 + 2;
const PUZZLE_NONCE_RANGE: std::ops::Range<usize> = 74..82;
const SERVER_HELLO_LEN: usize = 32 + 2 + 8 + 32;
const MAX_HANDSHAKE_PADDING: usize = 1024;
const STEALTH_HANDSHAKE_NONCE_LEN: usize = 24;
const VARIABLE_HANDSHAKE_NONCE_LEN: usize = 24;
const VARIABLE_HANDSHAKE_LEN_LEN: usize = 4;
const VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX: usize = 512;
pub const PROTOCOL_VERSION: u16 = 1;
pub const CAP_TCP_CONNECT: u64 = 1 << 0;
pub const CAP_UDP_ASSOCIATE: u64 = 1 << 1;
pub const DEFAULT_CAPABILITIES: u64 = CAP_TCP_CONNECT | CAP_UDP_ASSOCIATE;

#[derive(Clone, Debug)]
pub struct HandshakeConfig {
    pub psk: Vec<u8>,
    pub auth_key: [u8; 32],
    pub clock_skew_secs: i64,
    pub max_handshake_padding: usize,
    pub puzzle_difficulty_bits: u8,
    pub stealth_frame_size: Option<usize>,
}

impl HandshakeConfig {
    pub fn new(
        psk: Vec<u8>,
        clock_skew_secs: i64,
        max_handshake_padding: usize,
        puzzle_difficulty_bits: u8,
    ) -> Self {
        let auth_key = derive_auth_key(&psk);
        Self {
            psk,
            auth_key,
            clock_skew_secs,
            max_handshake_padding,
            puzzle_difficulty_bits,
            stealth_frame_size: None,
        }
    }

    pub fn with_stealth_frame_size(mut self, frame_size: Option<usize>) -> Self {
        self.stealth_frame_size = frame_size;
        self
    }
}

#[derive(Clone)]
pub struct SessionKeys {
    pub tx: XChaCha20Poly1305,
    pub rx: XChaCha20Poly1305,
    pub(crate) tx_len_mask: [u8; 32],
    pub(crate) rx_len_mask: [u8; 32],
    pub(crate) nonce_tag: [u8; 8],
}

pub struct AuthenticatedSession {
    pub keys: SessionKeys,
    pub user: String,
}

#[derive(Clone, Debug)]
pub struct HandshakeUser {
    pub name: String,
    pub config: HandshakeConfig,
}

pub fn parse_psk(input: &str) -> Result<Vec<u8>> {
    let key = if let Some(rest) = input.strip_prefix("hex:") {
        hex::decode(rest).context("invalid hex PSK")?
    } else if let Some(rest) = input.strip_prefix("base64:") {
        base64::engine::general_purpose::STANDARD
            .decode(rest)
            .context("invalid base64 PSK")?
    } else {
        input.as_bytes().to_vec()
    };

    if key.len() < 16 {
        bail!("PSK must be at least 16 bytes");
    }
    Ok(key)
}

pub async fn connect_handshake<S>(stream: &mut S, cfg: &HandshakeConfig) -> Result<SessionKeys>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    if let Some(frame_size) = cfg.stealth_frame_size {
        return connect_stealth_handshake(stream, cfg, frame_size).await;
    }
    connect_plain_handshake(stream, cfg).await
}

async fn connect_plain_handshake<S>(stream: &mut S, cfg: &HandshakeConfig) -> Result<SessionKeys>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let secret = StaticSecret::random_from_rng(OsRng);
    let client_hello = build_client_hello(cfg, &secret, cfg.max_handshake_padding)?;
    let envelope = mask_variable_handshake_envelope(
        &cfg.auth_key,
        b"plain-client",
        &[],
        &client_hello.wire,
        VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX,
    )?;
    stream.write_all(&envelope).await?;

    let reply_payload = read_variable_handshake_envelope(
        stream,
        &cfg.auth_key,
        b"plain-server",
        &client_hello.tag,
        SERVER_HELLO_LEN,
        SERVER_HELLO_LEN + VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX,
    )
    .await
    .context("server handshake failed")?;
    if reply_payload.len() < SERVER_HELLO_LEN {
        bail!("short server handshake reply");
    }
    finish_client_handshake(
        cfg,
        &secret,
        &client_hello.body,
        &client_hello.nonce,
        &reply_payload[..SERVER_HELLO_LEN],
    )
}

async fn connect_stealth_handshake<S>(
    stream: &mut S,
    cfg: &HandshakeConfig,
    frame_size: usize,
) -> Result<SessionKeys>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    validate_stealth_handshake_frame(frame_size)?;
    let secret = StaticSecret::random_from_rng(OsRng);
    let max_padding = stealth_client_padding_cap(cfg, frame_size)?;
    let client_hello = build_client_hello(cfg, &secret, max_padding)?;
    let block = mask_stealth_handshake_block(
        &cfg.auth_key,
        b"client-hello",
        &[],
        &client_hello.wire,
        frame_size,
    )?;
    stream.write_all(&block).await?;

    let mut reply_block = vec![0_u8; frame_size];
    stream
        .read_exact(&mut reply_block)
        .await
        .context("stealth server handshake failed")?;
    let reply_plain = unmask_stealth_handshake_block(
        &cfg.auth_key,
        b"server-hello",
        &client_hello.tag,
        &reply_block,
    )?;
    finish_client_handshake(
        cfg,
        &secret,
        &client_hello.body,
        &client_hello.nonce,
        &reply_plain[..SERVER_HELLO_LEN],
    )
}

pub async fn accept_handshake<S>(stream: &mut S, cfg: &HandshakeConfig) -> Result<SessionKeys>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    accept_handshake_inner(stream, cfg, None).await
}

pub async fn accept_handshake_with_replay<S>(
    stream: &mut S,
    cfg: &HandshakeConfig,
    replay: Arc<Mutex<ReplayCache>>,
) -> Result<SessionKeys>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    accept_handshake_inner(stream, cfg, Some(replay)).await
}

pub async fn accept_handshake_with_users<S>(
    stream: &mut S,
    users: &[HandshakeUser],
    replay: Arc<Mutex<ReplayCache>>,
) -> Result<AuthenticatedSession>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    accept_handshake_users_inner(stream, users, Some(replay)).await
}

async fn accept_handshake_inner<S>(
    stream: &mut S,
    cfg: &HandshakeConfig,
    replay: Option<Arc<Mutex<ReplayCache>>>,
) -> Result<SessionKeys>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let client_hello = if let Some(frame_size) = cfg.stealth_frame_size {
        read_stealth_client_hello(stream, cfg, frame_size).await?
    } else {
        read_plain_client_hello(stream, cfg).await?
    };
    verify_client_hello(cfg, &client_hello, replay).await?;

    let client_public_bytes = slice_32(&client_hello.fixed_body[32..64])?;
    let client_public = PublicKey::from(client_public_bytes);
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    let shared = secret.diffie_hellman(&client_public);

    let client_capabilities = u64::from_be_bytes(client_hello.fixed_body[66..74].try_into()?);
    let server_capabilities = DEFAULT_CAPABILITIES & client_capabilities;
    let tag = server_hmac(
        &cfg.auth_key,
        &client_hello.body,
        public.as_bytes(),
        PROTOCOL_VERSION,
        server_capabilities,
    )?;
    let mut reply = Vec::with_capacity(SERVER_HELLO_LEN);
    reply.extend_from_slice(public.as_bytes());
    reply.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    reply.extend_from_slice(&server_capabilities.to_be_bytes());
    reply.extend_from_slice(&tag);

    if let Some(frame_size) = cfg.stealth_frame_size {
        let block = mask_stealth_handshake_block(
            &cfg.auth_key,
            b"server-hello",
            &client_hello.tag,
            &reply,
            frame_size,
        )?;
        stream.write_all(&block).await?;
    } else {
        let envelope = mask_variable_handshake_envelope(
            &cfg.auth_key,
            b"plain-server",
            &client_hello.tag,
            &reply,
            VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX,
        )?;
        stream.write_all(&envelope).await?;
    }

    derive_keys(
        &cfg.psk,
        shared.as_bytes(),
        &client_hello.fixed_body[8..32],
        b"server",
    )
}

async fn accept_handshake_users_inner<S>(
    stream: &mut S,
    users: &[HandshakeUser],
    replay: Option<Arc<Mutex<ReplayCache>>>,
) -> Result<AuthenticatedSession>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let first = users
        .first()
        .context("at least one handshake user is required")?;
    let client_hello = if let Some(frame_size) = first.config.stealth_frame_size {
        read_stealth_client_hello_for_users(stream, users, frame_size).await?
    } else {
        read_plain_client_hello_for_users(stream, users).await?
    };

    let mut matched = None;
    for user in users {
        if verify_client_hello(&user.config, &client_hello, None)
            .await
            .is_ok()
        {
            matched = Some(user);
            break;
        }
    }
    let user = matched.context("client authentication failed for all configured users")?;
    verify_client_hello(&user.config, &client_hello, replay).await?;

    let client_public_bytes = slice_32(&client_hello.fixed_body[32..64])?;
    let client_public = PublicKey::from(client_public_bytes);
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    let shared = secret.diffie_hellman(&client_public);

    let client_capabilities = u64::from_be_bytes(client_hello.fixed_body[66..74].try_into()?);
    let server_capabilities = DEFAULT_CAPABILITIES & client_capabilities;
    let tag = server_hmac(
        &user.config.auth_key,
        &client_hello.body,
        public.as_bytes(),
        PROTOCOL_VERSION,
        server_capabilities,
    )?;
    let mut reply = Vec::with_capacity(SERVER_HELLO_LEN);
    reply.extend_from_slice(public.as_bytes());
    reply.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    reply.extend_from_slice(&server_capabilities.to_be_bytes());
    reply.extend_from_slice(&tag);

    if let Some(frame_size) = user.config.stealth_frame_size {
        let block = mask_stealth_handshake_block(
            &user.config.auth_key,
            b"server-hello",
            &client_hello.tag,
            &reply,
            frame_size,
        )?;
        stream.write_all(&block).await?;
    } else {
        let envelope = mask_variable_handshake_envelope(
            &user.config.auth_key,
            b"plain-server",
            &client_hello.tag,
            &reply,
            VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX,
        )?;
        stream.write_all(&envelope).await?;
    }

    Ok(AuthenticatedSession {
        keys: derive_keys(
            &user.config.psk,
            shared.as_bytes(),
            &client_hello.fixed_body[8..32],
            b"server",
        )?,
        user: user.name.clone(),
    })
}

struct ClientHelloMaterial {
    tag: [u8; CLIENT_HELLO_TAG_LEN],
    body: Vec<u8>,
    nonce: [u8; 24],
    wire: Vec<u8>,
}

struct ParsedClientHello {
    tag: [u8; CLIENT_HELLO_TAG_LEN],
    fixed_body: [u8; CLIENT_HELLO_FIXED_BODY_LEN],
    body: Vec<u8>,
}

fn build_client_hello(
    cfg: &HandshakeConfig,
    secret: &StaticSecret,
    max_padding: usize,
) -> Result<ClientHelloMaterial> {
    let public = PublicKey::from(secret);
    let now = unix_now()?;
    let mut nonce = [0_u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let max_padding = max_padding.min(MAX_HANDSHAKE_PADDING);
    let padding_len = random_padding_len(max_padding);
    let mut padding = vec![0_u8; padding_len];
    OsRng.fill_bytes(&mut padding);

    let mut body = Vec::with_capacity(CLIENT_HELLO_FIXED_BODY_LEN + padding_len);
    body.extend_from_slice(&now.to_be_bytes());
    body.extend_from_slice(&nonce);
    body.extend_from_slice(public.as_bytes());
    body.extend_from_slice(&PROTOCOL_VERSION.to_be_bytes());
    body.extend_from_slice(&DEFAULT_CAPABILITIES.to_be_bytes());
    body.extend_from_slice(&0_u64.to_be_bytes());
    body.extend_from_slice(&(padding_len as u16).to_be_bytes());
    body.extend_from_slice(&padding);
    let body = puzzle::solve(body, PUZZLE_NONCE_RANGE, cfg.puzzle_difficulty_bits);
    let tag = hmac(&cfg.auth_key, &body)?;
    let mut wire = Vec::with_capacity(CLIENT_HELLO_TAG_LEN + body.len());
    wire.extend_from_slice(&tag);
    wire.extend_from_slice(&body);
    Ok(ClientHelloMaterial {
        tag,
        body,
        nonce,
        wire,
    })
}

fn finish_client_handshake(
    cfg: &HandshakeConfig,
    secret: &StaticSecret,
    body: &[u8],
    nonce: &[u8; 24],
    reply: &[u8],
) -> Result<SessionKeys> {
    let server_public = PublicKey::from(slice_32(&reply[..32])?);
    let server_version = u16::from_be_bytes(reply[32..34].try_into()?);
    let server_capabilities = u64::from_be_bytes(reply[34..42].try_into()?);
    if server_version != PROTOCOL_VERSION {
        bail!("unsupported server protocol version {server_version}");
    }
    if server_capabilities & CAP_TCP_CONNECT == 0 {
        bail!("server does not support TCP CONNECT");
    }
    let expected = server_hmac(
        &cfg.auth_key,
        body,
        server_public.as_bytes(),
        server_version,
        server_capabilities,
    )?;
    if expected.ct_eq(&reply[42..SERVER_HELLO_LEN]).unwrap_u8() != 1 {
        bail!("server authentication failed");
    }

    let shared = secret.diffie_hellman(&server_public);
    derive_keys(&cfg.psk, shared.as_bytes(), nonce, b"client")
}

async fn read_plain_client_hello<S>(
    stream: &mut S,
    cfg: &HandshakeConfig,
) -> Result<ParsedClientHello>
where
    S: AsyncRead + Unpin,
{
    let min_len = plain_client_hello_min_len();
    let max_len = plain_client_hello_max_len(cfg.max_handshake_padding);
    let payload = read_variable_handshake_envelope(
        stream,
        &cfg.auth_key,
        b"plain-client",
        &[],
        min_len,
        max_len,
    )
    .await
    .context("client handshake failed")?;
    parse_plain_client_hello(cfg, &payload)
}

async fn read_plain_client_hello_for_users<S>(
    stream: &mut S,
    users: &[HandshakeUser],
) -> Result<ParsedClientHello>
where
    S: AsyncRead + Unpin,
{
    let mut nonce = [0_u8; VARIABLE_HANDSHAKE_NONCE_LEN];
    let mut masked_len = [0_u8; VARIABLE_HANDSHAKE_LEN_LEN];
    stream
        .read_exact(&mut nonce)
        .await
        .context("client handshake nonce failed")?;
    stream
        .read_exact(&mut masked_len)
        .await
        .context("client handshake length failed")?;

    let mut candidates = Vec::new();
    for (index, user) in users.iter().enumerate() {
        let len = unmask_variable_handshake_len(
            &user.config.auth_key,
            b"plain-client",
            &[],
            &nonce,
            masked_len,
        )?;
        if (plain_client_hello_min_len()
            ..=plain_client_hello_max_len(user.config.max_handshake_padding))
            .contains(&len)
        {
            candidates.push((index, len));
        }
    }

    let (index, payload_len) = match candidates.as_slice() {
        [(index, payload_len)] => (*index, *payload_len),
        [] => bail!("client handshake did not match any configured user"),
        _ => bail!("client handshake length matched multiple users"),
    };

    let mut payload = vec![0_u8; payload_len];
    stream
        .read_exact(&mut payload)
        .await
        .context("client handshake payload failed")?;
    xor_variable_handshake_payload(
        &users[index].config.auth_key,
        b"plain-client",
        &[],
        &nonce,
        &mut payload,
    )?;
    parse_plain_client_hello(&users[index].config, &payload)
}

fn parse_plain_client_hello(cfg: &HandshakeConfig, plain: &[u8]) -> Result<ParsedClientHello> {
    if plain.len() < plain_client_hello_min_len() {
        bail!("short client handshake");
    }
    let tag = slice_32(&plain[..CLIENT_HELLO_TAG_LEN])?;
    let fixed_body: [u8; CLIENT_HELLO_FIXED_BODY_LEN] = plain
        [CLIENT_HELLO_TAG_LEN..CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN]
        .try_into()
        .map_err(|_| anyhow!("expected fixed client hello body"))?;
    let padding_len = u16::from_be_bytes(fixed_body[82..84].try_into()?) as usize;
    if padding_len > cfg.max_handshake_padding.min(MAX_HANDSHAKE_PADDING) {
        bail!("handshake padding exceeds configured user limit");
    }
    let total = CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN + padding_len;
    if total > plain.len() {
        bail!("short client handshake padding");
    }
    let mut body = Vec::with_capacity(CLIENT_HELLO_FIXED_BODY_LEN + padding_len);
    body.extend_from_slice(&fixed_body);
    body.extend_from_slice(&plain[CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN..total]);
    Ok(ParsedClientHello {
        tag,
        fixed_body,
        body,
    })
}

async fn read_stealth_client_hello<S>(
    stream: &mut S,
    cfg: &HandshakeConfig,
    frame_size: usize,
) -> Result<ParsedClientHello>
where
    S: AsyncRead + Unpin,
{
    validate_stealth_handshake_frame(frame_size)?;
    let mut block = vec![0_u8; frame_size];
    stream
        .read_exact(&mut block)
        .await
        .context("stealth client handshake failed")?;
    let plain = unmask_stealth_handshake_block(&cfg.auth_key, b"client-hello", &[], &block)?;
    parse_stealth_client_hello(cfg, &plain, frame_size)
}

async fn read_stealth_client_hello_for_users<S>(
    stream: &mut S,
    users: &[HandshakeUser],
    frame_size: usize,
) -> Result<ParsedClientHello>
where
    S: AsyncRead + Unpin,
{
    validate_stealth_handshake_frame(frame_size)?;
    let mut block = vec![0_u8; frame_size];
    stream
        .read_exact(&mut block)
        .await
        .context("stealth client handshake failed")?;
    for user in users {
        if user.config.stealth_frame_size != Some(frame_size) {
            continue;
        }
        let Ok(plain) =
            unmask_stealth_handshake_block(&user.config.auth_key, b"client-hello", &[], &block)
        else {
            continue;
        };
        if let Ok(parsed) = parse_stealth_client_hello(&user.config, &plain, frame_size) {
            return Ok(parsed);
        }
    }
    bail!("stealth client handshake did not match any configured user")
}

fn plain_client_hello_min_len() -> usize {
    CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN
}

fn plain_client_hello_max_len(max_padding: usize) -> usize {
    plain_client_hello_min_len()
        + max_padding.min(MAX_HANDSHAKE_PADDING)
        + VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX
}

fn parse_stealth_client_hello(
    cfg: &HandshakeConfig,
    plain: &[u8],
    frame_size: usize,
) -> Result<ParsedClientHello> {
    let min_len = CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN;
    if plain.len() < min_len {
        bail!("short stealth client handshake");
    }
    let tag = slice_32(&plain[..CLIENT_HELLO_TAG_LEN])?;
    let fixed_body: [u8; CLIENT_HELLO_FIXED_BODY_LEN] = plain
        [CLIENT_HELLO_TAG_LEN..CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN]
        .try_into()
        .map_err(|_| anyhow!("expected fixed client hello body"))?;
    let padding_len = u16::from_be_bytes(fixed_body[82..84].try_into()?) as usize;
    let max_padding = stealth_client_padding_cap(cfg, frame_size)?;
    if padding_len > max_padding {
        bail!("handshake padding exceeds configured stealth limit");
    }
    let total = CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN + padding_len;
    if total > plain.len() {
        bail!("short stealth client handshake padding");
    }
    let mut body = Vec::with_capacity(CLIENT_HELLO_FIXED_BODY_LEN + padding_len);
    body.extend_from_slice(&fixed_body);
    body.extend_from_slice(&plain[CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN..total]);
    Ok(ParsedClientHello {
        tag,
        fixed_body,
        body,
    })
}

async fn verify_client_hello(
    cfg: &HandshakeConfig,
    client_hello: &ParsedClientHello,
    replay: Option<Arc<Mutex<ReplayCache>>>,
) -> Result<()> {
    if !puzzle::verify(&client_hello.body, cfg.puzzle_difficulty_bits) {
        bail!("client puzzle verification failed");
    }
    let expected = hmac(&cfg.auth_key, &client_hello.body)?;
    if expected.ct_eq(&client_hello.tag).unwrap_u8() != 1 {
        bail!("client authentication failed");
    }

    let timestamp = i64::from_be_bytes(client_hello.fixed_body[..8].try_into()?);
    let client_version = u16::from_be_bytes(client_hello.fixed_body[64..66].try_into()?);
    let client_capabilities = u64::from_be_bytes(client_hello.fixed_body[66..74].try_into()?);
    if client_version != PROTOCOL_VERSION {
        bail!("unsupported client protocol version {client_version}");
    }
    if client_capabilities & CAP_TCP_CONNECT == 0 {
        bail!("client does not support TCP CONNECT");
    }
    let now = unix_now()?;
    if (now - timestamp).abs() > cfg.clock_skew_secs {
        bail!("handshake timestamp outside allowed window");
    }

    let client_public_bytes = slice_32(&client_hello.fixed_body[32..64])?;
    if let Some(replay) = replay {
        replay
            .lock()
            .await
            .check_and_insert(now, client_public_bytes)?;
    }
    Ok(())
}

fn stealth_client_padding_cap(cfg: &HandshakeConfig, frame_size: usize) -> Result<usize> {
    validate_stealth_handshake_frame(frame_size)?;
    let available = frame_size
        .checked_sub(
            STEALTH_HANDSHAKE_NONCE_LEN + CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN,
        )
        .context("shared.stealth.frame_size is too small for client handshake")?;
    Ok(cfg
        .max_handshake_padding
        .min(MAX_HANDSHAKE_PADDING)
        .min(available))
}

fn validate_stealth_handshake_frame(frame_size: usize) -> Result<()> {
    let min_client =
        STEALTH_HANDSHAKE_NONCE_LEN + CLIENT_HELLO_TAG_LEN + CLIENT_HELLO_FIXED_BODY_LEN;
    let min_server = STEALTH_HANDSHAKE_NONCE_LEN + SERVER_HELLO_LEN;
    if frame_size < min_client.max(min_server) {
        bail!("shared.stealth.frame_size is too small for stealth handshake");
    }
    Ok(())
}

fn mask_variable_handshake_envelope(
    key: &[u8; 32],
    label: &[u8],
    context: &[u8],
    clear: &[u8],
    max_extra_padding: usize,
) -> Result<Vec<u8>> {
    let mut nonce = [0_u8; VARIABLE_HANDSHAKE_NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);

    let extra_len = random_padding_len(max_extra_padding);
    let payload_len = clear
        .len()
        .checked_add(extra_len)
        .context("handshake envelope payload length overflow")?;
    let mut payload = vec![0_u8; payload_len];
    payload[..clear.len()].copy_from_slice(clear);
    if extra_len > 0 {
        OsRng.fill_bytes(&mut payload[clear.len()..]);
    }

    let mut masked_len = (payload_len as u32).to_be_bytes();
    xor_variable_handshake_len(key, label, context, &nonce, &mut masked_len)?;
    xor_variable_handshake_payload(key, label, context, &nonce, &mut payload)?;

    let mut envelope = Vec::with_capacity(
        VARIABLE_HANDSHAKE_NONCE_LEN + VARIABLE_HANDSHAKE_LEN_LEN + payload.len(),
    );
    envelope.extend_from_slice(&nonce);
    envelope.extend_from_slice(&masked_len);
    envelope.extend_from_slice(&payload);
    Ok(envelope)
}

async fn read_variable_handshake_envelope<S>(
    stream: &mut S,
    key: &[u8; 32],
    label: &[u8],
    context: &[u8],
    min_payload_len: usize,
    max_payload_len: usize,
) -> Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut nonce = [0_u8; VARIABLE_HANDSHAKE_NONCE_LEN];
    let mut masked_len = [0_u8; VARIABLE_HANDSHAKE_LEN_LEN];
    stream
        .read_exact(&mut nonce)
        .await
        .context("handshake envelope nonce failed")?;
    stream
        .read_exact(&mut masked_len)
        .await
        .context("handshake envelope length failed")?;

    let payload_len = unmask_variable_handshake_len(key, label, context, &nonce, masked_len)?;
    if payload_len < min_payload_len || payload_len > max_payload_len {
        bail!("handshake envelope length outside configured bounds");
    }

    let mut payload = vec![0_u8; payload_len];
    stream
        .read_exact(&mut payload)
        .await
        .context("handshake envelope payload failed")?;
    xor_variable_handshake_payload(key, label, context, &nonce, &mut payload)?;
    Ok(payload)
}

fn unmask_variable_handshake_len(
    key: &[u8; 32],
    label: &[u8],
    context: &[u8],
    nonce: &[u8; VARIABLE_HANDSHAKE_NONCE_LEN],
    mut masked_len: [u8; VARIABLE_HANDSHAKE_LEN_LEN],
) -> Result<usize> {
    xor_variable_handshake_len(key, label, context, nonce, &mut masked_len)?;
    Ok(u32::from_be_bytes(masked_len) as usize)
}

fn xor_variable_handshake_len(
    key: &[u8; 32],
    label: &[u8],
    context: &[u8],
    nonce: &[u8; VARIABLE_HANDSHAKE_NONCE_LEN],
    data: &mut [u8; VARIABLE_HANDSHAKE_LEN_LEN],
) -> Result<()> {
    xor_hmac_stream_with_domain(key, b"handshake-len:", label, nonce, context, data)
}

fn xor_variable_handshake_payload(
    key: &[u8; 32],
    label: &[u8],
    context: &[u8],
    nonce: &[u8; VARIABLE_HANDSHAKE_NONCE_LEN],
    data: &mut [u8],
) -> Result<()> {
    xor_hmac_stream_with_domain(key, b"handshake-payload:", label, nonce, context, data)
}

fn mask_stealth_handshake_block(
    key: &[u8; 32],
    label: &[u8],
    context: &[u8],
    clear: &[u8],
    frame_size: usize,
) -> Result<Vec<u8>> {
    validate_stealth_handshake_frame(frame_size)?;
    if clear.len() + STEALTH_HANDSHAKE_NONCE_LEN > frame_size {
        bail!("stealth handshake payload exceeds frame size");
    }
    let mut nonce = [0_u8; STEALTH_HANDSHAKE_NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let payload_len = frame_size - STEALTH_HANDSHAKE_NONCE_LEN;
    let mut payload = vec![0_u8; payload_len];
    OsRng.fill_bytes(&mut payload);
    payload[..clear.len()].copy_from_slice(clear);
    xor_hmac_stream(key, label, &nonce, context, &mut payload)?;

    let mut block = Vec::with_capacity(frame_size);
    block.extend_from_slice(&nonce);
    block.extend_from_slice(&payload);
    Ok(block)
}

fn unmask_stealth_handshake_block(
    key: &[u8; 32],
    label: &[u8],
    context: &[u8],
    block: &[u8],
) -> Result<Vec<u8>> {
    if block.len() <= STEALTH_HANDSHAKE_NONCE_LEN {
        bail!("short stealth handshake block");
    }
    let nonce: [u8; STEALTH_HANDSHAKE_NONCE_LEN] = block[..STEALTH_HANDSHAKE_NONCE_LEN]
        .try_into()
        .map_err(|_| anyhow!("expected stealth handshake nonce"))?;
    let mut payload = block[STEALTH_HANDSHAKE_NONCE_LEN..].to_vec();
    xor_hmac_stream(key, label, &nonce, context, &mut payload)?;
    Ok(payload)
}

fn xor_hmac_stream(
    key: &[u8; 32],
    label: &[u8],
    nonce: &[u8; STEALTH_HANDSHAKE_NONCE_LEN],
    context: &[u8],
    data: &mut [u8],
) -> Result<()> {
    xor_hmac_stream_with_domain(key, b"stealth-handshake:", label, nonce, context, data)
}

fn xor_hmac_stream_with_domain(
    key: &[u8; 32],
    domain: &[u8],
    label: &[u8],
    nonce: &[u8; STEALTH_HANDSHAKE_NONCE_LEN],
    context: &[u8],
    data: &mut [u8],
) -> Result<()> {
    let mut offset = 0;
    let mut counter = 0_u32;
    while offset < data.len() {
        let mut mac =
            <HmacSha256 as Mac>::new_from_slice(key).map_err(|_| anyhow!("invalid HMAC key"))?;
        mac.update(domain);
        mac.update(label);
        mac.update(nonce);
        mac.update(context);
        mac.update(&counter.to_be_bytes());
        let block = mac.finalize().into_bytes();
        for byte in block {
            if offset == data.len() {
                break;
            }
            data[offset] ^= byte;
            offset += 1;
        }
        counter = counter.wrapping_add(1);
    }
    Ok(())
}

fn derive_keys(psk: &[u8], shared: &[u8; 32], nonce: &[u8], role: &[u8]) -> Result<SessionKeys> {
    let mut salt = Sha256::new();
    salt.update(psk);
    salt.update(nonce);
    let salt = salt.finalize();

    let hk = Hkdf::<Sha256>::new(Some(&salt), shared);
    let mut client_to_server = [0_u8; 32];
    let mut server_to_client = [0_u8; 32];
    let mut client_len_mask = [0_u8; 32];
    let mut server_len_mask = [0_u8; 32];
    let mut nonce_tag = [0_u8; 8];
    hk.expand(b"espejismo v1 client-to-server", &mut client_to_server)
        .map_err(|_| anyhow!("hkdf expansion failed"))?;
    hk.expand(b"espejismo v1 server-to-client", &mut server_to_client)
        .map_err(|_| anyhow!("hkdf expansion failed"))?;
    hk.expand(b"espejismo v1 client-length-mask", &mut client_len_mask)
        .map_err(|_| anyhow!("hkdf expansion failed"))?;
    hk.expand(b"espejismo v1 server-length-mask", &mut server_len_mask)
        .map_err(|_| anyhow!("hkdf expansion failed"))?;
    hk.expand(b"espejismo v1 nonce-tag", &mut nonce_tag)
        .map_err(|_| anyhow!("hkdf expansion failed"))?;

    let (tx, rx, tx_len_mask, rx_len_mask) = if role == b"client" {
        (
            client_to_server,
            server_to_client,
            client_len_mask,
            server_len_mask,
        )
    } else {
        (
            server_to_client,
            client_to_server,
            server_len_mask,
            client_len_mask,
        )
    };

    Ok(SessionKeys {
        tx: XChaCha20Poly1305::new((&tx).into()),
        rx: XChaCha20Poly1305::new((&rx).into()),
        tx_len_mask,
        rx_len_mask,
        nonce_tag,
    })
}

pub(crate) fn encrypt(
    cipher: &XChaCha20Poly1305,
    seq: u64,
    nonce_tag: &[u8; 8],
    plain: &[u8],
) -> Result<Vec<u8>> {
    cipher
        .encrypt(XNonce::from_slice(&session_nonce(seq, nonce_tag)), plain)
        .map_err(|_| anyhow!("encryption failed"))
}

pub(crate) fn decrypt(
    cipher: &XChaCha20Poly1305,
    seq: u64,
    nonce_tag: &[u8; 8],
    encrypted: &[u8],
) -> Result<Vec<u8>> {
    cipher
        .decrypt(
            XNonce::from_slice(&session_nonce(seq, nonce_tag)),
            encrypted,
        )
        .map_err(|_| anyhow!("frame authentication failed"))
}

pub(crate) fn length_mask(key: &[u8; 32], seq: u64) -> Result<u32> {
    let mut mac = <HmacSha256 as Mac>::new_from_slice(key).map_err(|_| anyhow!("invalid key"))?;
    mac.update(b"len:");
    mac.update(&seq.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    Ok(u32::from_be_bytes(digest[..4].try_into()?))
}

fn session_nonce(seq: u64, tag: &[u8; 8]) -> [u8; 24] {
    let mut nonce = [0_u8; 24];
    nonce[..8].copy_from_slice(tag);
    nonce[16..].copy_from_slice(&seq.to_be_bytes());
    nonce
}

fn derive_auth_key(psk: &[u8]) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, psk);
    let mut key = [0_u8; 32];
    hk.expand(b"espejismo v1 handshake-auth-key", &mut key)
        .expect("32 bytes always fits in HKDF-SHA256 output");
    key
}

fn hmac(key: &[u8], input: &[u8]) -> Result<[u8; 32]> {
    let mut mac =
        <HmacSha256 as Mac>::new_from_slice(key).map_err(|_| anyhow!("invalid HMAC key"))?;
    mac.update(input);
    Ok(mac.finalize().into_bytes().into())
}

fn server_hmac(
    key: &[u8],
    client_hello: &[u8],
    server_pub: &[u8],
    version: u16,
    capabilities: u64,
) -> Result<[u8; 32]> {
    let mut data = Vec::with_capacity(client_hello.len() + server_pub.len() + 20);
    data.extend_from_slice(b"server-v1:");
    data.extend_from_slice(client_hello);
    data.extend_from_slice(server_pub);
    data.extend_from_slice(&version.to_be_bytes());
    data.extend_from_slice(&capabilities.to_be_bytes());
    hmac(key, &data)
}

fn random_padding_len(max_padding: usize) -> usize {
    if max_padding == 0 {
        return 0;
    }
    let mut bytes = [0_u8; 2];
    OsRng.fill_bytes(&mut bytes);
    usize::from(u16::from_be_bytes(bytes)) % (max_padding + 1)
}

fn slice_32(input: &[u8]) -> Result<[u8; 32]> {
    input.try_into().map_err(|_| anyhow!("expected 32 bytes"))
}

fn unix_now() -> Result<i64> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock is before UNIX epoch")?
        .as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::{
        accept_handshake, accept_handshake_with_users, connect_handshake, parse_plain_client_hello,
        HandshakeConfig, HandshakeUser, VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX,
    };
    use crate::protocol::replay::ReplayCache;
    use std::sync::Arc;
    use tokio::io::duplex;
    use tokio::sync::Mutex;

    #[tokio::test]
    async fn client_and_server_complete_variable_length_handshake() {
        let cfg = HandshakeConfig::new(b"test-secret-that-is-long-enough".to_vec(), 30, 128, 4);
        let (mut client, mut server) = duplex(4096);
        let client_cfg = cfg.clone();
        let server_cfg = cfg.clone();

        let client_task =
            tokio::spawn(async move { connect_handshake(&mut client, &client_cfg).await });
        let server_task =
            tokio::spawn(async move { accept_handshake(&mut server, &server_cfg).await });

        client_task.await.unwrap().unwrap();
        server_task.await.unwrap().unwrap();
    }

    #[test]
    fn variable_plain_envelope_masks_inner_hello_and_varies_length() {
        let cfg = HandshakeConfig::new(b"test-secret-that-is-long-enough".to_vec(), 30, 128, 0);
        let secret = x25519_dalek::StaticSecret::random_from_rng(rand::rngs::OsRng);
        let hello = super::build_client_hello(&cfg, &secret, cfg.max_handshake_padding).unwrap();

        let envelope_a = super::mask_variable_handshake_envelope(
            &cfg.auth_key,
            b"plain-client",
            &[],
            &hello.wire,
            VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX,
        )
        .unwrap();
        let envelope_b = super::mask_variable_handshake_envelope(
            &cfg.auth_key,
            b"plain-client",
            &[],
            &hello.wire,
            VARIABLE_HANDSHAKE_EXTRA_PADDING_MAX,
        )
        .unwrap();

        assert_ne!(&envelope_a[..32], &hello.wire[..32]);
        assert_ne!(&envelope_a[..32], &envelope_b[..32]);

        let nonce: [u8; 24] = envelope_a[..24].try_into().unwrap();
        let masked_len: [u8; 4] = envelope_a[24..28].try_into().unwrap();
        let len = super::unmask_variable_handshake_len(
            &cfg.auth_key,
            b"plain-client",
            &[],
            &nonce,
            masked_len,
        )
        .unwrap();
        let mut payload = envelope_a[28..28 + len].to_vec();
        super::xor_variable_handshake_payload(
            &cfg.auth_key,
            b"plain-client",
            &[],
            &nonce,
            &mut payload,
        )
        .unwrap();
        parse_plain_client_hello(&cfg, &payload).unwrap();
    }

    #[tokio::test]
    async fn variable_plain_handshake_supports_multiple_users() {
        let good = HandshakeConfig::new(b"good-secret-that-is-long-enough".to_vec(), 30, 128, 2);
        let other = HandshakeConfig::new(b"other-secret-that-is-long-enough".to_vec(), 30, 128, 2);
        let users = vec![
            HandshakeUser {
                name: "other".to_string(),
                config: other,
            },
            HandshakeUser {
                name: "good".to_string(),
                config: good.clone(),
            },
        ];
        let replay = Arc::new(Mutex::new(ReplayCache::new(60)));
        let (mut client, mut server) = duplex(4096);

        let client_task = tokio::spawn(async move { connect_handshake(&mut client, &good).await });
        let server_task =
            tokio::spawn(
                async move { accept_handshake_with_users(&mut server, &users, replay).await },
            );

        client_task.await.unwrap().unwrap();
        let session = server_task.await.unwrap().unwrap();
        assert_eq!(session.user, "good");
    }

    #[tokio::test]
    async fn client_and_server_complete_stealth_handshake() {
        let cfg = HandshakeConfig::new(b"test-secret-that-is-long-enough".to_vec(), 30, 128, 4)
            .with_stealth_frame_size(Some(4096));
        let (mut client, mut server) = duplex(8192);
        let client_cfg = cfg.clone();
        let server_cfg = cfg.clone();

        let client_task =
            tokio::spawn(async move { connect_handshake(&mut client, &client_cfg).await });
        let server_task =
            tokio::spawn(async move { accept_handshake(&mut server, &server_cfg).await });

        client_task.await.unwrap().unwrap();
        server_task.await.unwrap().unwrap();
    }
}
