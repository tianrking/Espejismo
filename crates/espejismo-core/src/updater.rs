//! Release metadata lookup helpers for optional startup update checks.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_RELEASE_URL: &str =
    "https://api.github.com/repos/tianrking/Espejismo/releases/latest";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub release_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseResponse {
    #[serde(alias = "latest_version", alias = "version")]
    tag_name: String,
    #[serde(default, alias = "url")]
    html_url: Option<String>,
}

pub fn default_release_url() -> &'static str {
    DEFAULT_RELEASE_URL
}

pub fn check_for_update(current_version: &str, release_url: Option<&str>) -> Result<UpdateInfo> {
    let url = release_url.unwrap_or(DEFAULT_RELEASE_URL);
    let response: ReleaseResponse = ureq::get(url)
        .set(
            "User-Agent",
            concat!("espejismo/", env!("CARGO_PKG_VERSION")),
        )
        .set("Accept", "application/json")
        .call()
        .with_context(|| format!("request update metadata from {url}"))?
        .into_json()
        .context("parse update metadata JSON")?;
    Ok(update_info_from_release(current_version, response))
}

fn update_info_from_release(current_version: &str, response: ReleaseResponse) -> UpdateInfo {
    let latest = response.tag_name.trim().to_string();
    UpdateInfo {
        current_version: current_version.to_string(),
        latest_version: latest.clone(),
        update_available: is_newer_version(current_version, &latest),
        release_url: response.html_url,
    }
}

fn is_newer_version(current: &str, latest: &str) -> bool {
    let current = normalize_version(current);
    let latest = normalize_version(latest);
    match (
        parse_numeric_version(&current),
        parse_numeric_version(&latest),
    ) {
        (Some(mut current), Some(mut latest)) => {
            // Tags often omit trailing zero components (1.2 and 1.2.0 are
            // the same release). Ignore those before comparing components.
            while current.last() == Some(&0) {
                current.pop();
            }
            while latest.last() == Some(&0) {
                latest.pop();
            }
            latest > current
        }
        _ => latest != current,
    }
}

fn normalize_version(version: &str) -> String {
    version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('V')
        .to_string()
}

fn parse_numeric_version(version: &str) -> Option<Vec<u64>> {
    let mut out = Vec::new();
    for part in version.split('.') {
        let digits = part
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        if digits.is_empty() {
            return None;
        }
        out.push(digits.parse().ok()?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_semver_like_tags() {
        assert!(is_newer_version("0.0.1", "v0.0.2"));
        assert!(is_newer_version("1.9.0", "1.10.0"));
        assert!(!is_newer_version("1.10.0", "1.9.9"));
        assert!(!is_newer_version("v1.0.0", "1.0.0"));
    }

    #[test]
    fn compares_version_component_boundaries() {
        assert!(!is_newer_version("1.2", "1.2.0"));
        assert!(!is_newer_version("1.2.0.0", "1.2"));
        assert!(is_newer_version("1.2.0", "1.2.1"));
        assert!(is_newer_version("1.2.9", "1.3"));
        assert!(!is_newer_version("1.2.0", "1.2.0"));
        assert!(!is_newer_version("v1.2.0", " V1.2.0 "));
    }

    #[test]
    fn malformed_numeric_component_falls_back_to_tag_comparison() {
        assert!(is_newer_version("1.2.0", "1.2.x"));
        assert!(!is_newer_version("1.2.x", "1.2.x"));
        assert!(is_newer_version("1.2.18446744073709551616", "1.2.0"));
    }

    #[test]
    fn parses_documented_release_field_aliases() {
        for key in ["tag_name", "latest_version", "version"] {
            let json =
                format!("{{\"{key}\":\"v2.0.0\",\"html_url\":\"https://example.test/release\"}}");
            let response: ReleaseResponse = serde_json::from_str(&json).unwrap();
            let info = update_info_from_release("1.9.9", response);
            assert!(info.update_available, "field {key}");
            assert_eq!(info.latest_version, "v2.0.0");
            assert_eq!(
                info.release_url.as_deref(),
                Some("https://example.test/release")
            );
        }
    }

    #[test]
    fn missing_optional_release_url_is_accepted() {
        let response: ReleaseResponse = serde_json::from_str(r#"{"version":"1.0.0"}"#).unwrap();
        assert_eq!(response.html_url, None);
    }

    #[test]
    fn maps_release_metadata_to_update_info() {
        let info = update_info_from_release(
            "0.0.1",
            ReleaseResponse {
                tag_name: "v0.0.2".to_string(),
                html_url: Some("https://example.test/releases/v0.0.2".to_string()),
            },
        );
        assert!(info.update_available);
        assert_eq!(info.latest_version, "v0.0.2");
    }
}
