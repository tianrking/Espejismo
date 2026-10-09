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
            // Pin HTTPS proxy TLS to the ring provider already selected by the
            // workspace, while retaining rustls' safe TLS 1.2/1.3 defaults.
            let mut config = ClientConfig::builder_with_provider(https_proxy_tls_provider())
                .with_safe_default_protocol_versions()
                .expect("ring supports the safe default TLS protocol versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            // TLS key logging exposes traffic secrets and is intended only for
            // explicitly configured diagnostics. This proxy path never opts in.
            config.key_log = Arc::new(tokio_rustls::rustls::NoKeyLog);
            // CONNECT changes proxy state and must never be replayed as TLS
            // 1.3 early data. Keep 0-RTT disabled even if rustls defaults change.
            config.enable_early_data = false;
            // HTTP CONNECT is used without an application protocol. Keep this
            // explicit so a future TLS config change cannot negotiate h2/HTTP.
            config.alpn_protocols.clear();
            Arc::new(config)
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
        io::{AsyncReadExt, duplex},
        time::timeout,
    };
    use tokio_rustls::TlsAcceptor;
    use tokio_rustls::rustls::{
        DigitallySignedStruct, ServerConfig, SignatureScheme,
        client::danger::{HandshakeSignatureValid, ServerCertVerifier},
        pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime},
    };

    use super::{build_connect_request, connect_tls_to_proxy_with_timeout, https_proxy_tls_config};

    #[test]
    fn https_proxy_handshakes_share_rustls_session_cache() {
        let first = https_proxy_tls_config();
        let second = https_proxy_tls_config();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert!(
            !first.enable_early_data,
            "HTTPS proxy requests must wait for the authenticated TLS handshake"
        );
        assert!(
            first.alpn_protocols.is_empty(),
            "HTTPS proxy TLS must not negotiate an application protocol"
        );
        assert!(
            !first.key_log.will_log("CLIENT_TRAFFIC_SECRET_0"),
            "HTTPS proxy TLS secrets must not be written to a key log"
        );
    }

    #[tokio::test]
    async fn https_proxy_tls_never_negotiates_server_advertised_alpn() {
        use tokio_rustls::TlsConnector;
        use tokio_rustls::rustls::ClientConfig;

        let cert = CertificateDer::from(
            include_bytes!("../tests/data/untrusted-localhost-cert.der").to_vec(),
        );
        let key = PrivateKeyDer::try_from(
            include_bytes!("../tests/data/untrusted-localhost-key.der").to_vec(),
        )
        .unwrap();
        let server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert.clone()], key)
            .unwrap();
        let client_config = Arc::new(
            ClientConfig::builder()
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AlpnTestVerifier))
                .with_no_client_auth(),
        );
        assert!(client_config.alpn_protocols.is_empty());

        for advertised in [
            vec![],
            vec![b"h2".to_vec()],
            vec![b"http/1.1".to_vec()],
            vec![b"h2".to_vec(), b"http/1.1".to_vec()],
        ] {
            let mut config = server_config.clone();
            config.alpn_protocols = advertised.clone();
            let (client, server) = tokio::io::duplex(4096);
            let server = tokio::spawn(async move {
                TlsAcceptor::from(Arc::new(config))
                    .accept(server)
                    .await
                    .unwrap()
                    .get_ref()
                    .1
                    .alpn_protocol()
                    .map(ToOwned::to_owned)
            });
            let server_name = ServerName::try_from("localhost").unwrap();
            let client = TlsConnector::from(client_config.clone())
                .connect(server_name, client)
                .await
                .expect("TLS should succeed when the client offers no ALPN");

            assert_eq!(client.get_ref().1.alpn_protocol(), None, "server ALPN: {advertised:?}");
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
            assert_eq!(server.await.unwrap(), None, "server ALPN: {advertised:?}");
        }
    }

    #[tokio::test]
    async fn tls12_ocsp_staple_bytes_reach_certificate_verifier_unchanged() {
        use std::sync::Mutex;
        use tokio_rustls::TlsConnector;
        use tokio_rustls::rustls::{ClientConfig, ServerConfig, version};

        // rustls transports the staple to the verifier but does not validate
        // OCSP itself. Exercise empty, minimal, typical, and near-record-sized
        // opaque responses; TLS may fragment the largest response across records.
        for staple in [
            Vec::new(),
            vec![0x30],
            vec![0x30; 4096],
            vec![0x30; 16 * 1024],
        ] {
            let seen = Arc::new(Mutex::new(None));
            let cert = CertificateDer::from(
                include_bytes!("../tests/data/untrusted-localhost-cert.der").to_vec(),
            );
            let key = PrivateKeyDer::try_from(
                include_bytes!("../tests/data/untrusted-localhost-key.der").to_vec(),
            )
            .unwrap();
            let server_config = ServerConfig::builder_with_protocol_versions(&[&version::TLS12])
                .with_no_client_auth()
                .with_single_cert_with_ocsp(vec![cert], key, staple.clone())
                .unwrap();
            let verifier = Arc::new(OcspCaptureVerifier(seen.clone()));
            let client_config = ClientConfig::builder_with_protocol_versions(&[&version::TLS12])
                .dangerous()
                .with_custom_certificate_verifier(verifier)
                .with_no_client_auth();

            let (client, server) = duplex(64 * 1024);
            let server_task = tokio::spawn(async move {
                TlsAcceptor::from(Arc::new(server_config))
                    .accept(server)
                    .await
                    .unwrap();
            });
            let name = ServerName::try_from("localhost").unwrap();
            TlsConnector::from(Arc::new(client_config))
                .connect(name, client)
                .await
                .unwrap();
            server_task.await.unwrap();
            assert_eq!(*seen.lock().unwrap(), Some(staple));
        }
    }

    #[tokio::test]
    async fn tls12_ocsp_policy_rejection_fails_handshake_for_missing_or_malformed_staple() {
        use tokio_rustls::TlsConnector;
        use tokio_rustls::rustls::{ClientConfig, ServerConfig, version};

        // rustls forwards OCSP bytes to the configured verifier; it does not
        // apply an OCSP freshness policy on its own. This strict test verifier
        // rejects absent and malformed responses and verifies that the error
        // reaches the TLS handshake caller.
        for staple in [Vec::new(), vec![0x30]] {
            let cert = CertificateDer::from(
                include_bytes!("../tests/data/untrusted-localhost-cert.der").to_vec(),
            );
            let key = PrivateKeyDer::try_from(
                include_bytes!("../tests/data/untrusted-localhost-key.der").to_vec(),
            )
            .unwrap();
            let server_config = ServerConfig::builder_with_protocol_versions(&[&version::TLS12])
                .with_no_client_auth()
                .with_single_cert_with_ocsp(vec![cert], key, staple.clone())
                .unwrap();
            let client_config = ClientConfig::builder_with_protocol_versions(&[&version::TLS12])
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(OcspRejectingVerifier))
                .with_no_client_auth();

            let (client, server) = duplex(16 * 1024);
            let server_task = tokio::spawn(async move {
                TlsAcceptor::from(Arc::new(server_config))
                    .accept(server)
                    .await
            });
            let result = TlsConnector::from(Arc::new(client_config))
                .connect(ServerName::try_from("localhost").unwrap(), client)
                .await;
            assert!(result.is_err(), "staple {staple:?} should be rejected");
            assert!(server_task.await.unwrap().is_err());
        }
    }

    #[derive(Debug)]
    struct OcspRejectingVerifier;

    impl ServerCertVerifier for OcspRejectingVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<
            tokio_rustls::rustls::client::danger::ServerCertVerified,
            tokio_rustls::rustls::Error,
        > {
            if ocsp_response.is_empty() || ocsp_response.first() != Some(&0x30) {
                return Err(tokio_rustls::rustls::Error::General(
                    "OCSP staple missing or malformed".into(),
                ));
            }
            Err(tokio_rustls::rustls::Error::General(
                "OCSP response rejected by test policy".into(),
            ))
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            AlpnTestVerifier.verify_tls12_signature(message, cert, dss)
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            AlpnTestVerifier.verify_tls13_signature(message, cert, dss)
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            AlpnTestVerifier.supported_verify_schemes()
        }
    }

    #[derive(Debug)]
    struct OcspCaptureVerifier(Arc<std::sync::Mutex<Option<Vec<u8>>>>);

    impl ServerCertVerifier for OcspCaptureVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            ocsp_response: &[u8],
            _now: UnixTime,
        ) -> Result<
            tokio_rustls::rustls::client::danger::ServerCertVerified,
            tokio_rustls::rustls::Error,
        > {
            *self.0.lock().unwrap() = Some(ocsp_response.to_vec());
            Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            AlpnTestVerifier.verify_tls12_signature(message, cert, dss)
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, tokio_rustls::rustls::Error> {
            AlpnTestVerifier.verify_tls13_signature(message, cert, dss)
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            AlpnTestVerifier.supported_verify_schemes()
        }
    }

    #[tokio::test]
    async fn https_proxy_session_tickets_are_reused_and_replenished() {
        use tokio_rustls::rustls::{ClientConfig, HandshakeKind, ServerConfig, version};

        let cert = CertificateDer::from(
            include_bytes!("../tests/data/untrusted-localhost-cert.der").to_vec(),
        );
        let key = PrivateKeyDer::try_from(
            include_bytes!("../tests/data/untrusted-localhost-key.der").to_vec(),
        )
        .unwrap();
        let mut server_config = ServerConfig::builder_with_protocol_versions(&[&version::TLS13])
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        // TLS 1.3 tickets are single-use; multiple tickets let a resumed
        // connection consume one while refreshing the client's cache.
        server_config.send_tls13_tickets = 2;
        assert_eq!(
            server_config.max_early_data_size, 0,
            "the HTTPS proxy must not authorize TLS 1.3 early data"
        );

        let mut client_config = ClientConfig::builder_with_protocol_versions(&[&version::TLS13])
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AlpnTestVerifier))
            .with_no_client_auth();
        // Simulate a client willing to send 0-RTT. The server's ticket must
        // still refuse early application data because CONNECT is stateful.
        client_config.enable_early_data = true;
        assert!(client_config.enable_early_data);
        let client_config = Arc::new(client_config);

        async fn handshake(
            client_config: Arc<ClientConfig>,
            server_config: Arc<ServerConfig>,
        ) -> bool {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            use tokio_rustls::{TlsAcceptor, TlsConnector};

            let (client_io, server_io) = duplex(16 * 1024);
            let server_task = tokio::spawn(async move {
                let mut tls = TlsAcceptor::from(server_config)
                    .accept(server_io)
                    .await
                    .unwrap();
                tls.write_all(b"ready").await.unwrap();
                tls.flush().await.unwrap();
            });
            let name = ServerName::try_from("localhost").unwrap();
            let mut tls = TlsConnector::from(client_config)
                .connect(name, client_io)
                .await
                .unwrap();
            let mut ready = [0; 5];
            tls.read_exact(&mut ready).await.unwrap();
            assert_eq!(&ready, b"ready");
            let resumed = tls.get_ref().1.handshake_kind() == Some(HandshakeKind::Resumed);
            assert!(
                !tls.get_ref().1.is_early_data_accepted(),
                "HTTPS proxy must reject TLS early data, including on resumed sessions"
            );
            server_task.await.unwrap();
            resumed
        }

        let server_config = Arc::new(server_config);
        assert!(!handshake(client_config.clone(), server_config.clone()).await);
        assert!(handshake(client_config.clone(), server_config.clone()).await);
        // The resumed handshake must receive fresh tickets so the cache can
        // continue across further proxy connections.
        assert!(handshake(client_config, server_config).await);
    }

    #[tokio::test]
    async fn https_proxy_tls12_sessions_resume_with_shared_config() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio_rustls::rustls::{ClientConfig, HandshakeKind, ServerConfig, version};
        use tokio_rustls::{TlsAcceptor, TlsConnector};

        let cert = CertificateDer::from(
            include_bytes!("../tests/data/untrusted-localhost-cert.der").to_vec(),
        );
        let key = PrivateKeyDer::try_from(
            include_bytes!("../tests/data/untrusted-localhost-key.der").to_vec(),
        )
        .unwrap();
        let server = Arc::new(
            ServerConfig::builder_with_protocol_versions(&[&version::TLS12])
                .with_no_client_auth()
                .with_single_cert(vec![cert], key)
                .unwrap(),
        );
        let client = Arc::new(
            ClientConfig::builder_with_protocol_versions(&[&version::TLS12])
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AlpnTestVerifier))
                .with_no_client_auth(),
        );

        async fn handshake(client: Arc<ClientConfig>, server: Arc<ServerConfig>) -> HandshakeKind {
            let (client_io, server_io) = duplex(16 * 1024);
            let server_task = tokio::spawn(async move {
                let mut tls = TlsAcceptor::from(server).accept(server_io).await.unwrap();
                tls.write_all(b"ready").await.unwrap();
                tls.flush().await.unwrap();
            });
            let name = ServerName::try_from("localhost").unwrap();
            let mut tls = TlsConnector::from(client)
                .connect(name, client_io)
                .await
                .unwrap();
            let mut ready = [0; 5];
            tls.read_exact(&mut ready).await.unwrap();
            assert_eq!(&ready, b"ready");
            let kind = tls.get_ref().1.handshake_kind().unwrap();
            server_task.await.unwrap();
            kind
        }

        assert_eq!(
            handshake(client.clone(), server.clone()).await,
            HandshakeKind::Full
        );
        assert_eq!(handshake(client, server).await, HandshakeKind::Resumed);
    }

    #[tokio::test]
    async fn https_proxy_session_cache_is_partitioned_by_server_name() {
        use tokio::io::AsyncWriteExt;
        use tokio_rustls::TlsConnector;
        use tokio_rustls::rustls::{ClientConfig, HandshakeKind, ServerConfig, version};

        let cert = CertificateDer::from(
            include_bytes!("../tests/data/untrusted-localhost-cert.der").to_vec(),
        );
        let key = PrivateKeyDer::try_from(
            include_bytes!("../tests/data/untrusted-localhost-key.der").to_vec(),
        )
        .unwrap();
        let mut server = ServerConfig::builder_with_protocol_versions(&[&version::TLS13])
            .with_no_client_auth()
            .with_single_cert(vec![cert], key)
            .unwrap();
        server.send_tls13_tickets = 2;
        let server = Arc::new(server);
        let client = Arc::new(
            ClientConfig::builder_with_protocol_versions(&[&version::TLS13])
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(AlpnTestVerifier))
                .with_no_client_auth(),
        );

        async fn handshake(
            client: Arc<ClientConfig>,
            server: Arc<ServerConfig>,
            name: &'static str,
        ) -> HandshakeKind {
            let (client_io, server_io) = duplex(16 * 1024);
            let server_task = tokio::spawn(async move {
                let mut tls = TlsAcceptor::from(server).accept(server_io).await.unwrap();
                tls.write_all(b"ready").await.unwrap();
            });
            let mut tls = TlsConnector::from(client)
                .connect(ServerName::try_from(name).unwrap(), client_io)
                .await
                .unwrap();
            let mut ready = [0; 5];
            tls.read_exact(&mut ready).await.unwrap();
            assert_eq!(&ready, b"ready");
            let kind = tls.get_ref().1.handshake_kind().unwrap();
            server_task.await.unwrap();
            kind
        }

        assert_eq!(
            handshake(client.clone(), server.clone(), "localhost").await,
            HandshakeKind::Full
        );
        // A different SNI must not consume the localhost ticket; returning to
        // localhost should still resume from the shared config's cache.
        assert_eq!(
            handshake(client.clone(), server.clone(), "otherhost").await,
            HandshakeKind::Full
        );
        assert_eq!(
            handshake(client, server, "localhost").await,
            HandshakeKind::Resumed
        );
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

    #[tokio::test]
    async fn https_proxy_rejects_invalid_tls_server_name_before_handshake() {
        // A proxy endpoint host is also the TLS identity. Reject malformed
        // names before emitting a ClientHello so no ambiguous identity is used.
        let (client, mut peer) = duplex(4096);
        let result =
            connect_tls_to_proxy_with_timeout(client, "proxy host", Duration::from_secs(2)).await;

        let error = result.expect_err("a TLS server name containing whitespace is invalid");
        assert!(format!("{error:#}").contains("invalid HTTPS proxy TLS server name"));
        let mut received = [0_u8; 1];
        assert_eq!(peer.read(&mut received).await.unwrap(), 0);
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
