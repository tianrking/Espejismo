use espejismo_core::ingress::http_proxy::accept_http_proxy_with_auth;
use espejismo_core::ProxyAuth;
use tokio::io::{duplex, AsyncReadExt, AsyncWriteExt};

#[tokio::test]
async fn connect_returns_success_and_preserves_tunnel_bytes_sent_with_headers() {
    let (mut client, mut proxy) = duplex(4096);
    let request = tokio::spawn(async move {
        client
            .write_all(b"CONNECT example.test:443 HTTP/1.1\r\nHost: example.test:443\r\n\r\nTLS")
            .await
            .unwrap();
        let mut response = [0; 128];
        let n = client.read(&mut response).await.unwrap();
        response[..n].to_vec()
    });

    let target = accept_http_proxy_with_auth(&mut proxy, None).await.unwrap();
    assert_eq!(target.authority, "example.test:443");
    assert_eq!(target.method, "CONNECT");
    assert_eq!(target.prebuffer, b"TLS");
    assert_eq!(request.await.unwrap(), b"HTTP/1.1 200 Connection Established\r\n\r\n");
}

#[tokio::test]
async fn forwards_absolute_request_as_origin_form_and_keeps_initial_body() {
    let (mut client, mut proxy) = duplex(4096);
    let writer = tokio::spawn(async move {
        client
            .write_all(b"POST http://example.test/upload?q=1 HTTP/1.1\r\nHost: example.test\r\nProxy-Authorization: Basic dTpw\r\nContent-Length: 4\r\n\r\ndata")
            .await
            .unwrap();
    });

    let target = accept_http_proxy_with_auth(
        &mut proxy,
        Some(&ProxyAuth { username: "u".into(), password: "p".into() }),
    )
    .await
    .unwrap();
    writer.await.unwrap();
    assert_eq!(target.authority, "example.test:80");
    assert_eq!(target.method, "POST");
    assert_eq!(target.path, "/upload?q=1");
    assert_eq!(target.content_length, Some(4));
    assert_eq!(target.prebuffer_body_bytes, 4);
    assert!(target.prebuffer.ends_with(b"\r\n\r\ndata"));
    assert!(!target.prebuffer.windows(20).any(|part| part == b"Proxy-Authorization"));
}

#[tokio::test]
async fn rejects_bad_credentials_with_407() {
    let (mut client, mut proxy) = duplex(4096);
    let writer = tokio::spawn(async move {
        client
            .write_all(b"GET http://example.test/ HTTP/1.1\r\nProxy-Authorization: Basic dTpiYWQ=\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        response
    });
    let auth = ProxyAuth { username: "u".into(), password: "p".into() };
    let error = accept_http_proxy_with_auth(&mut proxy, Some(&auth)).await.unwrap_err();
    assert!(format!("{error:#}").contains("authentication failed"));
    drop(proxy);
    assert!(writer.await.unwrap().starts_with(b"HTTP/1.1 407 Proxy Authentication Required\r\n"));
}

#[tokio::test]
async fn rejects_missing_credentials_with_407_before_connecting() {
    let (mut client, mut proxy) = duplex(4096);
    let request = tokio::spawn(async move {
        client
            .write_all(b"CONNECT example.test:443 HTTP/1.1\r\nHost: example.test:443\r\n\r\n")
            .await
            .unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        response
    });
    let auth = ProxyAuth { username: "u".into(), password: "p".into() };

    let error = accept_http_proxy_with_auth(&mut proxy, Some(&auth))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("authentication failed"));
    drop(proxy);

    let response = request.await.unwrap();
    assert!(response.starts_with(b"HTTP/1.1 407 Proxy Authentication Required\r\n"));
    assert!(!response.windows(3).any(|part| part == b"200"));
}

#[tokio::test]
async fn rejects_connect_with_bad_credentials_without_establishing_tunnel() {
    let (mut client, mut proxy) = duplex(4096);
    let request = tokio::spawn(async move {
        client
            .write_all(
                b"CONNECT example.test:443 HTTP/1.1\r\n\
                  Proxy-Authorization: Basic dTpiYWQ=\r\n\
                  \r\nTLS",
            )
            .await
            .unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        response
    });
    let auth = ProxyAuth { username: "u".into(), password: "p".into() };

    let error = accept_http_proxy_with_auth(&mut proxy, Some(&auth))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("authentication failed"));
    drop(proxy);

    let response = request.await.unwrap();
    assert!(response.starts_with(b"HTTP/1.1 407 Proxy Authentication Required\r\n"));
    assert!(!response.windows(3).any(|part| part == b"200"));
    assert!(!response.ends_with(b"TLS"));
}

#[tokio::test]
async fn rejects_origin_form_target_without_sending_success_response() {
    let (mut client, mut proxy) = duplex(4096);
    let writer = tokio::spawn(async move {
        client.write_all(b"GET /relative HTTP/1.1\r\nHost: example.test\r\n\r\n").await.unwrap();
        let mut response = Vec::new();
        client.read_to_end(&mut response).await.unwrap();
        response
    });
    let error = accept_http_proxy_with_auth(&mut proxy, None).await.unwrap_err();
    assert!(format!("{error:#}").contains("absolute http:// requests"));
    drop(proxy);
    assert!(writer.await.unwrap().is_empty());
}
