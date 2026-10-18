use crate::app::{AppCommand, AppEvent, CLIPBOARD_PAUSE_DURATION};
use crate::update::UpdateStatus;
use log::error;
use std::{borrow::Cow, ops::Sub, time::Instant};
use tokio::sync::mpsc;

const APP_NAME: &str = "UrlDebloater";
const TRAY_ID: &str = "urldebloater";
const TRAY_ICON_SIZE: u32 = 32;

fn update_label(status: &UpdateStatus) -> Cow<'static, str> {
    match status {
        UpdateStatus::Available { version } => Cow::Owned(format!("Update available: v{version}")),
        UpdateStatus::Downloading {
            downloaded, total, ..
        } => match total.filter(|total| *total > 0) {
            Some(total) => Cow::Owned(format!(
                "Updating… {}%",
                (downloaded.saturating_mul(100) / total).min(100)
            )),
            None => Cow::Borrowed("Updating…"),
        },
        UpdateStatus::Installing { .. } => Cow::Borrowed("Updating…"),
        UpdateStatus::Failed {
            version: Some(version),
            ..
        } => Cow::Owned(format!("Update available: v{version} (retry)")),
        UpdateStatus::Failed { version: None, .. } => Cow::Borrowed("Update failed - retry"),
        UpdateStatus::Idle | UpdateStatus::Checking => Cow::Borrowed("Update available"),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrayAction {
    DebloatClipboard,
    PauseClipboardDebloater,
    InstallUpdate,
    CheckForUpdates,
    #[cfg(feature = "config-gui-integration")]
    OpenConfigGui,
    Exit,
}

const MENU_ACTIONS: &[TrayAction] = &[
    TrayAction::DebloatClipboard,
    TrayAction::PauseClipboardDebloater,
    TrayAction::InstallUpdate,
    TrayAction::CheckForUpdates,
    #[cfg(feature = "config-gui-integration")]
    TrayAction::OpenConfigGui,
    TrayAction::Exit,
];

impl TrayAction {
    fn send(self, command_tx: &mpsc::Sender<AppCommand>) {
        let command = match self {
            Self::DebloatClipboard => AppCommand::ClearClipboard,
            Self::PauseClipboardDebloater => AppCommand::ToggleClipboardDebloater,
            Self::InstallUpdate => AppCommand::InstallUpdate,
            Self::CheckForUpdates => AppCommand::CheckForUpdates,
            #[cfg(feature = "config-gui-integration")]
            Self::OpenConfigGui => AppCommand::OpenConfigGui,
            Self::Exit => AppCommand::Exit,
        };
        if let Err(err) = command_tx.try_send(command) {
            error!("Failed to send tray command: {err}");
        }
    }
}

struct MenuEntry {
    action: TrayAction,
    label: Cow<'static, str>,
    visible: bool,
    enabled: bool,
}

struct TrayState {
    clipboard_debloater_enabled: bool,
    pause_expires_at: Option<Instant>,
    update_status: UpdateStatus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TrayChange {
    Unchanged,
    MenuChanged,
    MenuAndIconChanged,
    Exit,
}

impl TrayState {
    fn new(clipboard_debloater_enabled: bool) -> Self {
        Self {
            clipboard_debloater_enabled,
            pause_expires_at: None,
            update_status: UpdateStatus::Idle,
        }
    }

    fn apply_event(&mut self, event: AppEvent) -> TrayChange {
        match event {
            AppEvent::ClipboardStatusChanged {
                clipboard_debloater_enabled,
                pause_expires_at,
            } => {
                self.clipboard_debloater_enabled = clipboard_debloater_enabled;
                self.pause_expires_at = pause_expires_at;
                TrayChange::MenuChanged
            }
            AppEvent::UpdateStatusChanged(status) => {
                self.update_status = status;
                TrayChange::MenuAndIconChanged
            }
            AppEvent::UpdateProgress { downloaded, total } => {
                if let UpdateStatus::Downloading {
                    downloaded: current,
                    total: current_total,
                    ..
                } = &mut self.update_status
                {
                    *current = downloaded;
                    *current_total = total;
                    TrayChange::MenuChanged
                } else {
                    TrayChange::Unchanged
                }
            }
            AppEvent::Exit => TrayChange::Exit,
        }
    }

    fn has_update(&self) -> bool {
        self.update_status.has_update()
    }

    fn menu_entries(&self) -> impl Iterator<Item = MenuEntry> + '_ {
        MENU_ACTIONS
            .iter()
            .copied()
            .map(|action| self.menu_entry(action))
    }

    fn menu_entry(&self, action: TrayAction) -> MenuEntry {
        let (label, visible, enabled) = match action {
            TrayAction::DebloatClipboard => (
                Cow::Borrowed("Debloat clipboard"),
                !self.clipboard_debloater_enabled,
                true,
            ),
            TrayAction::PauseClipboardDebloater => (
                Cow::Owned(pause_clipboard_title(self.pause_expires_at)),
                self.clipboard_debloater_enabled,
                true,
            ),
            TrayAction::InstallUpdate => (
                update_label(&self.update_status),
                update_visible(&self.update_status),
                update_enabled(&self.update_status),
            ),
            TrayAction::CheckForUpdates => (
                Cow::Borrowed(check_label(&self.update_status)),
                true,
                check_enabled(&self.update_status),
            ),
            #[cfg(feature = "config-gui-integration")]
            TrayAction::OpenConfigGui => (Cow::Borrowed("Settings"), true, true),
            TrayAction::Exit => (Cow::Borrowed("Exit"), true, true),
        };
        MenuEntry {
            action,
            label,
            visible,
            enabled,
        }
    }
}

fn badged_icon(source: &[u8], dot: [u8; 4]) -> Vec<u8> {
    let mut data = source.to_vec();
    let size = TRAY_ICON_SIZE as i32;
    let center = size as f32 - 6.0;

    for y in (size - 13)..size {
        for x in (size - 13)..size {
            let dx = x as f32 + 0.5 - center;
            let dy = y as f32 + 0.5 - center;
            let distance = dx * dx + dy * dy;
            let offset = ((y * size + x) * 4) as usize;
            if distance <= 5.0 * 5.0 {
                data[offset..offset + 4].copy_from_slice(&dot);
            } else if distance <= 6.3 * 6.3 {
                data[offset + 3] = 0;
            }
        }
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu_entry(state: &TrayState, action: TrayAction) -> MenuEntry {
        state
            .menu_entries()
            .find(|entry| entry.action == action)
            .expect("menu entry")
    }

    #[test]
    fn update_item_appears_when_available() {
        let mut state = TrayState::new(true);
        state.apply_event(AppEvent::UpdateStatusChanged(UpdateStatus::Available {
            version: "0.2.0".into(),
        }));

        let entry = menu_entry(&state, TrayAction::InstallUpdate);
        assert!(entry.visible && entry.enabled);
    }

    #[test]
    fn clipboard_entries_follow_auto_debloater_state() {
        let mut state = TrayState::new(true);
        assert!(!menu_entry(&state, TrayAction::DebloatClipboard).visible);
        assert!(menu_entry(&state, TrayAction::PauseClipboardDebloater).visible);

        state.apply_event(AppEvent::ClipboardStatusChanged {
            clipboard_debloater_enabled: false,
            pause_expires_at: None,
        });

        assert!(menu_entry(&state, TrayAction::DebloatClipboard).visible);
        assert!(!menu_entry(&state, TrayAction::PauseClipboardDebloater).visible);
    }
}

#[cfg(target_os = "linux")]
pub(crate) mod platform {
    use super::{APP_NAME, TRAY_ICON_SIZE, TRAY_ID, TrayChange, TrayState, badged_icon};
    use crate::app::{AppCommand, AppEvent};
    use anyhow::Context;
    use ksni::{
        Icon, OfflineReason, ToolTip, Tray, TrayMethods,
        menu::{MenuItem, StandardItem},
    };
    use log::{info, warn};
    use tokio::sync::mpsc;

    const UPDATE_DOT_ARGB: [u8; 4] = [0xFF, 0xF5, 0xC5, 0x42];
    const TRAY_ICON_ARGB: &[u8] = include_bytes!(concat!(
        env!("CARGO_WORKSPACE_DIR"),
        "assets/icon/tray_32.argb"
    ));

    struct DebloaterTray {
        command_tx: mpsc::Sender<AppCommand>,
        state: TrayState,
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
            let data = if self.state.has_update() {
                badged_icon(TRAY_ICON_ARGB, UPDATE_DOT_ARGB)
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
            self.state
                .menu_entries()
                .map(|entry| {
                    let action = entry.action;
                    StandardItem {
                        label: entry.label.into_owned(),
                        visible: entry.visible,
                        enabled: entry.enabled,
                        activate: Box::new(move |tray: &mut Self| action.send(&tray.command_tx)),
                        ..Default::default()
                    }
                    .into()
                })
                .collect()
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
            state: TrayState::new(clipboard_debloater_enabled),
        };
        let handle = tray
            .assume_sni_available(true)
            .spawn()
            .await
            .context("spawn tray icon")?;
        info!("Tray icon service started");

        while let Some(event) = event_rx.recv().await {
            let change = handle
                .update(move |tray: &mut DebloaterTray| tray.state.apply_event(event))
                .await;
            if matches!(change, Some(TrayChange::Exit) | None) {
                break;
            }
        }

        handle.shutdown().await;
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
pub(crate) mod platform {
    use super::{
        APP_NAME, TRAY_ICON_SIZE, TRAY_ID, TrayAction, TrayChange, TrayState, badged_icon,
    };
    use crate::app::{AppCommand, AppEvent};
    use anyhow::Context;
    use log::{debug, error};
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

    const UPDATE_DOT_RGBA: [u8; 4] = [0xF5, 0xC5, 0x42, 0xFF];
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

        let state = TrayState::new(clipboard_debloater_enabled);
        let menu_items = MenuItems::new(&state);
        let menu_item_actions = menu_items.action_ids();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let id: &str = event.id().as_ref();
            if let Some((_, action)) = menu_item_actions
                .iter()
                .find(|(item_id, _)| item_id.as_str() == id)
            {
                action.send(&command_tx);
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

        let menu = new_menu(&menu_items, &state)?;
        let mut tray = TrayHandler {
            menu_items,
            menu: Some(menu),
            tray_icon: None,
            state,
            #[cfg(target_os = "macos")]
            update_badge: None,
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
        menu_items: MenuItems,
        menu: Option<Menu>,
        tray_icon: Option<TrayIcon>,
        state: TrayState,
        #[cfg(target_os = "macos")]
        update_badge: Option<TrayIcon>,
    }

    impl TrayHandler {
        fn sync_menu(&self) {
            let Some(menu) = &self.menu else {
                return;
            };
            if let Err(err) = self.menu_items.sync(menu, &self.state) {
                error!("Failed to update tray menu: {err:#}");
            }
        }

        fn update_icon(&mut self) {
            #[cfg(target_os = "windows")]
            if let Some(tray_icon) = &self.tray_icon {
                match create_icon(self.state.has_update()) {
                    Ok(icon) => {
                        if let Err(err) = tray_icon.set_icon(Some(icon)) {
                            error!("Failed to update tray icon: {err:#}");
                        }
                    }
                    Err(err) => error!("Failed to create tray icon: {err:#}"),
                }
            }

            #[cfg(target_os = "macos")]
            if self.state.has_update() {
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
                UserEvent::App(event) => match self.state.apply_event(event) {
                    TrayChange::Unchanged => {}
                    TrayChange::MenuChanged => self.sync_menu(),
                    TrayChange::MenuAndIconChanged => {
                        self.sync_menu();
                        self.update_icon();
                    }
                    TrayChange::Exit => event_loop.exit(),
                },
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

    struct NativeMenuItem {
        action: TrayAction,
        item: MenuItem,
    }

    struct MenuItems {
        items: Vec<NativeMenuItem>,
    }

    impl MenuItems {
        fn new(state: &TrayState) -> Self {
            Self {
                items: state
                    .menu_entries()
                    .map(|entry| NativeMenuItem {
                        action: entry.action,
                        item: MenuItem::new(entry.label.as_ref(), entry.enabled, None),
                    })
                    .collect(),
            }
        }

        fn action_ids(&self) -> Vec<(String, TrayAction)> {
            self.items
                .iter()
                .map(|entry| (entry.item.id().0.clone(), entry.action))
                .collect()
        }

        fn sync(&self, menu: &Menu, state: &TrayState) -> anyhow::Result<()> {
            let mut position = 0;
            for (native, entry) in self.items.iter().zip(state.menu_entries()) {
                debug_assert_eq!(native.action, entry.action);
                native.item.set_text(entry.label.as_ref());
                native.item.set_enabled(entry.enabled);
                sync_menu_item(menu, &native.item, entry.visible, position)?;
                if entry.visible {
                    position += 1;
                }
            }
            Ok(())
        }
    }

    fn new_menu(menu_items: &MenuItems, state: &TrayState) -> anyhow::Result<Menu> {
        let menu = Menu::new();
        menu_items.sync(&menu, state).context("build tray menu")?;
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
            badged_icon(TRAY_ICON_RGBA, UPDATE_DOT_RGBA)
        } else {
            TRAY_ICON_RGBA.to_vec()
        };
        Icon::from_rgba(data, TRAY_ICON_SIZE, TRAY_ICON_SIZE).context("create tray icon")
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
