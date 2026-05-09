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
        }
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
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    let now = unix_now()?;
    let mut nonce = [0_u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let max_padding = cfg.max_handshake_padding.min(MAX_HANDSHAKE_PADDING);
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
    let mut hello = Vec::with_capacity(CLIENT_HELLO_TAG_LEN + body.len());
    hello.extend_from_slice(&tag);
    hello.extend_from_slice(&body);
    stream.write_all(&hello).await?;

    let mut reply = [0_u8; SERVER_HELLO_LEN];
    stream
        .read_exact(&mut reply)
        .await
        .context("server handshake failed")?;
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
        &body,
        server_public.as_bytes(),
        server_version,
        server_capabilities,
    )?;
    if expected.ct_eq(&reply[42..]).unwrap_u8() != 1 {
        bail!("server authentication failed");
    }

    let shared = secret.diffie_hellman(&server_public);
    derive_keys(&cfg.psk, shared.as_bytes(), &nonce, b"client")
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

async fn accept_handshake_inner<S>(
    stream: &mut S,
    cfg: &HandshakeConfig,
    replay: Option<Arc<Mutex<ReplayCache>>>,
) -> Result<SessionKeys>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut tag = [0_u8; CLIENT_HELLO_TAG_LEN];
    let mut fixed_body = [0_u8; CLIENT_HELLO_FIXED_BODY_LEN];
    stream
        .read_exact(&mut tag)
        .await
        .context("client handshake tag failed")?;
    stream
        .read_exact(&mut fixed_body)
        .await
        .context("client handshake body failed")?;

    let padding_len = u16::from_be_bytes(fixed_body[82..84].try_into()?) as usize;
    let max_padding = cfg.max_handshake_padding.min(MAX_HANDSHAKE_PADDING);
    if padding_len > max_padding {
        bail!("handshake padding exceeds configured limit");
    }
    let mut padding = vec![0_u8; padding_len];
    if padding_len > 0 {
        stream
            .read_exact(&mut padding)
            .await
            .context("client handshake padding failed")?;
    }

    let mut body = Vec::with_capacity(CLIENT_HELLO_FIXED_BODY_LEN + padding_len);
    body.extend_from_slice(&fixed_body);
    body.extend_from_slice(&padding);
    if !puzzle::verify(&body, cfg.puzzle_difficulty_bits) {
        bail!("client puzzle verification failed");
    }
    let expected = hmac(&cfg.auth_key, &body)?;
    if expected.ct_eq(&tag).unwrap_u8() != 1 {
        bail!("client authentication failed");
    }

    let timestamp = i64::from_be_bytes(fixed_body[..8].try_into()?);
    let client_version = u16::from_be_bytes(fixed_body[64..66].try_into()?);
    let client_capabilities = u64::from_be_bytes(fixed_body[66..74].try_into()?);
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

    let client_public_bytes = slice_32(&fixed_body[32..64])?;
    if let Some(replay) = replay {
        replay
            .lock()
            .await
            .check_and_insert(now, client_public_bytes)?;
    }
    let client_public = PublicKey::from(client_public_bytes);
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = PublicKey::from(&secret);
    let shared = secret.diffie_hellman(&client_public);

    let server_capabilities = DEFAULT_CAPABILITIES & client_capabilities;
    let tag = server_hmac(
        &cfg.auth_key,
        &body,
        public.as_bytes(),
        PROTOCOL_VERSION,
        server_capabilities,
    )?;
    stream.write_all(public.as_bytes()).await?;
    stream.write_all(&PROTOCOL_VERSION.to_be_bytes()).await?;
    stream.write_all(&server_capabilities.to_be_bytes()).await?;
    stream.write_all(&tag).await?;

    derive_keys(&cfg.psk, shared.as_bytes(), &fixed_body[8..32], b"server")
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
    use super::{accept_handshake, connect_handshake, HandshakeConfig};
    use tokio::io::duplex;

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
}
