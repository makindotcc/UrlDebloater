#![cfg_attr(
    all(target_os = "windows", not(feature = "console")),
    windows_subsystem = "windows"
)]
use crate::clipboard_poller::ClipboardPoller;
#[cfg(all(target_os = "linux", feature = "config-gui"))]
use crate::gui::ConfigWindow;
use anyhow::Context;
use auto_launch::AutoLaunch;
use config::AppConfig;
#[cfg(feature = "tray")]
use log::info;
use log::{error, LevelFilter, Metadata, Record};
use std::env;
#[cfg(feature = "tray")]
use std::sync::mpsc;
#[cfg(all(target_os = "linux", feature = "config-gui"))]
use std::{cell::RefCell, rc::Rc};
use std::{
    io::{self, ErrorKind},
    sync::{Arc, RwLock, RwLockReadGuard},
    thread,
    time::{Duration, Instant},
};
use urlwasher::{text_washer::TextWasher, UrlWasher};

mod clipboard_poller;
mod config;
mod gui;

const APP_NAME: &str = "UrlDebloater";
const CONFIG_RELOAD_INTERVAL: Duration = Duration::from_secs(1);
#[cfg(feature = "tray")]
const CLIPBOARD_PAUSE_DURATION: Duration = Duration::from_secs(30);
#[cfg(feature = "tray")]
const TRAY_ICON_SIZE: i32 = 32;
#[cfg(feature = "tray")]
const TRAY_ICON_ARGB: &[u8] = include_bytes!("tray_icon_32.argb");
static LOGGER: SimpleLogger = SimpleLogger;

pub struct AppState {
    text_washer: TextWasher,
    config: AppConfig,
    #[cfg_attr(
        not(all(target_os = "linux", feature = "config-gui")),
        allow(dead_code)
    )]
    auto_launch: AutoLaunch,
}

impl AppState {
    pub fn new(config: AppConfig, auto_launch: AutoLaunch) -> Self {
        Self {
            text_washer: TextWasher {
                url_washer: UrlWasher::new(config.url_washer.clone()),
            },
            config,
            auto_launch,
        }
    }
}

#[derive(Clone)]
pub struct AppStateFlow {
    state: Arc<RwLock<AppState>>,
}

impl AppStateFlow {
    pub fn new(state: AppState) -> Self {
        Self {
            state: Arc::new(RwLock::new(state)),
        }
    }

    pub fn current(&self) -> RwLockReadGuard<'_, AppState> {
        self.state.read().expect("app state lock poisoned")
    }

    pub fn modify_config(&self, apply_changes: impl FnOnce(&mut AppConfig)) {
        let new_config = {
            let mut state = self.state.write().expect("app state lock poisoned");
            let mut new_config = state.config.clone();
            apply_changes(&mut new_config);
            state.text_washer = TextWasher {
                url_washer: UrlWasher::new(new_config.url_washer.clone()),
            };
            state.config = new_config.clone();
            new_config
        };
        if let Err(err) = config::save_to_file(&new_config) {
            error!("Could not save config: {err:?}");
        }
    }

    pub fn reload_config(&self, mut config: AppConfig) {
        let mut state = self.state.write().expect("app state lock poisoned");
        config.clipboard_patcher_paused_until = state.config.clipboard_patcher_paused_until;
        state.text_washer = TextWasher {
            url_washer: UrlWasher::new(config.url_washer.clone()),
        };
        state.config = config;
    }
}

const AUTOSTART_ARG: &str = "-autostart";

