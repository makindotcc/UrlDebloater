use crate::app::CLIPBOARD_PAUSE_DURATION;
use crate::update::UpdateStatus;
use std::{ops::Sub, time::Instant};

const APP_NAME: &str = "UrlDebloater";
const TRAY_ID: &str = "urldebloater";
const TRAY_ICON_SIZE: u32 = 32;

fn update_label(status: &UpdateStatus) -> String {
    match status {
        UpdateStatus::Available { version } => format!("Update available: v{version}"),
        UpdateStatus::Downloading {
            downloaded, total, ..
        } => match total.filter(|total| *total > 0) {
            Some(total) => format!(
                "Updating… {}%",
                (downloaded.saturating_mul(100) / total).min(100)
            ),
            None => String::from("Updating…"),
        },
        UpdateStatus::Installing { .. } => String::from("Updating…"),
        UpdateStatus::Failed {
            version: Some(version),
            ..
        } => format!("Update available: v{version} (retry)"),
        UpdateStatus::Failed { version: None, .. } => String::from("Update failed - retry"),
        UpdateStatus::Idle | UpdateStatus::Checking => String::from("Update available"),
    }
}

fn update_visible(status: &UpdateStatus) -> bool {
    status.has_update()
}

fn update_enabled(status: &UpdateStatus) -> bool {
    matches!(
        status,
        UpdateStatus::Available { .. }
            | UpdateStatus::Failed {
                version: Some(_),
                ..
            }
    )
}

fn check_label(status: &UpdateStatus) -> &'static str {
    match status {
        UpdateStatus::Checking => "Checking for updates…",
        UpdateStatus::Failed { version: None, .. } => "Update failed - retry",
        _ => "Check for updates",
    }
}

