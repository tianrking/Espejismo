use std::future::Future;
use std::pin::Pin;
use std::process::Command;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;

use crate::egress::EgressPolicy;
use crate::protocol::request::StreamPriority;

pub type BoxFutureResult<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;

#[derive(Clone, Debug, Serialize)]
pub struct AuthRequest {
    pub peer: String,
    pub user_hint: Option<String>,
    pub credential: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AuthDecision {
    pub accepted: bool,
    pub user: Option<String>,
}

impl AuthDecision {
    pub fn accepted(user: impl Into<String>) -> Self {
        Self {
            accepted: true,
            user: Some(user.into()),
        }
    }

    pub fn rejected() -> Self {
        Self {
            accepted: false,
            user: None,
        }
    }
}

pub trait Authenticator: Send + Sync {
    fn authenticate<'a>(&'a self, request: AuthRequest) -> BoxFutureResult<'a, AuthDecision>;
}

#[derive(Clone, Debug)]
pub struct CommandAuthenticator {
    program: String,
    args: Vec<String>,
}

impl CommandAuthenticator {
    pub fn new(program: impl Into<String>, args: Vec<String>) -> Self {
        Self {
            program: program.into(),
            args,
        }
    }
}

impl Authenticator for CommandAuthenticator {
    fn authenticate<'a>(&'a self, request: AuthRequest) -> BoxFutureResult<'a, AuthDecision> {
        Box::pin(async move {
            let json = serde_json::to_string(&request)?;
            let output = Command::new(&self.program)
                .args(&self.args)
                .env("ESPEJISMO_AUTH_REQUEST", json)
                .output()?;
            if !output.status.success() {
                return Ok(AuthDecision::rejected());
            }
            if output.stdout.is_empty() {
                return Ok(AuthDecision::accepted(
                    request.user_hint.unwrap_or_else(|| "external".to_string()),
                ));
            }
            Ok(serde_json::from_slice(&output.stdout)?)
        })
    }
}

#[derive(Clone, Debug)]
pub struct HttpJsonAuthenticator {
    url: String,
}

impl HttpJsonAuthenticator {
    pub fn new(url: impl Into<String>) -> Self {
        Self { url: url.into() }
    }
}

impl Authenticator for HttpJsonAuthenticator {
    fn authenticate<'a>(&'a self, request: AuthRequest) -> BoxFutureResult<'a, AuthDecision> {
        Box::pin(async move {
            let response = ureq::post(&self.url).send_json(serde_json::to_value(&request)?)?;
            if response.status() != 200 {
                return Ok(AuthDecision::rejected());
            }
            Ok(response.into_json::<AuthDecision>()?)
        })
    }
}

#[derive(Clone, Debug)]
pub struct EgressRequest {
    pub user: String,
    pub authority: String,
    pub priority: StreamPriority,
    pub policy: EgressPolicy,
}

pub trait OutboundConnector: Send + Sync {
    fn connect_tcp<'a>(&'a self, request: EgressRequest) -> BoxFutureResult<'a, TcpStream>;

    fn relay_udp<'a>(
        &'a self,
        request: EgressRequest,
        payload: &'a [u8],
    ) -> BoxFutureResult<'a, Vec<u8>>;
}

#[derive(Clone, Debug)]
pub struct RequestContext {
    pub user: String,
    pub authority: String,
    pub is_udp: bool,
    pub priority: StreamPriority,
}

pub trait RequestPolicy: Send + Sync {
    fn check<'a>(&'a self, request: RequestContext) -> BoxFutureResult<'a, ()>;
}

#[derive(Clone, Debug, Serialize)]
pub struct TrafficEvent {
    pub event: &'static str,
    pub user: String,
    pub authority: Option<String>,
    pub client_to_remote: u64,
    pub remote_to_client: u64,
    pub reason: Option<String>,
}

pub trait TrafficObserver: Send + Sync {
    fn observe(&self, event: TrafficEvent);
}

pub trait TransportStream: AsyncRead + AsyncWrite + Send + Unpin {}

impl<T> TransportStream for T where T: AsyncRead + AsyncWrite + Send + Unpin {}

#[derive(Clone, Debug)]
pub struct TransportTarget {
    pub endpoint: String,
}

pub trait TransportConnector: Send + Sync {
    fn connect<'a>(
        &'a self,
        target: TransportTarget,
    ) -> BoxFutureResult<'a, Box<dyn TransportStream>>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopTrafficObserver;

impl TrafficObserver for NoopTrafficObserver {
    fn observe(&self, _event: TrafficEvent) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct RecordingObserver {
        events: Mutex<Vec<TrafficEvent>>,
    }

    impl TrafficObserver for RecordingObserver {
        fn observe(&self, event: TrafficEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[test]
    fn auth_decision_helpers_set_expected_state() {
        let accepted = AuthDecision::accepted("alice");
        assert!(accepted.accepted);
        assert_eq!(accepted.user.as_deref(), Some("alice"));

        let rejected = AuthDecision::rejected();
        assert!(!rejected.accepted);
        assert!(rejected.user.is_none());
    }

    #[test]
    fn traffic_event_is_json_serializable() {
        let event = TrafficEvent {
            event: "stream_closed",
            user: "alice".to_string(),
            authority: Some("example.com:443".to_string()),
            client_to_remote: 10,
            remote_to_client: 20,
            reason: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("stream_closed"));
        assert!(json.contains("example.com:443"));
    }

    #[test]
    fn observer_receives_events() {
        let observer = RecordingObserver::default();
        observer.observe(TrafficEvent {
            event: "stream_failed",
            user: "alice".to_string(),
            authority: None,
            client_to_remote: 0,
            remote_to_client: 0,
            reason: Some("timeout".to_string()),
        });
        assert_eq!(observer.events.lock().unwrap().len(), 1);
    }
}
