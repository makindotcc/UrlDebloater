use std::{ops::Sub, time::Instant};

use crate::app::{AppCommand, AppEvent, CLIPBOARD_PAUSE_DURATION};
use anyhow::Context;
use log::{debug, error};
use tokio::sync::mpsc;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

const APP_NAME: &str = "UrlDebloater";
const TRAY_ID: &str = "urldebloater";
const TRAY_ICON_SIZE: u32 = 32;
const TRAY_ICON_RGBA: &[u8] = include_bytes!("tray_icon_32.rgba");

pub fn run(
    command_tx: mpsc::Sender<AppCommand>,
    event_rx: mpsc::Receiver<AppEvent>,
) -> anyhow::Result<()> {
    let menu_items = MenuItems::new();
    let menu_item_ids = menu_items.to_ids();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let id: &str = event.id().as_ref();
        if menu_item_ids.debloat_clipboard == id {
            if let Err(err) = command_tx.try_send(AppCommand::ClearClipboard) {
                error!("Failed to send ClearClipboard event: {err}");
            }
        } else if menu_item_ids.pause_clipboard_debloat == id {
            if let Err(err) = command_tx.try_send(AppCommand::ToggleClipboardDebloater) {
                error!("Failed to send PauseClipboardDebloater event: {err}");
            }
        } else if menu_item_ids.reload_config == id {
            if let Err(err) = command_tx.try_send(AppCommand::ReloadConfig) {
                error!("Failed to send ReloadConfig event: {err}");
            }
        } else if menu_item_ids.exit == id {
            if let Err(err) = command_tx.try_send(AppCommand::Exit) {
                error!("Failed to send Exit event: {err}");
            }
        } else {
            debug!("Ignoring tray menu event for '{id}'");
        }
    }));
    platform::run(menu_items, event_rx)
}

fn create_tray(menu: Menu) -> anyhow::Result<TrayIcon> {
    TrayIconBuilder::new()
        .with_id(TRAY_ID)
        .with_menu(Box::new(menu))
        .with_tooltip(APP_NAME)
        .with_icon(create_icon()?)
        .with_title(APP_NAME)
        .build()
        .context("create tray icon")
}

fn create_icon() -> anyhow::Result<Icon> {
    Icon::from_rgba(TRAY_ICON_RGBA.to_vec(), TRAY_ICON_SIZE, TRAY_ICON_SIZE).context("create tray icon")
}

struct MenuItems {
    debloat_clipboard: MenuItem,
    pause_clipboard_debloat: MenuItem,
    reload_config: MenuItem,
    exit: MenuItem,
}

impl MenuItems {
    fn new() -> Self {
        Self {
            debloat_clipboard: MenuItem::new("Debloat clipboard", true, None),
            pause_clipboard_debloat: MenuItem::new(&Self::pause_clipboard_title(None), true, None),
            reload_config: MenuItem::new("Reload config", true, None),
            exit: MenuItem::new("Exit", true, None),
        }
    }

    fn to_ids(&self) -> MenuItemIds {
        MenuItemIds {
            debloat_clipboard: self.debloat_clipboard.id().0.clone(),
            pause_clipboard_debloat: self.pause_clipboard_debloat.id().0.clone(),
            reload_config: self.reload_config.id().0.clone(),
            exit: self.exit.id().0.clone(),
        }
    }

    fn pause_clipboard_title(expires_at: Option<Instant>) -> String {
        match expires_at {
            Some(expires_at) => format!(
                "Clipboard debloater is paused for {} seconds.",
                expires_at.sub(Instant::now()).as_secs().max(0)
            ),
            None => format!(
                "Pause clipboard debloater for {} seconds.",
                CLIPBOARD_PAUSE_DURATION.as_secs()
            ),
        }
    }
}

struct MenuItemIds {
    debloat_clipboard: String,
    pause_clipboard_debloat: String,
    reload_config: String,
    exit: String,
}