fn main() -> anyhow::Result<()> {
    init_logger();

    let started_from_autolaunch = env::args().skip(1).next() == Some(String::from(AUTOSTART_ARG));
    let (first_launch, config) = config::from_file()
        .map(|config| (false, config))
        .unwrap_or_else(|err| {
            let config_not_found = err
                .downcast_ref::<io::Error>()
                .is_some_and(|err| err.kind() == ErrorKind::NotFound);
            if !config_not_found {
                error!("Could not read config file: {err:?}. Using default...");
            }
            (config_not_found, AppConfig::default())
        });
    let auto_launch = {
        let app_path = env::current_exe().expect("Could not get current exe path");
        let app_path = app_path.to_str().expect("Invalid current exe path");
        AutoLaunch::new(APP_NAME, app_path, &[AUTOSTART_ARG] as &[&str])
    };
    if first_launch {
        auto_launch
            .enable()
            .expect("Could not enable auto launch on initial debloater startup");
    }
    let app_state = AppState::new(config, auto_launch);
    let app_state_flow = AppStateFlow::new(app_state);
    spawn_background_jobs(app_state_flow.clone());
    spawn_config_reloader(app_state_flow.clone());
    let open_config_window =
        !started_from_autolaunch && cfg!(all(target_os = "linux", feature = "config-gui"));
    run_gui(app_state_flow, open_config_window);
}

fn spawn_config_reloader(app_state_flow: AppStateFlow) {
    thread::Builder::new()
        .name("config-reloader".into())
        .spawn(move || {
            let mut last_modified = config::modified_time().ok();
            loop {
                thread::sleep(CONFIG_RELOAD_INTERVAL);

                let modified = match config::modified_time() {
                    Ok(modified) => modified,
                    Err(err) => {
                        if !err
                            .downcast_ref::<io::Error>()
                            .is_some_and(|err| err.kind() == ErrorKind::NotFound)
                        {
                            error!("Could not check config file changes: {err:?}");
                        }
                        last_modified = None;
                        continue;
                    }
                };

                if last_modified.is_some_and(|last_modified| last_modified >= modified) {
                    continue;
                }
                last_modified = Some(modified);

                match config::from_file() {
                    Ok(config) => {
                        app_state_flow.reload_config(config);
                        info!("Reloaded config from file");
                    }
                    Err(err) => error!("Could not reload config file: {err:?}"),
                }
            }
        })
        .expect("Could not spawn config reloader thread");
}

fn spawn_background_jobs(app_state_flow: AppStateFlow) {
    thread::Builder::new()
        .name("clipboard".into())
        .stack_size(128 * 1024)
        .spawn(move || loop {
            if let Err(err) = run_clipboard_patcher(&app_state_flow) {
                error!("Could not run clipboard patcher: {err:?}.");
                thread::sleep(Duration::from_secs(5));
            }
        })
        .expect("Could not spawn clipboard thread");
}

fn run_clipboard_patcher(app_state_flow: &AppStateFlow) -> anyhow::Result<()> {
    let runtime = async_runtime()?;
    let mut arboard = arboard::Clipboard::new().context("Could not create clipboard accessor")?;
    let mut clipboard_poller = ClipboardPoller::new();
    loop {
        {
            let app_state = app_state_flow.current();
            if !app_state.config.enable_clipboard_patcher {
                drop(app_state);
                thread::sleep(Duration::from_millis(500));
                continue;
            }
            if let Some(paused_until) = app_state.config.clipboard_patcher_paused_until {
                if paused_until > Instant::now() {
                    let remaining_pause = paused_until.duration_since(Instant::now());
                    drop(app_state);
                    thread::sleep(remaining_pause.min(Duration::from_millis(500)));
                    continue;
                }
            }
        }

        let dirty_text = clipboard_poller
            .poll(&mut arboard)
            .context("Could not poll clipboard")?;
        let app_state = app_state_flow.current();
        let clean_text = runtime.block_on(app_state.text_washer.wash(dirty_text));
        if clean_text != dirty_text
            && arboard
                .get_text()
                .is_ok_and(|current_clipboard| dirty_text == current_clipboard)
        {
            if let Err(err) = clipboard_poller.set_text(&mut arboard, clean_text) {
                error!("Could not copy cleaned text to clipboard: {err:?}");
            }
        }
    }
}

