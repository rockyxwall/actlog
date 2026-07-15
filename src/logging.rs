use crate::db;
use anyhow::{Context, Result};
use simplelog::*;
use std::fs::File;

pub fn init_logging() -> Result<()> {
    let data_dir = db::get_data_dir().context("Failed to get data directory for logging")?;
    let log_path = data_dir.join("actlog.log");

    // Truncate log file on startup to avoid unbounded growth
    let log_file = File::create(&log_path).context("Failed to create log file")?;

    let config = Config::default();
    let mut loggers: Vec<Box<dyn SharedLogger>> = Vec::new();

    // Always log to file
    loggers.push(WriteLogger::new(
        LevelFilter::Info,
        config.clone(),
        log_file,
    ));

    // Log to console in debug builds
    if cfg!(debug_assertions) {
        loggers.push(TermLogger::new(
            LevelFilter::Debug,
            config,
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ));
    }

    CombinedLogger::init(loggers).context("Failed to initialize combined logger")?;
    Ok(())
}