fn new_menu(menu_items: &MenuItems) -> anyhow::Result<Menu> {
    let menu = Menu::new();
    menu.append(&menu_items.debloat_clipboard)
        .context("build tray item 'Debloat clipboard'")?;
    menu.append(&menu_items.pause_clipboard_debloat)
        .context("build tray item 'Debloat pause'")?;
    menu.append(&menu_items.reload_config)
        .context("build tray item 'Reload config'")?;
    menu.append(&menu_items.exit)
        .context("build tray item 'Exit'")?;
    Ok(menu)
}

#[cfg(target_os = "linux")]
mod platform {
    use super::{create_tray, new_menu};
    use crate::{app::AppEvent, tray::MenuItems};
    use anyhow::Context;
    use tokio::sync::mpsc;

    pub fn run(
        menu_items: MenuItems,
        mut event_rx: mpsc::Receiver<AppEvent>,
    ) -> anyhow::Result<()> {
        gtk::init().context("initialize GTK for tray icon")?;

        let menu = new_menu(&menu_items).context("create tray menu")?;
        let _tray_icon = create_tray(menu).context("create tray icon")?;
        gtk::glib::spawn_future_local(async move {
            while let Some(event) = event_rx.recv().await {
                match event {
                    AppEvent::ClipboardPauseProgressUpdate { expires_at } => {
                        menu_items
                            .pause_clipboard_debloat
                            .set_text(MenuItems::pause_clipboard_title(expires_at));
                    }
                    AppEvent::Exit => {
                        gtk::main_quit();
                        break;
                    }
                }
            }
        });
        gtk::main();
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::{MenuItems, create_tray, install_menu_event_handler, new_menu};
    use crate::app::AppCommand;
    use anyhow::Context;
    use log::{debug, error};
    use tokio::sync::mpsc;
    use tray_icon::{TrayIcon, TrayIconEvent, menu::Menu};
    use winit::{
        application::ApplicationHandler,
        event::StartCause,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::WindowId,
    };

    pub fn run(
        menu_items: MenuItems,
        app_command_tx: mpsc::Sender<AppCommand>,
        app_event_rx: mpsc::Receiver<AppEvent>,
    ) -> anyhow::Result<()> {
        let event_loop = EventLoop::<UserEvent>::with_user_event()
            .build()
            .context("create tray event loop")?;
        event_loop.set_control_flow(ControlFlow::Wait);

        install_tray_icon_forwarder(&event_loop);
        install_menu_event_handler(&app_event_rx);

        let mut tray = TrayHandler {
            menu: Some(new_menu(&menu_items)?),
            tray_icon: None,
        };
        event_loop.run_app(&mut tray).context("run tray event loop")
    }

    fn install_tray_icon_forwarder(event_loop: &EventLoop<UserEvent>) {
        let proxy = event_loop.create_proxy();
        TrayIconEvent::set_event_handler(Some(move |event| {
            if let Err(err) = proxy.send_event(UserEvent::TrayIcon(event)) {
                error!("Failed to forward tray icon event: {err}");
            }
        }));
    }

    struct TrayHandler {
        menu: Option<Menu>,
        tray_icon: Option<TrayIcon>,
    }

    impl ApplicationHandler<UserEvent> for TrayHandler {
        fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
            if !matches!(cause, StartCause::Init) {
                return;
            }

            let Some(menu) = self.menu.take() else {
                return;
            };
            match create_tray(menu) {
                Ok(tray_icon) => self.tray_icon = Some(tray_icon),
                Err(err) => error!("Failed to initialize tray icon: {err:#}"),
            }
        }

        fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
            match event {
                UserEvent::TrayIcon(event) => debug!("Tray icon event: {event:?}"),
            }
        }

        fn resumed(&mut self, _event_loop: &ActiveEventLoop) {}

        fn window_event(
            &mut self,
            _event_loop: &ActiveEventLoop,
            _window_id: WindowId,
            _event: winit::event::WindowEvent,
        ) {
        }
    }

    #[derive(Debug)]
    enum UserEvent {
        TrayIcon(TrayIconEvent),
    }
}
