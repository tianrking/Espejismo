use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProxyAuth {
    pub username: String,
    pub password: String,
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
