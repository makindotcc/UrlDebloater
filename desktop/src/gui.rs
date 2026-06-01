#[cfg(target_os = "linux")]
use std::collections::HashMap;

#[cfg(target_os = "linux")]
use glib::Propagation;
#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use tracing::{error, warn};
use tray_icon::{
    menu::{AboutMetadata, CheckMenuItem, Menu, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};
#[cfg(target_os = "linux")]
use url::Url;
#[cfg(target_os = "linux")]
use urlwasher::{rule_set, RedirectWashPolicy, RuleName, WashingProgram, PUBLIC_MIXER_INSTANCE};

#[cfg(target_os = "linux")]
use crate::AppStateFlow;
use crate::APP_NAME;

#[cfg(target_os = "linux")]
pub struct ConfigWindow {
    window: gtk::Window,
}

#[cfg(target_os = "linux")]
#[derive(PartialEq, Eq, Clone)]
struct UiConfigState {
    mixer_instance: String,
    redirect_policy: HashMap<RuleName, RedirectWashPolicy>,
    enable_clipboard_patcher: bool,
    auto_start: bool,
}

#[cfg(target_os = "linux")]
impl ConfigWindow {
    pub fn new(app_state_flow: AppStateFlow, on_close: impl Fn() + 'static) -> Self {
        let ui_config_state = current_ui_config(&app_state_flow);
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title("UrlDebloater settings");
        window.set_default_size(620, 340);
        window.set_border_width(16);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
        window.add(&content);

        let desktop_heading = heading("Desktop settings");
        content.pack_start(&desktop_heading, false, false, 0);

        let enable_clipboard =
            gtk::CheckButton::with_label("Automatically debloat URLs in your clipboard");
        enable_clipboard.set_active(ui_config_state.enable_clipboard_patcher);
        content.pack_start(&enable_clipboard, false, false, 0);
        {
            let app_state_flow = app_state_flow.clone();
            enable_clipboard.connect_toggled(move |checkbox| {
                app_state_flow.modify_config(|config| {
                    config.enable_clipboard_patcher = checkbox.is_active();
                });
            });
        }

        let auto_start = gtk::CheckButton::with_label("Start debloater with system startup");
        auto_start.set_active(ui_config_state.auto_start);
        content.pack_start(&auto_start, false, false, 0);
        {
            let app_state_flow = app_state_flow.clone();
            auto_start.connect_toggled(move |checkbox| {
                let auto_launch = &app_state_flow.current().auto_launch;
                let result = if checkbox.is_active() {
                    auto_launch.enable()
                } else {
                    auto_launch.disable()
                };
                if let Err(err) = result {
                    error!("Could not update auto start: {err}");
                }
            });
        }

        let separator = gtk::Separator::new(gtk::Orientation::Horizontal);
        content.pack_start(&separator, false, false, 4);

        let links_heading = heading("Per user generated links");
        links_heading.set_tooltip_text(Some(
            "Section for links that cannot be anonymised without requesting service server.",
        ));
        content.pack_start(&links_heading, false, false, 0);

        let mixer_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let mixer_label = gtk::Label::new(Some("Mixer instance url:"));
        mixer_label.set_halign(gtk::Align::Start);
        mixer_label.set_tooltip_text(Some(
            "To remove tracking capabilities of short links like https://vm.tiktok.com/PerUserId \
we need request target server to unroll it. You can do this from your local network, but there is \
a risk that they will catch you by correlating your IP address. This option allows you to resolve \
these links via service hosted on other network. It sends url to third party person if you don't \
host mixer yourself.",
        ));
        let mixer_entry = gtk::Entry::new();
        mixer_entry.set_hexpand(true);
        mixer_entry.set_text(&ui_config_state.mixer_instance);
        let public_instance_button = gtk::Button::with_label("use public instance");
        mixer_row.pack_start(&mixer_label, false, false, 0);
        mixer_row.pack_start(&mixer_entry, true, true, 0);
        mixer_row.pack_start(&public_instance_button, false, false, 0);
        content.pack_start(&mixer_row, false, false, 0);

        let mixer_error = gtk::Label::new(None);
        mixer_error.set_halign(gtk::Align::Start);
        content.pack_start(&mixer_error, false, false, 0);
        update_mixer_config(&app_state_flow, &mixer_entry, &mixer_error);
        {
            let app_state_flow = app_state_flow.clone();
            let mixer_error = mixer_error.clone();
            mixer_entry.connect_changed(move |entry| {
                update_mixer_config(&app_state_flow, entry, &mixer_error);
            });
        }
        {
            let mixer_entry = mixer_entry.clone();
            public_instance_button.connect_clicked(move |_| {
                mixer_entry.set_text(PUBLIC_MIXER_INSTANCE);
            });
        }

        for rule in rule_set().iter().filter(|rule| {
            rule.washing_programs
                .contains(&WashingProgram::ResolveRedirection)
        }) {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            let label = gtk::Label::new(Some(&rule.domains.join(", ")));
            label.set_halign(gtk::Align::Start);
            label.set_hexpand(true);
            let combo = gtk::ComboBoxText::new();
            combo.append(Some("ignore"), "ignore");
            combo.append(Some("locally"), "locally");
            combo.append(Some("via-mixer"), "via mixer");

            let policy = ui_config_state
                .redirect_policy
                .get(&rule.name)
                .copied()
                .unwrap_or(RedirectWashPolicy::Ignore);
            combo.set_active_id(Some(policy_id(policy)));

            row.pack_start(&label, true, true, 0);
            row.pack_start(&combo, false, false, 0);
            content.pack_start(&row, false, false, 0);

            let app_state_flow = app_state_flow.clone();
            let rule_name = rule.name.clone();
            combo.connect_changed(move |combo| {
                if let Some(policy) = combo.active_id().as_deref().and_then(policy_from_id) {
                    app_state_flow.modify_config(|config| {
                        config
                            .url_washer
                            .redirect_policy
                            .insert(rule_name.clone(), policy);
                    });
                }
            });
        }

        window.connect_delete_event(move |_, _| {
            on_close();
            Propagation::Proceed
        });

        content.show_all();
        Self { window }
    }

    pub fn show(&self) {
        self.window.show();
        self.window.present();
    }
}

#[cfg(target_os = "linux")]
fn current_ui_config(app_state_flow: &AppStateFlow) -> UiConfigState {
    let app_state = app_state_flow.current();
    let config = &app_state.config;
    let mixer_instance = config
        .url_washer
        .mixer_instance
        .as_ref()
        .map(|url| url.to_string())
        .unwrap_or_default();
    let auto_start = app_state.auto_launch.is_enabled().unwrap_or_else(|err| {
        warn!("Could not check if autostart is enabled: {err}");
        false
    });
    UiConfigState {
        mixer_instance,
        redirect_policy: config.url_washer.redirect_policy.clone(),
        enable_clipboard_patcher: config.enable_clipboard_patcher,
        auto_start,
    }
}

#[cfg(target_os = "linux")]
fn heading(text: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_markup(&format!("<b>{text}</b>"));
    label.set_halign(gtk::Align::Start);
    label
}

#[cfg(target_os = "linux")]
fn update_mixer_config(app_state_flow: &AppStateFlow, entry: &gtk::Entry, error: &gtk::Label) {
    let mixer_instance = entry.text().to_string();
    let parsed_mixer_instance = if mixer_instance.is_empty() {
        Ok(None)
    } else {
        Url::parse(&mixer_instance).map(Some)
    };

    match parsed_mixer_instance {
        Ok(mixer_instance) => {
            error.set_text("");
            app_state_flow.modify_config(|config| {
                config.url_washer.mixer_instance = mixer_instance;
            });
        }
        Err(err) => {
            error.set_markup(&format!(
                "<span foreground=\"red\">Invalid url: {err}</span>"
            ));
        }
    }
}

#[cfg(target_os = "linux")]
fn policy_id(policy: RedirectWashPolicy) -> &'static str {
    match policy {
        RedirectWashPolicy::Ignore => "ignore",
        RedirectWashPolicy::Locally => "locally",
        RedirectWashPolicy::ViaMixer => "via-mixer",
    }
}

#[cfg(target_os = "linux")]
fn policy_from_id(id: &str) -> Option<RedirectWashPolicy> {
    match id {
        "ignore" => Some(RedirectWashPolicy::Ignore),
        "locally" => Some(RedirectWashPolicy::Locally),
        "via-mixer" => Some(RedirectWashPolicy::ViaMixer),
        _ => None,
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
