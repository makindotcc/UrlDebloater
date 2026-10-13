//! Configuration window. Runs as a separate process spawned by the tray, so the
//! application itself does not pay for the GUI runtime while the window is closed.

use std::path::PathBuf;

use anyhow::{Context, Result};
use urldebloater_desktop_config::{Config, RedirectWashPolicy};
use url::Url;
use urldebloater_urlwasher::TwitterPolicy;
use winio::prelude::*;

const MARGIN: f64 = 12.0;
// GTK and WinUI controls are at least ~34px tall; keeping rows this tall avoids
// overlapping hit areas.
const ROW_HEIGHT: f64 = 34.0;
const WINDOW_WIDTH: f64 = 560.0;
const WINDOW_HEIGHT: f64 = 320.0;
const LABEL_WIDTH: f64 = 170.0;

const POLICIES: [&str; 3] = [
    "Ignore redirects",
    "Resolve redirects locally",
    "Resolve via mixer",
];
const TWITTER_POLICIES: [&str; 3] = ["Original", "VxTwitter", "FxTwitter"];

fn main() -> Result<()> {
    env_logger::init();

    let path = config_path_from_args()?;
    let config = match urldebloater_desktop_config::load_from(&path) {
        Ok(config) => config,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(err) => {
            log::warn!("Failed to read {}: {err}", path.display());
            Config::default()
        }
    };

    App::builder()
        .build()?
        .block_on(ConfigWindow::run_until_event((path, config)))
}

fn config_path_from_args() -> Result<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--config" {
            let path = args.next().context("--config needs a path")?;
            return Ok(PathBuf::from(path));
        }
    }
    Ok(urldebloater_desktop_config::config_path())
}

struct ConfigWindow {
    path: PathBuf,
    config: Config,
    window: Child<Window>,
    debloat: Child<CheckBox>,
    policy_label: Child<Label>,
    policy: Child<ComboBox>,
    mixer_label: Child<Label>,
    mixer: Child<Edit>,
    twitter_label: Child<Label>,
    twitter: Child<ComboBox>,
    save: Child<Button>,
    cancel: Child<Button>,
}

#[derive(Debug)]
enum Message {
    Noop,
    PolicyChanged,
    Save,
    Cancel,
}

impl Component for ConfigWindow {
    type Error = anyhow::Error;
    type Event = ();
    type Init<'a> = (PathBuf, Config);
    type Message = Message;

    async fn init((path, config): Self::Init<'_>, _sender: &ComponentSender<Self>) -> Result<Self> {
        init! {
            window: Window = (()) => {
                text: "UrlDebloater Configuration",
                size: Size::new(WINDOW_WIDTH, WINDOW_HEIGHT),
            },
            debloat: CheckBox = (&window) => {
                text: "Debloat clipboard",
            },
            policy_label: Label = (&window) => {
                text: "Redirect wash policy",
            },
            policy: ComboBox = (&window),
            mixer_label: Label = (&window) => {
                text: "Mixer URL",
            },
            mixer: Edit = (&window),
            twitter_label: Label = (&window) => {
                text: "Twitter policy",
            },
            twitter: ComboBox = (&window),
            save: Button = (&window) => {
                text: "Save",
            },
            cancel: Button = (&window) => {
                text: "Cancel",
            },
        }

        debloat.set_checked(config.clipboard_debloater)?;
        policy.set_items(POLICIES)?;
        policy.set_selection(config.redirect_wash_policy.index())?;
        mixer.set_text(
            config
                .redirect_wash_policy
                .mixer_url()
                .map_or("", Url::as_str),
        )?;
        twitter.set_items(TWITTER_POLICIES)?;
        twitter.set_selection(twitter_index(&config.washer_config.twitter_policy))?;

        window.show()?;

        Ok(Self {
            path,
            config,
            window,
            debloat,
            policy_label,
            policy,
            mixer_label,
            mixer,
            twitter_label,
            twitter,
            save,
            cancel,
        })
    }

    async fn start(&mut self, sender: &ComponentSender<Self>) -> ! {
        start! {
            sender,
            default: Message::Noop,
            self.window => {
                WindowEvent::Close => Message::Cancel,
            },
            self.debloat => {
                CheckBoxEvent::Click => Message::Noop,
            },
            self.policy => {
                ComboBoxEvent::Select => Message::PolicyChanged,
                ComboBoxEvent::Change => Message::PolicyChanged,
            },
            self.mixer => {
                EditEvent::Change => Message::Noop,
            },
            self.twitter => {
                ComboBoxEvent::Select => Message::Noop,
                ComboBoxEvent::Change => Message::Noop,
            },
            self.save => {
                ButtonEvent::Click => Message::Save,
            },
            self.cancel => {
                ButtonEvent::Click => Message::Cancel,
            },
        }
    }

