use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

#[derive(Clone, Serialize, Deserialize)]
pub struct ProxyAuth {
    pub username: String,
    pub password: String,
}

impl std::fmt::Debug for ProxyAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyAuth")
            .field("username", &"<redacted>")
            .field("password", &"<redacted>")
            .finish()
    }
}

impl ProxyAuth {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.username.is_empty(),
            "local.auth.username must not be empty"
        );
        ensure!(
            self.username.len() <= u8::MAX as usize,
            "local.auth.username must be at most 255 bytes"
        );
        ensure!(
            self.password.len() <= u8::MAX as usize,
            "local.auth.password must be at most 255 bytes"
        );
        Ok(())
    }

    pub fn matches(&self, username: &[u8], password: &[u8]) -> bool {
        let expected_user = self.username.as_bytes();
        let expected_pass = self.password.as_bytes();
        if username.len() != expected_user.len() || password.len() != expected_pass.len() {
            return false;
        }
        bool::from(username.ct_eq(expected_user) & password.ct_eq(expected_pass))
    }
}

#[cfg(test)]
mod debug_tests {
    use super::ProxyAuth;

    #[test]
    fn debug_output_redacts_local_proxy_credentials() {
        let output = format!("{:?}", ProxyAuth {
            username: "private-user".into(),
            password: "private-password".into(),
        });
        assert!(output.contains("<redacted>"));
        assert!(!output.contains("private-user"));
        assert!(!output.contains("private-password"));
    }
}
