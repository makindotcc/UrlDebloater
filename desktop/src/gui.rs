use std::sync::Arc;

use eframe::egui;
use tracing::debug;
use tray_icon::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};
use url::Url;
use urlwasher::{RedirectWashPolicy, UrlWasherConfig, PUBLIC_MIXER_INSTANCE};

use crate::AppState;

pub struct ConfigWindow {
    hide: bool,
    app_state: Arc<AppState>,
    config_state: UiConfigState,
}

#[derive(PartialEq, Eq, Clone)]
struct UiConfigState {
    mixer_instance: String,
    tiktok_policy: RedirectWashPolicy,
}

impl ConfigWindow {
    pub fn new(app_state: Arc<AppState>) -> Self {
        let washer_config = app_state.text_washer.url_washer.config();
        Self {
            hide: false,
            app_state,
            config_state: UiConfigState {
                mixer_instance: washer_config
                    .mixer_instance
                    .map(|url| url.to_string())
                    .unwrap_or_default(),
                tiktok_policy: washer_config.tiktok_policy,
            },
        }
    }
}

impl eframe::App for ConfigWindow {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.hide {
            self.hide = false;
            frame.set_visible(false);
        }

        let previous_config = self.config_state.clone();
        egui::CentralPanel::default().show(ctx, |ui| {
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
                    ui.text_edit_singleline(&mut self.config_state.mixer_instance)
                        .labelled_by(name_label.id);
                    if ui.button("use public instance").clicked() {
                        self.config_state.mixer_instance = PUBLIC_MIXER_INSTANCE.to_string();
                    }
                });
                if let Err(err) = Url::parse(&self.config_state.mixer_instance) {
                    ui.colored_label(ui.visuals().error_fg_color, format!("Invalid url: {err}"));
                }

                ui.separator();
                egui::ComboBox::from_label("TikTok")
                    .selected_text(format!("{}", self.config_state.tiktok_policy))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.config_state.tiktok_policy, RedirectWashPolicy::Ignore, "ignore");
                        ui.selectable_value(&mut self.config_state.tiktok_policy, RedirectWashPolicy::Locally, "locally");
                        ui.selectable_value(&mut self.config_state.tiktok_policy, RedirectWashPolicy::ViaMixer, "via mixer");
                    });
            }
        });

        if previous_config != self.config_state {
            debug!("Config changed.");
            self.app_state
                .text_washer
                .url_washer
                .set_config(UrlWasherConfig {
                    mixer_instance: Url::parse(&self.config_state.mixer_instance)
                        .map(Some)
                        .unwrap_or_default(),
                    tiktok_policy: self.config_state.tiktok_policy,
                });
        }
    }

    fn on_close_event(&mut self) -> bool {
        self.hide = true;
        false
    }
}

pub struct TrayMenu {
    _tray_icon: TrayIcon,
    pub open_config: MenuItem,
}

impl TrayMenu {
    pub fn new() -> Self {
        let tray_menu = Menu::new();
        let open_config = MenuItem::new("Open configuration", true, None);
        tray_menu
            .append_items(&[
                &PredefinedMenuItem::about(
                    None,
                    Some(AboutMetadata {
                        name: Some("UrlDebloater".to_string()),
                        comments: Some("Remove tracking parameters from URLs...".to_string()),
                        ..Default::default()
                    }),
                ),
                &open_config,
                &PredefinedMenuItem::separator(),
                &PredefinedMenuItem::quit(None),
            ])
            .unwrap();
        let icon = load_tray_icon();
        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("UrlDebloater")
            .with_icon(icon)
            .with_menu(Box::new(tray_menu))
            .build()
            .expect("Could not create tray icon");
        Self {
            _tray_icon: tray_icon,
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
