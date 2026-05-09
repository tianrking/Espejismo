use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::prelude::*;

use crate::config::{LogConfig, LogFormat};

pub struct LogGuard {
    _guard: Option<WorkerGuard>,
}

pub fn init_logging(config: &LogConfig) -> Result<LogGuard> {
    let filter = EnvFilter::try_new(&config.level)
        .or_else(|_| EnvFilter::try_new("info"))
        .context("create log filter")?;

    if let Some(path) = &config.file {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("create log directory {}", parent.display()))?;
            }
        }
        let directory = path.parent().unwrap_or_else(|| Path::new("."));
        let filename = path
            .file_name()
            .context("logging.file must include a file name")?;
        let appender = tracing_appender::rolling::never(directory, filename);
        let (writer, guard) = tracing_appender::non_blocking(appender);
        init_with_writer(config, filter, writer)?;
        Ok(LogGuard {
            _guard: Some(guard),
        })
    } else {
        init_with_writer(config, filter, std::io::stderr)?;
        Ok(LogGuard { _guard: None })
    }
}

fn init_with_writer<W>(config: &LogConfig, filter: EnvFilter, writer: W) -> Result<()>
where
    W: for<'writer> MakeWriter<'writer> + Send + Sync + 'static,
{
    match config.format {
        LogFormat::Compact => tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .compact()
                    .with_ansi(config.ansi)
                    .with_writer(writer),
            )
            .try_init()
            .context("initialize compact logger"),
        LogFormat::Pretty => tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .pretty()
                    .with_ansi(config.ansi)
                    .with_writer(writer),
            )
            .try_init()
            .context("initialize pretty logger"),
        LogFormat::Json => tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_ansi(false)
                    .with_current_span(false)
                    .with_span_list(false)
                    .with_writer(writer),
            )
            .try_init()
            .context("initialize json logger"),
    }
}
