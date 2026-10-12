use crate::app::{AppCommand, AppEvent, CLIPBOARD_PAUSE_DURATION};
use std::{ops::Sub, time::Instant};
use tokio::sync::mpsc;

const APP_NAME: &str = "UrlDebloater";
const TRAY_ID: &str = "urldebloater";
const TRAY_ICON_SIZE: u32 = 32;

pub async fn run(
    command_tx: mpsc::Sender<AppCommand>,
    event_rx: mpsc::Receiver<AppEvent>,
) -> anyhow::Result<()> {
    platform::run(command_tx, event_rx).await
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

#[cfg(target_os = "linux")]
mod platform {
    use super::{APP_NAME, TRAY_ICON_SIZE, TRAY_ID, pause_clipboard_title};
    use crate::app::{AppCommand, AppEvent};
    use anyhow::Context;
    use ksni::{
        Icon, OfflineReason, ToolTip, Tray, TrayMethods,
        menu::{MenuItem, StandardItem},
    };
    use log::{error, info, warn};
    use std::time::Instant;
    use tokio::sync::mpsc;

    const TRAY_ICON_ARGB: &[u8] = include_bytes!("tray_icon_32.argb");

    struct DebloaterTray {
        command_tx: mpsc::Sender<AppCommand>,
        pause_expires_at: Option<Instant>,
    }

    impl DebloaterTray {
        fn send(&self, command: AppCommand) {
            if let Err(err) = self.command_tx.try_send(command) {
                error!("Failed to send tray command: {err}");
            }
        }
    }

    impl Tray for DebloaterTray {
        const MENU_ON_ACTIVATE: bool = true;

        fn id(&self) -> String {
            TRAY_ID.into()
        }

        fn title(&self) -> String {
            APP_NAME.into()
        }

        fn tool_tip(&self) -> ToolTip {
            ToolTip {
                title: APP_NAME.into(),
                ..Default::default()
            }
        }

        fn icon_pixmap(&self) -> Vec<Icon> {
            vec![Icon {
                width: TRAY_ICON_SIZE as i32,
                height: TRAY_ICON_SIZE as i32,
                data: TRAY_ICON_ARGB.to_vec(),
            }]
        }

        fn menu(&self) -> Vec<MenuItem<Self>> {
            vec![
                StandardItem {
                    label: "Debloat clipboard".into(),
                    activate: Box::new(|tray: &mut Self| tray.send(AppCommand::ClearClipboard)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: pause_clipboard_title(self.pause_expires_at),
                    activate: Box::new(|tray: &mut Self| {
                        tray.send(AppCommand::ToggleClipboardDebloater)
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Reload config".into(),
                    activate: Box::new(|tray: &mut Self| tray.send(AppCommand::ReloadConfig)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Exit".into(),
                    activate: Box::new(|tray: &mut Self| tray.send(AppCommand::Exit)),
                    ..Default::default()
                }
                .into(),
            ]
        }

        fn watcher_offline(&self, reason: OfflineReason) -> bool {
            warn!(
                "StatusNotifierWatcher unavailable ({reason:?}); tray icon appears when a panel registers"
            );
            true
        }
    }

    pub async fn run(
        command_tx: mpsc::Sender<AppCommand>,
        mut event_rx: mpsc::Receiver<AppEvent>,
    ) -> anyhow::Result<()> {
        let tray = DebloaterTray {
            command_tx,
            pause_expires_at: None,
        };
        let handle = tray
            .assume_sni_available(true)
            .spawn()
            .await
            .context("spawn tray icon")?;
        info!("Tray icon service started");

        while let Some(event) = event_rx.recv().await {
            match event {
                AppEvent::ClipboardPauseProgressUpdate { expires_at } => {
                    handle
                        .update(|tray: &mut DebloaterTray| tray.pause_expires_at = expires_at)
                        .await;
                }
                AppEvent::Exit => break,
            }
        }

        handle.shutdown().await;
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use anyhow::Context;
    use log::{debug, error};
    use tokio::sync::mpsc;
    use tray_icon::{
        Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
        menu::{Menu, MenuEvent, MenuItem},
    };
    use winit::{
        application::ApplicationHandler,
        event::StartCause,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::WindowId,
    };

    use super::{APP_NAME, TRAY_ICON_SIZE, TRAY_ID, pause_clipboard_title};
    use crate::app::{AppCommand, AppEvent};

    const TRAY_ICON_RGBA: &[u8] = include_bytes!("tray_icon_32.rgba");

    pub fn run(
        command_tx: mpsc::Sender<AppCommand>,
        mut event_rx: mpsc::Receiver<AppEvent>,
    ) -> anyhow::Result<()> {
        let event_loop = EventLoop::<UserEvent>::with_user_event()
            .build()
            .context("create tray event loop")?;
        event_loop.set_control_flow(ControlFlow::Wait);

        install_tray_icon_forwarder(&event_loop);

        let menu_items = MenuItems::new();
        let menu_item_ids = menu_items.to_ids();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let id: &str = event.id().as_ref();
            if menu_item_ids.debloat_clipboard == id {
                try_send_command(&command_tx, AppCommand::ClearClipboard);
            } else if menu_item_ids.pause_clipboard_debloat == id {
                try_send_command(&command_tx, AppCommand::ToggleClipboardDebloater);
            } else if menu_item_ids.reload_config == id {
                try_send_command(&command_tx, AppCommand::ReloadConfig);
            } else if menu_item_ids.exit == id {
                try_send_command(&command_tx, AppCommand::Exit);
            } else {
                debug!("Ignoring tray menu event for '{id}'");
            }
        }));

        let proxy = event_loop.create_proxy();
        std::thread::Builder::new()
            .name("tray-events".into())
            .spawn(move || {
                while let Some(event) = event_rx.blocking_recv() {
                    if proxy.send_event(UserEvent::App(event)).is_err() {
                        break;
                    }
                }
            })
            .context("spawn tray event bridge")?;

        let menu = new_menu(&menu_items)?;
        let mut tray = TrayHandler {
            menu_items,
            menu: Some(menu),
            tray_icon: None,
        };
        event_loop.run_app(&mut tray).context("run tray event loop")
    }

    fn try_send_command(command_tx: &mpsc::Sender<AppCommand>, command: AppCommand) {
        if let Err(err) = command_tx.try_send(command) {
            error!("Failed to send tray command: {err}");
        }
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
        menu_items: MenuItems,
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

        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
            match event {
                UserEvent::TrayIcon(event) => debug!("Tray icon event: {event:?}"),
                UserEvent::App(AppEvent::ClipboardPauseProgressUpdate { expires_at }) => {
                    self.menu_items
                        .pause_clipboard_debloat
                        .set_text(pause_clipboard_title(expires_at));
                }
                UserEvent::App(AppEvent::Exit) => event_loop.exit(),
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

    enum UserEvent {
        TrayIcon(TrayIconEvent),
        App(AppEvent),
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
                pause_clipboard_debloat: MenuItem::new(&pause_clipboard_title(None), true, None),
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
        Icon::from_rgba(TRAY_ICON_RGBA.to_vec(), TRAY_ICON_SIZE, TRAY_ICON_SIZE)
            .context("create tray icon")
    }
}
