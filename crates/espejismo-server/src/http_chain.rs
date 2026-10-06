use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result};
use base64::Engine;
use espejismo_core::{EgressProxy, EgressProxyKind, TransportStream};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

const MAX_HTTP_CONNECT_RESPONSE: usize = 16 * 1024;
const HTTPS_PROXY_TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

// Rustls keeps TLS session tickets in ClientConfig's resumption store. Reuse
// one config so separate CONNECT tunnels to the same proxy can resume TLS.
static HTTPS_PROXY_TLS_CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();

pub(crate) async fn connect_via_http_proxy(
    proxy: &EgressProxy,
    authority: &str,
) -> Result<Box<dyn TransportStream>> {
    let mut stream = TcpStream::connect(&proxy.endpoint)
        .await
        .with_context(|| format!("connect HTTP proxy {}", proxy.endpoint))?;
    let request = build_connect_request(proxy, authority)?;
    match proxy.kind {
        EgressProxyKind::Http => {
            stream.write_all(request.as_bytes()).await?;
            read_connect_response(&mut stream).await?;
            Ok(Box::new(stream))
        }
        EgressProxyKind::Https => {
            let (host, _) = espejismo_core::split_authority(&proxy.endpoint)
                .context("HTTPS proxy endpoint must be host:port")?;
            let mut tls = connect_tls_to_proxy(stream, &host).await?;
            tls.write_all(request.as_bytes()).await?;
            read_connect_response(&mut tls).await?;
            Ok(Box::new(tls))
        }
        _ => anyhow::bail!("invalid proxy kind for HTTP CONNECT"),
    }
}

fn build_connect_request(proxy: &EgressProxy, authority: &str) -> Result<String> {
    espejismo_core::split_authority(authority)?;
    let mut request = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: Keep-Alive\r\n"
    );
    if let Some(username) = &proxy.username {
        let password = proxy.password.as_deref().unwrap_or("");
        let token =
            base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
        request.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    request.push_str("\r\n");
    Ok(request)
}

async fn connect_tls_to_proxy(
    stream: TcpStream,
    host: &str,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>> {
    connect_tls_to_proxy_with_timeout(stream, host, HTTPS_PROXY_TLS_HANDSHAKE_TIMEOUT).await
}

async fn connect_tls_to_proxy_with_timeout<S>(
    stream: S,
    host: &str,
    handshake_timeout: Duration,
) -> Result<tokio_rustls::client::TlsStream<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let config = https_proxy_tls_config();
    let server_name = tokio_rustls::rustls::pki_types::ServerName::try_from(host.to_string())
        .with_context(|| format!("invalid HTTPS proxy TLS server name {host}"))?;
    timeout(
        handshake_timeout,
        TlsConnector::from(config).connect(server_name, stream),
    )
    .await
    .context("TLS handshake with HTTPS proxy timed out")?
    .context("TLS handshake with HTTPS proxy")
}

fn https_proxy_tls_config() -> Arc<ClientConfig> {
    HTTPS_PROXY_TLS_CONFIG
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            )
        })
        .clone()
}

async fn read_connect_response<S>(stream: &mut S) -> Result<()>
where
    S: AsyncRead + Unpin,
{
    let mut response = Vec::new();
    let mut buf = [0_u8; 512];
    loop {
        let n = stream.read(&mut buf).await?;
        anyhow::ensure!(n > 0, "HTTP proxy closed before CONNECT response");
        response.extend_from_slice(&buf[..n]);
        anyhow::ensure!(
            response.len() <= MAX_HTTP_CONNECT_RESPONSE,
            "HTTP proxy CONNECT response too large"
        );
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let text = std::str::from_utf8(&response).context("HTTP proxy response is not UTF-8")?;
    let status_line = text
        .lines()
        .next()
        .context("HTTP proxy response is empty")?;
    let mut parts = status_line.split_whitespace();
    let version = parts.next().unwrap_or_default();
    let status = parts.next().unwrap_or_default();
    anyhow::ensure!(
        version.starts_with("HTTP/") && status == "200",
        "HTTP proxy CONNECT failed: {status_line}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use espejismo_core::{EgressProxy, EgressProxyKind};
    use tokio::{
        io::{AsyncReadExt, duplex},
        time::timeout,
    };

    use super::{
        build_connect_request, connect_tls_to_proxy_with_timeout, https_proxy_tls_config,
    };

    #[test]
    fn https_proxy_handshakes_share_rustls_session_cache() {
        let first = https_proxy_tls_config();
        let second = https_proxy_tls_config();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
    }

    #[tokio::test]
    async fn https_proxy_tls_handshake_times_out_and_closes_connection() {
        let (client, mut peer) = duplex(4096);
        let server = tokio::spawn(async move {
            let mut received = [0_u8; 1024];
            let n = peer.read(&mut received).await.unwrap();
            assert!(n > 0, "client should send a TLS ClientHello");
            let n = timeout(Duration::from_secs(1), peer.read(&mut received))
                .await
                .expect("client socket should close after timeout")
                .unwrap();
            assert_eq!(n, 0, "cancelled TLS handshake must close its socket");
        });

        let result =
            connect_tls_to_proxy_with_timeout(client, "localhost", Duration::from_millis(30)).await;
        let error = result.expect_err("stalled TLS peer should hit handshake deadline");
        assert!(format!("{error:#}").contains("TLS handshake with HTTPS proxy timed out"));
        server.await.unwrap();
    }

    #[test]
    fn builds_http_connect_request_with_basic_auth() {
        let proxy = EgressProxy {
            kind: EgressProxyKind::Http,
            endpoint: "127.0.0.1:8080".to_string(),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
        };
        let request = build_connect_request(&proxy, "example.com:443").unwrap();
        assert!(request.starts_with("CONNECT example.com:443 HTTP/1.1\r\n"));
        assert!(request.contains("Host: example.com:443\r\n"));
        assert!(request.contains("Proxy-Authorization: Basic dXNlcjpwYXNz\r\n"));
        assert!(request.ends_with("\r\n\r\n"));
    }

    #[test]
    fn builds_https_proxy_connect_request_like_http_connect() {
        let proxy = EgressProxy {
            kind: EgressProxyKind::Https,
            endpoint: "proxy.example.com:8443".to_string(),
            username: None,
            password: None,
        };
        let request = build_connect_request(&proxy, "example.com:443").unwrap();
        assert!(request.starts_with("CONNECT example.com:443 HTTP/1.1\r\n"));
        assert!(!request.contains("Proxy-Authorization"));
    }
}
