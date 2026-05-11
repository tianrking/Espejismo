use std::path::PathBuf;

use anyhow::Result;

use crate::config::{LogConfig, LogFormat};
use crate::updater::check_for_update;

#[derive(Clone, Debug, Default)]
pub struct LogOverrides {
    pub level: Option<String>,
    pub format: Option<String>,
    pub file: Option<PathBuf>,
    pub no_ansi: bool,
}

pub fn print_update_check(update_url: Option<&str>) -> Result<()> {
    let info = check_for_update(env!("CARGO_PKG_VERSION"), update_url)?;
    if info.update_available {
        println!(
            "update available: {} -> {}",
            info.current_version, info.latest_version
        );
        if let Some(url) = info.release_url {
            println!("release: {url}");
        }
    } else {
        println!("up to date: {}", info.current_version);
    }
    Ok(())
}

pub fn apply_log_overrides(config: &mut LogConfig, overrides: &LogOverrides) -> Result<()> {
    if let Some(level) = &overrides.level {
        config.level = level.clone();
    }
    if let Some(format) = &overrides.format {
        config.format = parse_log_format(format)?;
    }
    if let Some(file) = &overrides.file {
        config.file = Some(file.clone());
    }
    if overrides.no_ansi {
        config.ansi = false;
    }
    Ok(())
}

pub fn parse_log_format(format: &str) -> Result<LogFormat> {
    match format {
        "compact" => Ok(LogFormat::Compact),
        "pretty" => Ok(LogFormat::Pretty),
        "json" => Ok(LogFormat::Json),
        _ => anyhow::bail!("log format must be compact, pretty, or json"),
    }
}

pub fn report_config_check(warnings: Vec<String>, errors: Vec<String>) -> Result<()> {
    for warning in &warnings {
        println!("WARN {warning}");
    }
    for error in &errors {
        println!("ERROR {error}");
    }
    if errors.is_empty() {
        println!("config check passed");
        Ok(())
    } else {
        anyhow::bail!("config check failed with {} error(s)", errors.len())
    }
}

#[cfg(test)]
mod tests {
    use super::{apply_log_overrides, parse_log_format, LogOverrides};
    use crate::config::{LogConfig, LogFormat};

    #[test]
    fn parse_log_format_rejects_unknown_value() {
        assert!(parse_log_format("yaml").is_err());
        assert!(matches!(parse_log_format("json").unwrap(), LogFormat::Json));
    }

    #[test]
    fn log_overrides_update_selected_fields() {
        let mut config = LogConfig::default();
        apply_log_overrides(
            &mut config,
            &LogOverrides {
                level: Some("debug".to_string()),
                format: Some("json".to_string()),
                no_ansi: true,
                ..LogOverrides::default()
            },
        )
        .unwrap();

        assert_eq!(config.level, "debug");
        assert!(matches!(config.format, LogFormat::Json));
        assert!(!config.ansi);
    }
}
