#[cfg(all(target_os = "linux", feature = "config-gui"))]
use fltk::{
    button::{Button, CheckButton},
    enums::{CallbackTrigger, Color, Font},
    frame::Frame,
    group::Scroll,
    input::Input,
    prelude::*,
    window::Window,
};
#[cfg(all(target_os = "linux", feature = "config-gui"))]
use log::{error, warn};
#[cfg(all(target_os = "linux", feature = "config-gui"))]
use url::Url;
#[cfg(all(target_os = "linux", feature = "config-gui"))]
use urlwasher::PUBLIC_MIXER_INSTANCE;

#[cfg(all(target_os = "linux", feature = "config-gui"))]
use crate::AppStateFlow;

#[cfg(all(target_os = "linux", feature = "config-gui"))]
pub struct ConfigWindow {
    window: Window,
}

#[cfg(all(target_os = "linux", feature = "config-gui"))]
#[derive(PartialEq, Eq, Clone)]
struct UiConfigState {
    mixer_instance: String,
    resolve_redirects: bool,
    enable_clipboard_patcher: bool,
    auto_start: bool,
}

#[cfg(all(target_os = "linux", feature = "config-gui"))]
impl ConfigWindow {
    pub fn new(app_state_flow: AppStateFlow, on_close: impl Fn() + 'static) -> Self {
        let ui_config_state = current_ui_config(&app_state_flow);
        let mut window = Window::new(100, 100, 720, 560, "UrlDebloater settings");
        window.make_resizable(true);

        let mut scroll = Scroll::new(0, 0, 720, 560, "");
        scroll.set_scrollbar_size(14);

        let mut y = 16;
        heading(24, y, 660, "Desktop settings");
        y += 34;

        let mut enable_clipboard = CheckButton::new(
            24,
            y,
            650,
            28,
            "Automatically debloat URLs in your clipboard",
        );
        enable_clipboard.set_value(ui_config_state.enable_clipboard_patcher);
        {
            let app_state_flow = app_state_flow.clone();
            enable_clipboard.set_callback(move |checkbox| {
                app_state_flow.modify_config(|config| {
                    config.enable_clipboard_patcher = checkbox.value();
                });
            });
        }
        y += 34;

        let mut auto_start =
            CheckButton::new(24, y, 650, 28, "Start debloater with system startup");
        auto_start.set_value(ui_config_state.auto_start);
        {
            let app_state_flow = app_state_flow.clone();
            auto_start.set_callback(move |checkbox| {
                let auto_launch = &app_state_flow.current().auto_launch;
                let result = if checkbox.value() {
                    auto_launch.enable()
                } else {
                    auto_launch.disable()
                };
                if let Err(err) = result {
                    error!("Could not update auto start: {err}");
                }
            });
        }
        y += 46;

        heading(24, y, 660, "Per user generated links");
        y += 34;

        Frame::new(24, y, 150, 28, "Mixer instance url:");
        let mut mixer_entry = Input::new(176, y, 360, 28, "");
        mixer_entry.set_value(&ui_config_state.mixer_instance);
        mixer_entry.set_trigger(CallbackTrigger::Changed);
        let mut public_instance_button = Button::new(548, y, 148, 28, "use public instance");
        y += 34;

        let mut mixer_error = Frame::new(176, y, 520, 22, "");
        mixer_error.set_label_color(Color::Red);
        update_mixer_config(&app_state_flow, &mixer_entry, &mut mixer_error);
        {
            let app_state_flow = app_state_flow.clone();
            let mut mixer_error = mixer_error.clone();
            mixer_entry.set_callback(move |entry| {
                update_mixer_config(&app_state_flow, entry, &mut mixer_error);
            });
        }
        {
            let app_state_flow = app_state_flow.clone();
            let mut mixer_entry = mixer_entry.clone();
            let mut mixer_error = mixer_error.clone();
            public_instance_button.set_callback(move |_| {
                mixer_entry.set_value(PUBLIC_MIXER_INSTANCE);
                update_mixer_config(&app_state_flow, &mixer_entry, &mut mixer_error);
            });
        }
        y += 38;

        let mut resolve_redirects =
            CheckButton::new(24, y, 650, 28, "Resolve short-link redirects");
        resolve_redirects.set_value(ui_config_state.resolve_redirects);
        {
            let app_state_flow = app_state_flow.clone();
            resolve_redirects.set_callback(move |checkbox| {
                app_state_flow.modify_config(|config| {
                    config.url_washer.resolve_redirects = checkbox.value();
                });
            });
        }
        y += 34;

        scroll.end();
        window.end();

        window.set_callback(move |window| {
            let mut window_to_delete = Some(window.clone());
            window.hide();
            on_close();
            fltk::app::add_timeout3(0.0, move |_| {
                if let Some(window) = window_to_delete.take() {
                    fltk::app::delete_widget(window);
                    trim_process_heap();
                }
            });
        });

        Self { window }
    }

    pub fn show(&self) {
        let mut window = self.window.clone();
        window.show();
    }
}

#[cfg(all(target_os = "linux", feature = "config-gui"))]
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
        resolve_redirects: config.url_washer.resolve_redirects,
        enable_clipboard_patcher: config.enable_clipboard_patcher,
        auto_start,
    }
}

#[cfg(all(target_os = "linux", feature = "config-gui"))]
fn heading(x: i32, y: i32, width: i32, text: &str) {
    let mut frame = Frame::new(x, y, width, 24, text);
    frame.set_align(fltk::enums::Align::Left | fltk::enums::Align::Inside);
    frame.set_label_font(Font::HelveticaBold);
    frame.set_label_size(14);
}

#[cfg(all(target_os = "linux", feature = "config-gui"))]
fn update_mixer_config(app_state_flow: &AppStateFlow, entry: &Input, error: &mut Frame) {
    let mixer_instance = entry.value();
    let parsed_mixer_instance = if mixer_instance.is_empty() {
        Ok(None)
    } else {
        Url::parse(&mixer_instance).map(Some)
    };

    match parsed_mixer_instance {
        Ok(mixer_instance) => {
            error.set_label("");
            app_state_flow.modify_config(|config| {
                config.url_washer.mixer_instance = mixer_instance;
            });
        }
        Err(err) => {
            error.set_label(&format!("Invalid url: {err}"));
        }
    }
}

#[cfg(all(target_os = "linux", feature = "config-gui"))]
fn trim_process_heap() {
    unsafe extern "C" {
        fn malloc_trim(pad: usize) -> i32;
    }
    unsafe {
        malloc_trim(0);
    }
}