fn check_enabled(status: &UpdateStatus) -> bool {
    !status.is_busy() && !status.has_update()
}

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
        APP_NAME, TRAY_ICON_SIZE, TRAY_ID, check_enabled, check_label, clipboard_menu_visibility,
        pause_clipboard_title, update_enabled, update_label, update_visible,
    };
    use crate::app::{AppCommand, AppEvent};
    use crate::update::UpdateStatus;
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

    fn badged_icon() -> Vec<u8> {
        const DOT: [u8; 4] = [0xFF, 0xF5, 0xC5, 0x42];
        let mut data = TRAY_ICON_ARGB.to_vec();
        let size = TRAY_ICON_SIZE as i32;
        let center = size as f32 - 6.0;

        for y in (size - 13)..size {
            for x in (size - 13)..size {
                let dx = x as f32 + 0.5 - center;
                let dy = y as f32 + 0.5 - center;
                let distance = dx * dx + dy * dy;
                let offset = ((y * size + x) * 4) as usize;
                if distance <= 5.0 * 5.0 {
                    data[offset..offset + 4].copy_from_slice(&DOT);
                } else if distance <= 6.3 * 6.3 {
                    data[offset + 3] = 0;
                }
            }
        }

        data
    }

    struct DebloaterTray {
        command_tx: mpsc::Sender<AppCommand>,
        clipboard_debloater_enabled: bool,
        pause_expires_at: Option<Instant>,
        update_status: UpdateStatus,
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
            let data = if self.update_status.has_update() || true {
                badged_icon()
            } else {
                TRAY_ICON_ARGB.to_vec()
            };
            vec![Icon {
                width: TRAY_ICON_SIZE as i32,
                height: TRAY_ICON_SIZE as i32,
                data,
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
                    label: update_label(&self.update_status),
                    visible: update_visible(&self.update_status),
                    enabled: update_enabled(&self.update_status),
                    activate: Box::new(|tray: &mut Self| tray.send(AppCommand::InstallUpdate)),
                    ..Default::default()
                }
                .into(),
                StandardItem {
                    label: check_label(&self.update_status).into(),
                    enabled: check_enabled(&self.update_status),
                    activate: Box::new(|tray: &mut Self| tray.send(AppCommand::CheckForUpdates)),
                    ..Default::default()
                }
                .into(),
                #[cfg(feature = "config-gui-integration")]
                StandardItem {
                    label: "Settings".into(),
                    activate: Box::new(|tray: &mut Self| tray.send(AppCommand::OpenConfigGui)),
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
            update_status: UpdateStatus::Idle,
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
                AppEvent::UpdateStatusChanged(status) => {
                    handle
                        .update(|tray: &mut DebloaterTray| {
                            tray.update_status = status;
                        })
                        .await;
                }
                AppEvent::UpdateProgress { downloaded, total } => {
                    handle
                        .update(|tray: &mut DebloaterTray| {
                            if let UpdateStatus::Downloading {
                                downloaded: current,
                                total: current_total,
                                ..
                            } = &mut tray.update_status
                            {
                                *current = downloaded;
                                *current_total = total;
                            }
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
                update_status: UpdateStatus::Idle,
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
        fn update_item_appears_when_available() {
            let (command_tx, _command_rx) = mpsc::channel(1);
            let tray = DebloaterTray {
                command_tx,
                clipboard_debloater_enabled: true,
                pause_expires_at: None,
                update_status: UpdateStatus::Available {
                    version: "0.2.0".into(),
                },
            };
            let entries: Vec<(String, bool, bool)> = tray
                .menu()
                .into_iter()
                .filter_map(|item| match item {
                    MenuItem::Standard(item) => Some((item.label, item.visible, item.enabled)),
                    _ => None,
                })
                .collect();

            assert!(entries.iter().any(|(label, visible, enabled)| {
                *visible && *enabled && label == "Update available: v0.2.0"
            }));
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
        APP_NAME, TRAY_ICON_SIZE, TRAY_ID, check_enabled, check_label, clipboard_menu_visibility,
        pause_clipboard_title, update_enabled, update_label, update_visible,
    };
    use crate::app::{AppCommand, AppEvent};
    use crate::update::UpdateStatus;
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
            } else if menu_item_ids.update == id {
                try_send_command(&command_tx, AppCommand::InstallUpdate);
            } else if menu_item_ids.check_updates == id {
                try_send_command(&command_tx, AppCommand::CheckForUpdates);
            } else if menu_item_ids.exit == id {
                try_send_command(&command_tx, AppCommand::Exit);
            } else {
                #[cfg(feature = "config-gui-integration")]
                {
                    if menu_item_ids.settings == id {
                        try_send_command(&command_tx, AppCommand::OpenConfigGui);
                        return;
                    }
                }
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
            update_status: UpdateStatus::Idle,
            #[cfg(target_os = "macos")]
            update_badge: None,
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
        update_status: UpdateStatus,
        #[cfg(target_os = "macos")]
        update_badge: Option<TrayIcon>,
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

        fn apply_update_status(&mut self, status: UpdateStatus) {
            self.update_status = status;
            self.sync_update_items();
            self.update_icon();
        }

        fn apply_update_progress(&mut self, downloaded: u64, total: Option<u64>) {
            if let UpdateStatus::Downloading {
                downloaded: current,
                total: current_total,
                ..
            } = &mut self.update_status
            {
                *current = downloaded;
                *current_total = total;
                self.sync_update_items();
            }
        }

        fn sync_update_items(&mut self) {
            let Some(menu) = &self.menu else {
                return;
            };
            if let Err(err) = self.menu_items.sync_update_items(menu, &self.update_status) {
                error!("Failed to update tray update items: {err:#}");
            }
        }

        fn update_icon(&mut self) {
            #[cfg(target_os = "windows")]
            if let Some(tray_icon) = &self.tray_icon {
                match create_icon(self.update_status.has_update()) {
                    Ok(icon) => {
                        if let Err(err) = tray_icon.set_icon(Some(icon)) {
                            error!("Failed to update tray icon: {err:#}");
                        }
                    }
                    Err(err) => error!("Failed to create tray icon: {err:#}"),
                }
            }

            #[cfg(target_os = "macos")]
            if self.update_status.has_update() {
                if self.update_badge.is_none() {
                    match create_update_badge() {
                        Ok(badge) => self.update_badge = Some(badge),
                        Err(err) => error!("Failed to create update badge: {err:#}"),
                    }
                }
            } else {
                self.update_badge = None;
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
                UserEvent::App(AppEvent::UpdateStatusChanged(status)) => {
                    self.apply_update_status(status);
                }
                UserEvent::App(AppEvent::UpdateProgress { downloaded, total }) => {
                    self.apply_update_progress(downloaded, total);
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
        update: MenuItem,
        check_updates: MenuItem,
        #[cfg(feature = "config-gui-integration")]
        settings: MenuItem,
        exit: MenuItem,
    }

    impl MenuItems {
        fn new() -> Self {
            Self {
                debloat_clipboard: MenuItem::new("Debloat clipboard", true, None),
                pause_clipboard_debloat: MenuItem::new(&pause_clipboard_title(None), true, None),
                update: MenuItem::new("Update available", false, None),
                check_updates: MenuItem::new("Check for updates", true, None),
                #[cfg(feature = "config-gui-integration")]
                settings: MenuItem::new("Settings", true, None),
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

        fn sync_update_items(&self, menu: &Menu, status: &UpdateStatus) -> anyhow::Result<()> {
            self.update.set_text(update_label(status));
            self.update.set_enabled(update_enabled(status));
            self.check_updates.set_text(check_label(status));
            self.check_updates.set_enabled(check_enabled(status));
            sync_menu_item(menu, &self.update, update_visible(status), 2)
        }

        fn to_ids(&self) -> MenuItemIds {
            MenuItemIds {
                debloat_clipboard: self.debloat_clipboard.id().0.clone(),
                pause_clipboard_debloat: self.pause_clipboard_debloat.id().0.clone(),
                update: self.update.id().0.clone(),
                check_updates: self.check_updates.id().0.clone(),
                #[cfg(feature = "config-gui-integration")]
                settings: self.settings.id().0.clone(),
                exit: self.exit.id().0.clone(),
            }
        }
    }

    struct MenuItemIds {
        debloat_clipboard: String,
        pause_clipboard_debloat: String,
        update: String,
        check_updates: String,
        #[cfg(feature = "config-gui-integration")]
        settings: String,
        exit: String,
    }

    fn new_menu(menu_items: &MenuItems, clipboard_debloater_enabled: bool) -> anyhow::Result<Menu> {
        let menu = Menu::new();
        menu_items
            .sync_clipboard_items(&menu, clipboard_debloater_enabled)
            .context("build clipboard tray items")?;
        menu.append(&menu_items.check_updates)
            .context("build tray item 'Check for updates'")?;
        #[cfg(feature = "config-gui-integration")]
        menu.append(&menu_items.settings)
            .context("build tray item 'Settings'")?;
        menu.append(&menu_items.exit)
            .context("build tray item 'Exit'")?;
        menu_items
            .sync_update_items(&menu, &UpdateStatus::Idle)
            .context("build update tray items")?;
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
            .with_icon(create_icon(false)?)
            .with_icon_as_template(true)
            .build()
            .context("create tray icon")
    }

    fn create_icon(badged: bool) -> anyhow::Result<Icon> {
        let data = if badged {
            badged_icon()
        } else {
            TRAY_ICON_RGBA.to_vec()
        };
        Icon::from_rgba(data, TRAY_ICON_SIZE, TRAY_ICON_SIZE).context("create tray icon")
    }

    fn badged_icon() -> Vec<u8> {
        const DOT: [u8; 4] = [0xF5, 0xC5, 0x42, 0xFF];
        let mut data = TRAY_ICON_RGBA.to_vec();
        let size = TRAY_ICON_SIZE as i32;
        let center = size as f32 - 6.0;

        for y in (size - 13)..size {
            for x in (size - 13)..size {
                let dx = x as f32 + 0.5 - center;
                let dy = y as f32 + 0.5 - center;
                let distance = dx * dx + dy * dy;
                let offset = ((y * size + x) * 4) as usize;
                if distance <= 5.0 * 5.0 {
                    data[offset..offset + 4].copy_from_slice(&DOT);
                } else if distance <= 6.3 * 6.3 {
                    data[offset + 3] = 0;
                }
            }
        }

        data
    }

    #[cfg(target_os = "macos")]
    fn create_update_badge() -> anyhow::Result<TrayIcon> {
        const BADGE_SIZE: u32 = 18;
        let mut data = vec![0u8; (BADGE_SIZE * BADGE_SIZE * 4) as usize];
        let center = BADGE_SIZE as f32 / 2.0;

        for y in 0..BADGE_SIZE as i32 {
            for x in 0..BADGE_SIZE as i32 {
                let dx = x as f32 + 0.5 - center;
                let dy = y as f32 + 0.5 - center;
                if dx * dx + dy * dy <= 4.5 * 4.5 {
                    let offset = ((y * BADGE_SIZE as i32 + x) * 4) as usize;
                    data[offset..offset + 4].copy_from_slice(&[0xF5, 0xC5, 0x42, 0xFF]);
                }
            }
        }

        TrayIconBuilder::new()
            .with_id("urldebloater-update")
            .with_tooltip("Update available")
            .with_icon(Icon::from_rgba(data, BADGE_SIZE, BADGE_SIZE)?)
            .with_icon_as_template(false)
            .build()
            .context("create update badge")
    }
}
