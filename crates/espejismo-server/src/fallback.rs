use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use rand::seq::SliceRandom;
use rand::Rng;
use tokio::io::{copy_bidirectional, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Duration};

use crate::tarpit;

#[derive(Clone)]
pub(crate) struct FallbackHttpRuntime {
    pub(crate) enabled: bool,
    pub(crate) upstream: Option<String>,
    pub(crate) probe_timeout: Duration,
    pub(crate) server: String,
    pub(crate) body: String,
}

pub(crate) async fn fallback_or_reject(
    mut stream: TcpStream,
    fallback: &FallbackHttpRuntime,
    reject_delay: Duration,
    tarpit: &tarpit::TarpitManager,
) {
    if fallback.enabled {
        let _ = write_builtin_fallback_response(&mut stream, fallback).await;
        return;
    }
    reject_or_quarantine(stream, reject_delay, tarpit).await;
}

pub(crate) async fn should_route_to_http_fallback(
    stream: &mut TcpStream,
    fallback: &FallbackHttpRuntime,
) -> Result<bool> {
    if !fallback.enabled {
        return Ok(false);
    }
    let mut buf = [0_u8; 16];
    let n = match timeout(fallback.probe_timeout, stream.peek(&mut buf)).await {
        Ok(Ok(n)) => n,
        Ok(Err(err)) => return Err(err.into()),
        Err(_) => return Ok(false),
    };
    if n == 0 {
        return Ok(false);
    }
    Ok(looks_like_http_probe(&buf[..n]))
}

pub(crate) async fn route_http_fallback(
    mut inbound: TcpStream,
    fallback: &FallbackHttpRuntime,
) -> Result<()> {
    if let Some(upstream) = &fallback.upstream {
        let mut upstream_stream = TcpStream::connect(upstream)
            .await
            .with_context(|| format!("connect fallback upstream {upstream}"))?;
        let _ = copy_bidirectional(&mut inbound, &mut upstream_stream).await?;
        return Ok(());
    }
    write_builtin_fallback_response(&mut inbound, fallback).await
}

async fn reject_or_quarantine(
    stream: TcpStream,
    reject_delay: Duration,
    tarpit: &tarpit::TarpitManager,
) {
    if reject_delay.is_zero() {
        tarpit.quarantine(stream).await;
    } else {
        quiet_reject(stream, reject_delay).await;
    }
}

async fn quiet_reject(mut stream: TcpStream, delay: Duration) {
    if !delay.is_zero() {
        sleep(delay).await;
    }
    let _ = stream.shutdown().await;
}

fn looks_like_http_probe(prefix: &[u8]) -> bool {
    let methods: [&[u8]; 10] = [
        b"GET ",
        b"POST ",
        b"HEAD ",
        b"PUT ",
        b"PATCH ",
        b"DELETE ",
        b"OPTIONS ",
        b"CONNECT ",
        b"TRACE ",
        b"PRI * HTTP/2.0",
    ];
    methods.iter().any(|m| prefix.starts_with(m))
}

async fn write_builtin_fallback_response(
    stream: &mut TcpStream,
    fallback: &FallbackHttpRuntime,
) -> Result<()> {
    let response = build_builtin_fallback_response(fallback);
    stream.write_all(&response).await?;
    stream.shutdown().await?;
    Ok(())
}

fn build_builtin_fallback_response(fallback: &FallbackHttpRuntime) -> Vec<u8> {
    let body = fallback.body.as_bytes();
    let now = SystemTime::now();
    let modified = now
        .checked_sub(Duration::from_secs(
            rand::thread_rng().gen_range(600..=86_400),
        ))
        .unwrap_or(UNIX_EPOCH);
    let etag = format!(
        "\"{:x}-{:x}-{:x}\"",
        body.len(),
        unix_secs(modified),
        rand::thread_rng().gen::<u32>()
    );
    let cache_header = [
        "Cache-Control: no-cache\r\n",
        "Cache-Control: max-age=0\r\n",
        "Accept-Ranges: bytes\r\n",
    ]
    .choose(&mut rand::thread_rng())
    .copied()
    .unwrap_or("Accept-Ranges: bytes\r\n");

    format!(
        "HTTP/1.1 200 OK\r\nDate: {}\r\nServer: {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nLast-Modified: {}\r\nETag: {}\r\n{}Connection: close\r\n\r\n{}",
        http_date(now),
        fallback_server_header(&fallback.server),
        body.len(),
        http_date(modified),
        etag,
        cache_header,
        fallback.body
    )
    .into_bytes()
}

fn fallback_server_header(configured: &str) -> String {
    let trimmed = configured.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("nginx") {
        let versions = ["1.18.0", "1.20.2", "1.22.1", "1.24.0", "1.25.5"];
        let version = versions
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or("1.24.0");
        return format!("nginx/{version}");
    }
    if trimmed.eq_ignore_ascii_case("caddy") {
        return "Caddy".to_string();
    }
    trimmed.to_string()
}

fn http_date(time: SystemTime) -> String {
    let secs = unix_secs(time) as i64;
    let days = secs.div_euclid(86_400);
    let day_secs = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;
    let second = day_secs % 60;
    let weekdays = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let weekday = weekdays[days.rem_euclid(7) as usize];
    let month_name = months[(month - 1) as usize];
    format!("{weekday}, {day:02} {month_name} {year:04} {hour:02}:{minute:02}:{second:02} GMT")
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = y + if month <= 2 { 1 } else { 0 };
    (year as i32, month as u32, day as u32)
}

fn unix_secs(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::{build_builtin_fallback_response, looks_like_http_probe, FallbackHttpRuntime};
    use tokio::time::Duration;

    #[test]
    fn detects_common_http_methods() {
        assert!(looks_like_http_probe(b"GET / HTTP/1.1\r\n"));
        assert!(looks_like_http_probe(b"POST /submit HTTP/1.1\r\n"));
        assert!(looks_like_http_probe(
            b"CONNECT example.com:443 HTTP/1.1\r\n"
        ));
    }

    #[test]
    fn ignores_non_http_prefixes() {
        assert!(!looks_like_http_probe(b"\x16\x03\x01\x02\x00"));
        assert!(!looks_like_http_probe(b"\x8f\xf2\x00\x11"));
    }

    #[test]
    fn builtin_fallback_response_has_browser_like_headers() {
        let fallback = FallbackHttpRuntime {
            enabled: true,
            upstream: None,
            probe_timeout: Duration::from_millis(250),
            server: "nginx".to_string(),
            body: "<html>ok</html>".to_string(),
        };

        let response = String::from_utf8(build_builtin_fallback_response(&fallback)).unwrap();
        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("\r\nDate: "));
        assert!(response.contains("\r\nServer: nginx/"));
        assert!(response.contains("\r\nLast-Modified: "));
        assert!(response.contains("\r\nETag: "));
        assert!(response.contains("\r\nContent-Length: 15\r\n"));
    }
}
