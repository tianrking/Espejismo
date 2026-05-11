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
        (Some(current), Some(latest)) => latest > current,
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
