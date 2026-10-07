use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result};
use base64::Engine;
use espejismo_core::{EgressProxy, EgressProxyKind, TransportStream};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{timeout, Duration};
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

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
            // Pin HTTPS proxy TLS to the ring provider already selected by the
            // workspace, while retaining rustls' safe TLS 1.2/1.3 defaults.
            Arc::new(
                ClientConfig::builder_with_provider(https_proxy_tls_provider())
                    .with_safe_default_protocol_versions()
                    .expect("ring supports the safe default TLS protocol versions")
                    .with_root_certificates(roots)
                    .with_no_client_auth(),
            )
        })
        .clone()
}

fn https_proxy_tls_provider() -> Arc<tokio_rustls::rustls::crypto::CryptoProvider> {
    Arc::new(tokio_rustls::rustls::crypto::ring::default_provider())
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
    use std::{sync::Arc, time::Duration};

    use espejismo_core::{EgressProxy, EgressProxyKind};
    use tokio::{
        io::{duplex, AsyncReadExt},
        time::timeout,
    };
    use tokio_rustls::rustls::{
        client::danger::{HandshakeSignatureValid, ServerCertVerifier},
        pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime},
        DigitallySignedStruct, ServerConfig, SignatureScheme,
    };
    use tokio_rustls::TlsAcceptor;

    use super::{build_connect_request, connect_tls_to_proxy_with_timeout, https_proxy_tls_config};

    #[test]
    fn https_proxy_handshakes_share_rustls_session_cache() {
        let first = https_proxy_tls_config();
        let second = https_proxy_tls_config();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert!(
            first.alpn_protocols.is_empty(),
            "HTTPS proxy TLS must not negotiate an application protocol"
        );
    }

    #[tokio::test]
    async fn https_proxy_tls_succeeds_when_server_advertises_alpn() {
        use tokio_rustls::rustls::ClientConfig;
        use tokio_rustls::TlsConnector;

        let cert = CertificateDer::from(
            include_bytes!("../tests/data/untrusted-localhost-cert.der").to_vec(),
        );
        let key = PrivateKeyDer::try_from(
            include_bytes!("../tests/data/untrusted-localhost-key.der").to_vec(),
        )
        .unwrap();
        let mut server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert.clone()], key)
            .unwrap();
        server_config.alpn_protocols = vec![b"h2".to_vec()];

        let client_config = Arc::new(
            ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AlpnTestVerifier))
                .with_no_client_auth(),
        );
        assert!(client_config.alpn_protocols.is_empty());

        let (client, server) = tokio::io::duplex(4096);
        let server = tokio::spawn(async move {
            TlsAcceptor::from(Arc::new(server_config))
                .accept(server)
                .await
                .unwrap()
                .get_ref()
                .1
                .alpn_protocol()
                .map(ToOwned::to_owned)
        });
        let server_name = ServerName::try_from("localhost").unwrap();
        let client = TlsConnector::from(client_config)
            .connect(server_name, client)
            .await
            .expect("TLS should succeed when the client offers no ALPN");

        assert_eq!(client.get_ref().1.alpn_protocol(), None);
        assert!(
            super::https_proxy_tls_provider()
                .cipher_suites
                .iter()
                .any(|suite| Some(suite.suite())
                    == client
                        .get_ref()
                        .1
                        .negotiated_cipher_suite()
                        .map(|s| s.suite())),
            "negotiated suite must be in the configured HTTPS proxy suite set"
        );
        assert_eq!(server.await.unwrap(), None);
    }

    #[derive(Debug)]
    struct AlpnTestVerifier;

    impl ServerCertVerifier for AlpnTestVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<
            tokio_rustls::rustls::client::danger::ServerCertVerified,
            tokio_rustls::rustls::Error,
        > {
            Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _message: &[u8],
            _cert: &CertificateDer<'_>,
            _dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            tokio_rustls::rustls::crypto::ring::default_provider()
                .signature_verification_algorithms
                .supported_schemes()
        }
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

    #[tokio::test]
    async fn https_proxy_rejects_untrusted_server_certificate() {
        // This self-signed localhost certificate is intentionally absent from
        // the bundled WebPKI roots used by the production client config.
        let cert = CertificateDer::from(
            include_bytes!("../tests/data/untrusted-localhost-cert.der").to_vec(),
        );
        let key = PrivateKeyDer::try_from(
            include_bytes!("../tests/data/untrusted-localhost-key.der").to_vec(),
        )
        .unwrap();
        let config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        let (client, server) = duplex(4096);
        let server = tokio::spawn(async move {
            let acceptor = TlsAcceptor::from(std::sync::Arc::new(config));
            acceptor.accept(server).await
        });

        let result =
            connect_tls_to_proxy_with_timeout(client, "localhost", Duration::from_secs(2)).await;
        let error = result.expect_err("self-signed proxy certificate must be rejected");
        assert!(format!("{error:#}").contains("TLS handshake with HTTPS proxy"));
        assert!(
            server.await.unwrap().is_err(),
            "server handshake should be aborted after client rejection"
        );
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
