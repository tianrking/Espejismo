//! Local proxy ingress protocols and client authentication.

pub mod auth;
pub mod http_proxy;
pub mod socks5;

pub use auth::ProxyAuth;
