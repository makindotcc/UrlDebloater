mod app;
mod clipboard;
mod config_integration;
mod tray;

use anyhow::Context;
use log::info;
use std::fs::{File, OpenOptions, TryLockError};

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let Some(_instance_lock) = instance_lock()? else {
        info!("UrlDebloater is already running");
        return Ok(());
    };

    info!("Starting application");

    app::run()
}

fn instance_lock() -> anyhow::Result<Option<File>> {
    let dir = urldebloater_desktop_config::config_dir();
    std::fs::create_dir_all(&dir).context("create configuration directory")?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(dir.join("daemon.lock"))
        .context("open daemon lock file")?;

    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(err)) => Err(err).context("lock daemon lock file"),
    }
}
