use crate::app::AppCommand;
use log::error;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{process::Command, sync::mpsc};

const RELOAD_DEBOUNCE: Duration = Duration::from_millis(300);
const CONFIG_GUI_BIN: &str = "urldebloater-desktop-config-gui";

pub fn open_gui(running: Arc<AtomicBool>) {
    let Some(exe) = config_gui_path() else {
        error!("Configuration window not found next to the application binary");
        return;
    };
    let path = urldebloater_desktop_config::config_path();
    match Command::new(&exe).arg("--config").arg(&path).spawn() {
        Ok(mut child) => {
            tokio::spawn(async move {
                match child.wait().await {
                    Ok(status) if !status.success() => {
                        error!("Configuration window exited with {status}");
                    }
                    Ok(_) => {}
                    Err(err) => error!("Configuration window failed: {err}"),
                }
                running.store(false, Ordering::Relaxed);
            });
        }
        Err(err) => {
            running.store(false, Ordering::Relaxed);
            error!("Failed to start {}: {err}", exe.display());
        }
    }
}

fn config_gui_path() -> Option<std::path::PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let name = if cfg!(windows) {
        format!("{CONFIG_GUI_BIN}.exe")
    } else {
        CONFIG_GUI_BIN.to_owned()
    };
    let path = dir.join(name);
    path.is_file().then_some(path)
}

pub fn watch(command_tx: mpsc::Sender<AppCommand>) -> Option<RecommendedWatcher> {
    let path = urldebloater_desktop_config::config_path();
    let dir = path.parent()?.to_path_buf();
    let watched = path;
    let last_reload = Arc::new(Mutex::new(None::<Instant>));
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        let Ok(event) = event else {
            return;
        };
        if !event.paths.iter().any(|event_path| *event_path == watched) {
            return;
        }
        // Atomic replacement emits several events; one reload is enough.
        let mut last = last_reload.lock().expect("config reload lock");
        if last.is_some_and(|at| at.elapsed() < RELOAD_DEBOUNCE) {
            return;
        }
        *last = Some(Instant::now());
        drop(last);
        if let Err(err) = command_tx.try_send(AppCommand::ReloadConfig) {
            error!("Failed to queue config reload: {err}");
        }
    })
    .map_err(|err| error!("Failed to watch {}: {err}", dir.display()))
    .ok()?;
    watcher
        .watch(&dir, RecursiveMode::NonRecursive)
        .map_err(|err| error!("Failed to watch {}: {err}", dir.display()))
        .ok()?;
    Some(watcher)
}