fn run_gui(app_state_flow: AppStateFlow, open_config_window: bool) -> ! {
    #[cfg(not(feature = "tray"))]
    {
        let _ = app_state_flow;
        if open_config_window {
            error!("Configuration window is not available when the tray feature is disabled");
        }
        loop {
            thread::sleep(Duration::from_secs(60));
        }
    }

    #[cfg(feature = "tray")]
    run_tray(app_state_flow, open_config_window)
}

#[cfg(feature = "tray")]
fn run_tray(app_state_flow: AppStateFlow, open_config_window: bool) -> ! {
    use ksni::blocking::TrayMethods;

    let (tray_event_tx, tray_event_rx) = mpsc::channel();
    let _tray_handle = UrlDebloaterTray {
        app_state_flow: app_state_flow.clone(),
        event_tx: tray_event_tx,
    }
    .assume_sni_available(true)
    .spawn()
    .expect("Could not create tray icon");

    #[cfg(all(target_os = "linux", feature = "config-gui"))]
    {
        let mut tray_event_rx = tray_event_rx;
        let _fltk_app = fltk::app::App::default().with_scheme(fltk::app::Scheme::Gtk);

        let config_window = Rc::new(RefCell::new(None));
        if open_config_window {
            open_config_window_once(&app_state_flow, &config_window);
        }

        loop {
            process_tray_events(&app_state_flow, &config_window, &mut tray_event_rx);
            if let Err(err) = fltk::app::wait_for(0.1) {
                error!("Could not run FLTK event loop: {err}");
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    #[cfg(not(all(target_os = "linux", feature = "config-gui")))]
    {
        if open_config_window {
            error!("Configuration window is not available when the config-gui feature is disabled");
        }
        while let Ok(tray_event) = tray_event_rx.recv() {
            process_headless_tray_event(&app_state_flow, tray_event);
        }
        std::process::exit(0);
    }
}

#[cfg(all(target_os = "linux", feature = "tray", feature = "config-gui"))]
fn process_tray_events(
    app_state_flow: &AppStateFlow,
    config_window: &Rc<RefCell<Option<ConfigWindow>>>,
    tray_event_rx: &mut mpsc::Receiver<TrayEvent>,
) {
    while let Ok(tray_event) = tray_event_rx.try_recv() {
        match tray_event {
            TrayEvent::OpenConfig => open_config_window_once(app_state_flow, config_window),
            tray_event => process_headless_tray_event(app_state_flow, tray_event),
        }
    }
}

#[cfg(all(target_os = "linux", feature = "tray", feature = "config-gui"))]
fn open_config_window_once(
    app_state_flow: &AppStateFlow,
    config_window: &Rc<RefCell<Option<ConfigWindow>>>,
) {
    let mut current_window = config_window.borrow_mut();
    if current_window.is_none() {
        let config_window_slot = Rc::clone(config_window);
        *current_window = Some(ConfigWindow::new(app_state_flow.clone(), move || {
            *config_window_slot.borrow_mut() = None;
        }));
    }
    if let Some(config_window) = current_window.as_ref() {
        config_window.show();
    }
}

#[cfg(feature = "tray")]
#[derive(Clone)]
struct UrlDebloaterTray {
    app_state_flow: AppStateFlow,
    event_tx: mpsc::Sender<TrayEvent>,
}

#[cfg(feature = "tray")]
impl UrlDebloaterTray {
    fn send_event(&self, tray_event: TrayEvent) {
        if let Err(err) = self.event_tx.send(tray_event) {
            error!("Could not send tray event: {err:?}");
        }
    }
}

#[cfg(feature = "tray")]
impl ksni::Tray for UrlDebloaterTray {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "urldebloater".into()
    }

    fn title(&self) -> String {
        APP_NAME.into()
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        vec![ksni::Icon {
            width: TRAY_ICON_SIZE,
            height: TRAY_ICON_SIZE,
            data: TRAY_ICON_ARGB.to_vec(),
        }]
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: APP_NAME.into(),
            description: "Remove tracking parameters from URLs.".into(),
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::{CheckmarkItem, StandardItem};

        let app_state = self.app_state_flow.current();
        let (pause_enabled, pause_checked, pause_label) = pause_clipboard_menu_state(&app_state);

        vec![
            StandardItem {
                label: "Debloat current clipboard".into(),
                icon_name: "edit-clear-symbolic".into(),
                activate: Box::new(|tray: &mut Self| tray.send_event(TrayEvent::WashClipboard)),
                ..Default::default()
            }
            .into(),
            CheckmarkItem {
                label: pause_label,
                enabled: pause_enabled,
                checked: pause_checked,
                activate: Box::new(|tray: &mut Self| {
                    tray.send_event(TrayEvent::PauseClipboardWasher)
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Open configuration".into(),
                icon_name: "preferences-system".into(),
                enabled: cfg!(all(target_os = "linux", feature = "config-gui")),
                activate: Box::new(|tray: &mut Self| tray.send_event(TrayEvent::OpenConfig)),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(feature = "tray")]
fn process_headless_tray_event(app_state_flow: &AppStateFlow, tray_event: TrayEvent) {
    match tray_event {
        TrayEvent::OpenConfig => {
            error!("Configuration window is not available on this platform yet");
        }
        TrayEvent::WashClipboard => {
            info!("Debloating clipboard from tray...");
            let app_state_flow = app_state_flow.clone();
            thread::spawn(move || {
                if let Err(err) = tray_wash_clipboard(&app_state_flow) {
                    error!("Could not wash clipboard from tray: {err:?}");
                }
            });
        }
        TrayEvent::PauseClipboardWasher => {
            app_state_flow.modify_config(|config| {
                if config.clipboard_patcher_paused_until.is_some() {
                    config.clipboard_patcher_paused_until = None;
                } else {
                    config.clipboard_patcher_paused_until =
                        Some(Instant::now() + CLIPBOARD_PAUSE_DURATION);
                }
            });
        }
    }
}

#[cfg(feature = "tray")]
#[derive(Clone, Copy)]
enum TrayEvent {
    OpenConfig,
    WashClipboard,
    PauseClipboardWasher,
}

#[cfg(feature = "tray")]
fn pause_clipboard_menu_state(app_state: &AppState) -> (bool, bool, String) {
    if app_state.config.enable_clipboard_patcher {
        match app_state.config.clipboard_patcher_paused_until {
            Some(paused_until) if paused_until > Instant::now() => (
                true,
                true,
                format!(
                    "Clipboard debloater paused for {} sec.",
                    paused_until.duration_since(Instant::now()).as_secs()
                ),
            ),
            _ => (
                true,
                false,
                format!(
                    "Pause clipboard debloater for {} sec.",
                    CLIPBOARD_PAUSE_DURATION.as_secs()
                ),
            ),
        }
    } else {
        (
            false,
            false,
            String::from("Clipboard debloater disabled in config"),
        )
    }
}

#[cfg(feature = "tray")]
fn tray_wash_clipboard(app_state_flow: &AppStateFlow) -> anyhow::Result<()> {
    let runtime = async_runtime()?;
    let mut clipboard = arboard::Clipboard::new().context("Could not create clipboard accessor")?;
    let clipboard_text = clipboard
        .get_text()
        .context("Could not get text from clipboard")?;
    let app_state = app_state_flow.current();
    clipboard
        .set_text(runtime.block_on(app_state.text_washer.wash(&clipboard_text)))
        .context("Could not copy clean text to clipboard")?;
    Ok(())
}

fn async_runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Could not create async runtime")
}

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &Record<'_>) {
        if self.enabled(record.metadata()) {
            eprintln!("{}: {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

fn init_logger() {
    log::set_logger(&LOGGER).expect("Could not initialize logger");
    log::set_max_level(LevelFilter::Info);
}
