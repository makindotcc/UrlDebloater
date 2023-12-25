use eframe::egui;
use tray_icon::{
    menu::{AboutMetadata, Menu, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

pub struct MyApp {
    hide: bool,
    name: String,
    age: u32,
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            name: "Arthur".to_owned(),
            age: 42,
            hide: true,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.hide {
            self.hide = false;
            frame.set_visible(false);
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("UrlDebloater");
            ui.horizontal(|ui| {
                let name_label = ui.label("Your name: ");
                ui.text_edit_singleline(&mut self.name)
                    .labelled_by(name_label.id);
            });
            ui.add(egui::Slider::new(&mut self.age, 0..=120).text("age"));
            if ui.button("Click each year").clicked() {
                self.age += 1;
            }
            ui.label(format!("Hello '{}', age {}", self.name, self.age));
        });
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
