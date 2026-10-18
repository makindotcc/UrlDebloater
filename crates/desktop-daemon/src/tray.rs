use crate::app::CLIPBOARD_PAUSE_DURATION;
use std::{ops::Sub, time::Instant};

const APP_NAME: &str = "UrlDebloater";
const TRAY_ID: &str = "urldebloater";
const TRAY_ICON_SIZE: u32 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ClipboardMenuVisibility {
    debloat_clipboard: bool,
    pause_clipboard_debloater: bool,
}

fn clipboard_menu_visibility(clipboard_debloater_enabled: bool) -> ClipboardMenuVisibility {
    ClipboardMenuVisibility {
        debloat_clipboard: !clipboard_debloater_enabled,
        pause_clipboard_debloater: clipboard_debloater_enabled,
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

#[cfg(target_os = "linux")]
pub(crate) mod platform {
    use super::{
        APP_NAME, TRAY_ICON_SIZE, TRAY_ID, clipboard_menu_visibility, pause_clipboard_title,
    };
    use crate::app::{AppCommand, AppEvent};
    use anyhow::Context;
    use ksni::{
        Icon, OfflineReason, ToolTip, Tray, TrayMethods,
        menu::{MenuItem, StandardItem},
    };
    use log::{error, info, warn};
    use std::time::Instant;
    use tokio::sync::mpsc;

    const TRAY_ICON_ARGB: &[u8] = include_bytes!(concat!(
        env!("CARGO_WORKSPACE_DIR"),
        "assets/icon/tray_32.argb"
    ));

    struct DebloaterTray {
        command_tx: mpsc::Sender<AppCommand>,
        clipboard_debloater_enabled: bool,
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
            let visibility = clipboard_menu_visibility(self.clipboard_debloater_enabled);
            vec![
                StandardItem {
                    label: "Debloat clipboard".into(),
                    visible: visibility.debloat_clipboard,
                    activate: Box::new(|tray: &mut Self| tray.send(AppCommand::ClearClipboard)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: pause_clipboard_title(self.pause_expires_at),
                    visible: visibility.pause_clipboard_debloater,
                    activate: Box::new(|tray: &mut Self| {
                        tray.send(AppCommand::ToggleClipboardDebloater)
                    }),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: "Settings...".into(),
                    activate: Box::new(|tray: &mut Self| tray.send(AppCommand::OpenConfigGui)),
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
        clipboard_debloater_enabled: bool,
    ) -> anyhow::Result<()> {
        let tray = DebloaterTray {
            command_tx,
            clipboard_debloater_enabled,
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
                AppEvent::ClipboardStatusChanged {
                    clipboard_debloater_enabled,
                    pause_expires_at,
                } => {
                    handle
                        .update(|tray: &mut DebloaterTray| {
                            tray.clipboard_debloater_enabled = clipboard_debloater_enabled;
                            tray.pause_expires_at = pause_expires_at;
                        })
                        .await;
                }
                AppEvent::Exit => break,
            }
        }

        handle.shutdown().await;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn menu_entries(clipboard_debloater_enabled: bool) -> Vec<(String, bool)> {
            let (command_tx, _command_rx) = mpsc::channel(1);
            let tray = DebloaterTray {
                command_tx,
                clipboard_debloater_enabled,
                pause_expires_at: None,
            };
            tray.menu()
                .into_iter()
                .filter_map(|item| match item {
                    MenuItem::Standard(item) => Some((item.label, item.visible)),
                    _ => None,
                })
                .collect()
        }

        fn is_visible(entries: &[(String, bool)], label: &str) -> bool {
            entries
                .iter()
                .any(|(entry_label, visible)| entry_label == label && *visible)
        }

        #[test]
        fn clipboard_entries_follow_auto_debloater_state() {
            let enabled = menu_entries(true);
            assert!(!is_visible(&enabled, "Debloat clipboard"));
            assert!(
                enabled
                    .iter()
                    .any(|(label, visible)| *visible
                        && label.starts_with("Pause clipboard debloater"))
            );

            let disabled = menu_entries(false);
            assert!(is_visible(&disabled, "Debloat clipboard"));
            assert!(
                !disabled
                    .iter()
                    .any(|(label, visible)| *visible
                        && label.starts_with("Pause clipboard debloater"))
            );
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) mod platform {
    use super::{
        APP_NAME, TRAY_ICON_SIZE, TRAY_ID, clipboard_menu_visibility, pause_clipboard_title,
    };
    use crate::app::{AppCommand, AppEvent};
    use anyhow::Context;
    use log::{debug, error};
    use std::time::Instant;
    use tokio::sync::mpsc;
    use tray_icon::{
        Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
        menu::{Menu, MenuEvent, MenuItem},
    };
    #[cfg(target_os = "macos")]
    use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
    use winit::{
        application::ApplicationHandler,
        event::StartCause,
        event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
        window::WindowId,
    };

    const TRAY_ICON_RGBA: &[u8] = include_bytes!(concat!(
        env!("CARGO_WORKSPACE_DIR"),
        "assets/icon/tray_32.rgba"
    ));

    pub fn run(
        command_tx: mpsc::Sender<AppCommand>,
        mut event_rx: mpsc::Receiver<AppEvent>,
        clipboard_debloater_enabled: bool,
    ) -> anyhow::Result<()> {
        let mut event_loop_builder = EventLoop::<UserEvent>::with_user_event();
        #[cfg(target_os = "macos")]
        event_loop_builder.with_activation_policy(ActivationPolicy::Accessory);
        let event_loop = event_loop_builder
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
            } else if menu_item_ids.settings == id {
                try_send_command(&command_tx, AppCommand::OpenConfigGui);
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

        let menu = new_menu(&menu_items, clipboard_debloater_enabled)?;
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

    impl TrayHandler {
        fn apply_clipboard_status(
            &mut self,
            clipboard_debloater_enabled: bool,
            pause_expires_at: Option<Instant>,
        ) {
            self.menu_items
                .pause_clipboard_debloat
                .set_text(pause_clipboard_title(pause_expires_at));
            if let Some(menu) = &self.menu
                && let Err(err) = self
                    .menu_items
                    .sync_clipboard_items(menu, clipboard_debloater_enabled)
            {
                error!("Failed to update tray menu: {err:#}");
            }
        }
    }

    impl ApplicationHandler<UserEvent> for TrayHandler {
        fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
            if !matches!(cause, StartCause::Init) {
                return;
            }

            let Some(menu) = &self.menu else {
                return;
            };
            match create_tray(menu.clone()) {
                Ok(tray_icon) => self.tray_icon = Some(tray_icon),
                Err(err) => error!("Failed to initialize tray icon: {err:#}"),
            }
        }

        fn user_event(&mut self, event_loop: &ActiveEventLoop, event: UserEvent) {
            match event {
                UserEvent::TrayIcon(event) => debug!("Tray icon event: {event:?}"),
                UserEvent::App(AppEvent::ClipboardStatusChanged {
                    clipboard_debloater_enabled,
                    pause_expires_at,
                }) => {
                    self.apply_clipboard_status(clipboard_debloater_enabled, pause_expires_at);
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
        settings: MenuItem,
        reload_config: MenuItem,
        exit: MenuItem,
    }

    impl MenuItems {
        fn new() -> Self {
            Self {
                debloat_clipboard: MenuItem::new("Debloat clipboard", true, None),
                pause_clipboard_debloat: MenuItem::new(&pause_clipboard_title(None), true, None),
                settings: MenuItem::new("Settings...", true, None),
                reload_config: MenuItem::new("Reload config", true, None),
                exit: MenuItem::new("Exit", true, None),
            }
        }

        fn sync_clipboard_items(
            &self,
            menu: &Menu,
            clipboard_debloater_enabled: bool,
        ) -> anyhow::Result<()> {
            let visibility = clipboard_menu_visibility(clipboard_debloater_enabled);
            sync_menu_item(
                menu,
                &self.debloat_clipboard,
                visibility.debloat_clipboard,
                0,
            )?;
            sync_menu_item(
                menu,
                &self.pause_clipboard_debloat,
                visibility.pause_clipboard_debloater,
                0,
            )
        }

        fn to_ids(&self) -> MenuItemIds {
            MenuItemIds {
                debloat_clipboard: self.debloat_clipboard.id().0.clone(),
                pause_clipboard_debloat: self.pause_clipboard_debloat.id().0.clone(),
                settings: self.settings.id().0.clone(),
                reload_config: self.reload_config.id().0.clone(),
                exit: self.exit.id().0.clone(),
            }
        }
    }

    struct MenuItemIds {
        debloat_clipboard: String,
        pause_clipboard_debloat: String,
        settings: String,
        reload_config: String,
        exit: String,
    }

    fn new_menu(menu_items: &MenuItems, clipboard_debloater_enabled: bool) -> anyhow::Result<Menu> {
        let menu = Menu::new();
        menu_items
            .sync_clipboard_items(&menu, clipboard_debloater_enabled)
            .context("build clipboard tray items")?;
        menu.append(&menu_items.settings)
            .context("build tray item 'Settings'")?;
        menu.append(&menu_items.reload_config)
            .context("build tray item 'Reload config'")?;
        menu.append(&menu_items.exit)
            .context("build tray item 'Exit'")?;
        Ok(menu)
    }

    fn sync_menu_item(
        menu: &Menu,
        item: &MenuItem,
        visible: bool,
        position: usize,
    ) -> anyhow::Result<()> {
        let present = menu.items().iter().any(|current| current.id() == item.id());
        match (present, visible) {
            (true, false) => menu.remove(item).context("remove tray menu item"),
            (false, true) => menu.insert(item, position).context("insert tray menu item"),
            _ => Ok(()),
        }
    }

    fn create_tray(menu: Menu) -> anyhow::Result<TrayIcon> {
        TrayIconBuilder::new()
            .with_id(TRAY_ID)
            .with_menu(Box::new(menu))
            .with_tooltip(APP_NAME)
            .with_icon(create_icon()?)
            .with_icon_as_template(true)
            .build()
            .context("create tray icon")
    }

    fn create_icon() -> anyhow::Result<Icon> {
        Icon::from_rgba(TRAY_ICON_RGBA.to_vec(), TRAY_ICON_SIZE, TRAY_ICON_SIZE)
            .context("create tray icon")
    }
}