    async fn update_children(&mut self) -> Result<bool> {
        update_children!(
            self.window,
            self.debloat,
            self.policy_label,
            self.policy,
            self.mixer_label,
            self.mixer,
            self.twitter_label,
            self.twitter,
            self.save,
            self.cancel,
        )
    }

    async fn update(
        &mut self,
        message: Self::Message,
        sender: &ComponentSender<Self>,
    ) -> Result<bool> {
        log::debug!("message: {message:?}");
        match message {
            Message::Noop => Ok(false),
            // The mixer URL only applies to the mixer policy.
            Message::PolicyChanged => Ok(true),
            Message::Save => {
                let config = self.collect_config()?;
                urldebloater_desktop_config::save_to(&self.path, &config)
                    .with_context(|| format!("save {}", self.path.display()))?;
                log::info!("Saved configuration to {}", self.path.display());
                sender.output(());
                Ok(false)
            }
            Message::Cancel => {
                sender.output(());
                Ok(false)
            }
        }
    }

    fn render(&mut self, _sender: &ComponentSender<Self>) -> Result<()> {
        let size = self.window.client_size()?;
        log::debug!("render: client size {size:?}");
        let width = (size.width - MARGIN * 2.0).max(0.0);
        let field_x = MARGIN + LABEL_WIDTH;
        let field_width = (width - LABEL_WIDTH).max(0.0);
        let mut y = MARGIN;

        self.debloat
            .set_rect(Rect::new(Point::new(MARGIN, y), Size::new(width, ROW_HEIGHT)))?;
        y += ROW_HEIGHT;

        self.policy_label
            .set_rect(Rect::new(Point::new(MARGIN, y), Size::new(LABEL_WIDTH, ROW_HEIGHT)))?;
        self.policy
            .set_rect(Rect::new(Point::new(field_x, y), Size::new(field_width, ROW_HEIGHT)))?;
        y += ROW_HEIGHT;

        // The mixer URL only matters for the mixer policy.
        let mixer_enabled = self.policy.selection()? == Some(2);
        self.mixer_label.set_enabled(mixer_enabled)?;
        self.mixer.set_enabled(mixer_enabled)?;
        self.mixer_label
            .set_rect(Rect::new(Point::new(MARGIN, y), Size::new(LABEL_WIDTH, ROW_HEIGHT)))?;
        self.mixer
            .set_rect(Rect::new(Point::new(field_x, y), Size::new(field_width, ROW_HEIGHT)))?;
        y += ROW_HEIGHT;

        self.twitter_label
            .set_rect(Rect::new(Point::new(MARGIN, y), Size::new(LABEL_WIDTH, ROW_HEIGHT)))?;
        self.twitter
            .set_rect(Rect::new(Point::new(field_x, y), Size::new(field_width, ROW_HEIGHT)))?;

        let buttons_y = (size.height - MARGIN - ROW_HEIGHT).max(y + ROW_HEIGHT);
        let button_width = 100.0;
        self.save.set_rect(Rect::new(
            Point::new(MARGIN + width - button_width * 2.0 - MARGIN, buttons_y),
            Size::new(button_width, ROW_HEIGHT),
        ))?;
        self.cancel.set_rect(Rect::new(
            Point::new(MARGIN + width - button_width, buttons_y),
            Size::new(button_width, ROW_HEIGHT),
        ))?;
        Ok(())
    }

    fn render_children(&mut self) -> Result<()> {
        Ok(self.window.render()?)
    }
}

impl ConfigWindow {
    fn collect_config(&self) -> Result<Config> {
        let mixer_url = match self.mixer.text() {
            Ok(text) if !text.trim().is_empty() => Url::parse(text.trim())
                .context("mixer URL is not a valid URL")?,
            _ => self
                .config
                .redirect_wash_policy
                .mixer_url()
                .cloned()
                .unwrap_or_else(|| Url::parse("https://urldebloater.makin.cc").expect("static URL")),
        };

        Ok(Config {
            clipboard_debloater: self.debloat.is_checked()?,
            redirect_wash_policy: RedirectWashPolicy::from_index(
                self.policy.selection()?.unwrap_or(1),
                mixer_url,
            ),
            washer_config: urldebloater_urlwasher::WasherConfig {
                twitter_policy: twitter_policy_from_index(self.twitter.selection()?.unwrap_or(0)),
            },
            ..self.config.clone()
        })
    }
}

fn twitter_index(policy: &TwitterPolicy) -> usize {
    match policy {
        TwitterPolicy::Original => 0,
        TwitterPolicy::VxTwitter => 1,
        TwitterPolicy::FxTwitter => 2,
    }
}

fn twitter_policy_from_index(index: usize) -> TwitterPolicy {
    match index {
        1 => TwitterPolicy::VxTwitter,
        2 => TwitterPolicy::FxTwitter,
        _ => TwitterPolicy::Original,
    }
}
