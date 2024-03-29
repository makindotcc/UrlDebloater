use std::collections::HashMap;

use eframe::egui;
use notify_rust::Notification;
use tracing::{debug, error};
use tray_icon::{
    menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};
use url::Url;
use urlwasher::{
    rule_set, RedirectWashPolicy, RuleName, UrlWasherConfig, WashingProgram, PUBLIC_MIXER_INSTANCE,
};

use crate::{AppConfig, AppStateFlow, APP_NAME};

pub struct ConfigWindow {
    hide: bool,
    ui_config_state: UiConfigState,
    app_state_flow: AppStateFlow,
}

#[derive(PartialEq, Eq, Clone)]
struct UiConfigState {
    mixer_instance: String,
    redirect_policy: HashMap<RuleName, RedirectWashPolicy>,
    enable_clipboard_patcher: bool,
    auto_start: bool,
}

fn apply_ui_config(app_config: &mut AppConfig, ui_config: &UiConfigState) {
    app_config.url_washer = UrlWasherConfig {
        mixer_instance: Url::parse(&ui_config.mixer_instance)
            .map(Some)
            .unwrap_or(None),
        redirect_policy: ui_config.redirect_policy.clone(),
    };
    app_config.enable_clipboard_patcher = ui_config.enable_clipboard_patcher;
}

impl ConfigWindow {
    pub fn new(app_state_flow: AppStateFlow, open_config_window: bool) -> Self {
        let app_state = app_state_flow.current();
        let config = &app_state.config;
        let mixer_instance = config
            .url_washer
            .mixer_instance
            .as_ref()
            .map(|url| url.to_string())
            .unwrap_or_default();
        let auto_start = app_state
            .auto_launch
            .is_enabled()
            .expect("Could not check if autostart is enabled");
        let ui_config_state = UiConfigState {
            mixer_instance,
            redirect_policy: config.url_washer.redirect_policy.clone(),
            enable_clipboard_patcher: config.enable_clipboard_patcher,
            auto_start,
        };
        drop(app_state);
        Self {
            hide: !open_config_window,
            ui_config_state,
            app_state_flow,
        }
    }
}

impl eframe::App for ConfigWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.hide {
            self.hide = false;
            frame.set_visible(false);
        }

        let previous_config = self.ui_config_state.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Desktop settings");
            ui.checkbox(&mut self.ui_config_state.enable_clipboard_patcher, "Automatically debloat URLs in your clipboard");
            if ui.checkbox(&mut self.ui_config_state.auto_start, "Start debloater with system startup").clicked() {
                let auto_launch = &self.app_state_flow.current().auto_launch;
                if self.ui_config_state.auto_start {
                    auto_launch.enable().expect("Could not enable auto start");
                } else {
                    auto_launch.disable().expect("Could not disable auto start");
                }
            }

            ui.separator();
            {
                ui.heading("Per user generated links")
                    .on_hover_text("Section for links that cannot be anonymised without requesting service server.");

                ui.horizontal(|ui| {
                    let name_label = ui
                        .label("Mixer instance url: ")
                        .on_hover_text("To remove tracking capabilities of short links like https://vm.tiktok.com/PerUserId \
                        we need request target server (in this case - tiktok) to unroll it.\n\
                        \
                        You can do this from your local network, but there is a risk that they will catch you by correlating your IP address.\n\
                        \n\
                        This option allows you to resolve these links via service hosted on other network.\n\
                        ⚠ It sends url to third party person if you don't host mixer yourself ⚠ (Not so scary for TikTok videos tho) \
                        ");
                    ui.text_edit_singleline(&mut self.ui_config_state.mixer_instance)
                        .labelled_by(name_label.id);
                    if ui.button("use public instance").clicked() {
                        self.ui_config_state.mixer_instance = PUBLIC_MIXER_INSTANCE.to_string();
                    }
                });
                if !self.ui_config_state.mixer_instance.is_empty() {
                    if let Err(err) = Url::parse(&self.ui_config_state.mixer_instance) {
                        ui.colored_label(ui.visuals().error_fg_color, format!("Invalid url: {err}"));
                    }
                }

                for rule in rule_set().iter().filter(|rule| rule.washing_programs.contains(&WashingProgram::ResolveRedirection)) {
                    let policy = match self.ui_config_state.redirect_policy.get_mut(&rule.name) {
                        Some(policy) => policy,
                        None => {
                            self.ui_config_state.redirect_policy.entry(rule.name.clone()).or_insert(RedirectWashPolicy::Ignore)
                        },
                    };

                    egui::ComboBox::from_label(rule.domains.join(", "))
                        .selected_text(policy.to_string())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(policy, RedirectWashPolicy::Ignore, "ignore");
                            ui.selectable_value(policy, RedirectWashPolicy::Locally, "locally");
                            ui.selectable_value(policy, RedirectWashPolicy::ViaMixer, "via mixer");
                        });
                }
            }
        });

        if previous_config != self.ui_config_state {
            debug!("Config changed.");
            self.app_state_flow.modify_config(|config| {
                apply_ui_config(config, &self.ui_config_state);
            });
        }
    }

    fn on_close_event(&mut self) -> bool {
        self.hide = true;
        if let Err(err) = Notification::new()
            .appname(APP_NAME)
            .summary(APP_NAME)
            .body("Minimized to tray icon :)")
            .show()
        {
            error!("Could not show error notification: {err}");
        }
        false
    }
}

pub struct TrayMenu {
    _tray_icon: TrayIcon,
    pub wash_clipboard: MenuItem,
    pub pause_clipboard_washer: CheckMenuItem,
    pub open_config: MenuItem,
}

impl TrayMenu {
    pub fn new() -> Self {
        let tray_menu = Menu::new();
        let wash_clipboard = MenuItem::new("Debloat current clipboard", true, None);
        let pause_clipboard_washer =
            CheckMenuItem::new("Pause clipboard debloater temporary", true, false, None);
        let open_config = MenuItem::new("Open configuration", true, None);
        tray_menu
            .append_items(&[
                &wash_clipboard,
                &pause_clipboard_washer,
                &PredefinedMenuItem::separator(),
                &open_config,
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::about(
                    None,
                    Some(AboutMetadata {
                        name: Some(APP_NAME.to_string()),
                        comments: Some("Remove tracking parameters from URLs...".to_string()),
                        ..Default::default()
                    }),
                ),
                &PredefinedMenuItem::quit(None),
            ])
            .unwrap();
        let icon = load_tray_icon();
        let tray_icon = TrayIconBuilder::new()
            .with_tooltip(APP_NAME)
            .with_icon(icon)
            .with_menu(Box::new(tray_menu))
            .build()
            .expect("Could not create tray icon");
        Self {
            _tray_icon: tray_icon,
            wash_clipboard,
            pause_clipboard_washer,
            open_config,
        }
    }
}

fn load_tray_icon() -> tray_icon::Icon {
    let (icon_rgba, icon_width, icon_height) = {
        let image = image::load_from_memory(include_bytes!("../tray_icon.png"))
            .expect("Failed to open icon path")
            .into_rgba8();
        let (width, height) = image.dimensions();
        let rgba = image.into_raw();
        (rgba, width, height)
    };
    tray_icon::Icon::from_rgba(icon_rgba, icon_width, icon_height).expect("Failed to open icon")
}
