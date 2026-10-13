//! Configuration window. Runs as a separate process spawned by the tray, so the
//! application itself does not pay for the GUI runtime while the window is closed.

use anyhow::{Context, Result};
use log::{error, info, warn};
use mitsuami::prelude::*;
use std::path::PathBuf;
use url::Url;
use urldebloater_desktop_config::{Config, RedirectWashPolicy};
use urldebloater_urlwasher::{TwitterPolicy, WasherConfig};

const WINDOW_WIDTH: f32 = 560.0;
const LABEL_WIDTH: f32 = 170.0;
/// Where a mixer policy points when the field is empty and the old config has none.
const DEFAULT_MIXER_URL: &str = "https://urldebloater.makin.cc";
/// What the selects offer, in the order they offer it.
const POLICY_LABELS: [&str; 3] = [
    "Ignore redirects",
    "Resolve redirects locally",
    "Resolve via mixer",
];
const TWITTER_LABELS: [&str; 3] = ["x.com", "VxTwitter.com", "FxTwitter.com"];

fn main() -> Result<()> {
    env_logger::init();

    let path = config_path_from_args()?;
    let config = match urldebloater_desktop_config::load_from(&path) {
        Ok(config) => config,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(err) => {
            warn!("Failed to read {}: {err}", path.display());
            Config::default()
        }
    };

    // The window is the app: Save and Cancel close it, and the process ends with it.
    // The signals are handles (`Copy`), so the closures a view body builds can stay
    // `Fn` whatever they carry.
    let open = signal(true);
    let path = signal(path);
    let config = signal(config);
    App::new()
        .id("com.urldebloater.config")
        .name("UrlDebloater Configuration")
        // The tray's icon, so the window shows what the tray shows.
        .icon(AppIcon::bytes(
            include_bytes!("../../desktop-daemon/tray_icon.png").as_slice(),
        ))
        .open(move || {
            Window::new("UrlDebloater Configuration")
                .size(WindowSize::FitHeight(WINDOW_WIDTH))
                .resizable(false)
                .bind(open)
                .content(move || config_form(open, path, config))
        })
        .run();
    Ok(())
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

/// The form, one signal per field so each widget binds to it.
fn config_form(open: Signal<bool>, path: Signal<PathBuf>, saved: Signal<Config>) -> impl View {
    let config = saved.get_untracked();
    let debloat = signal(config.clipboard_debloater);
    let mixer = signal(
        config
            .redirect_wash_policy
            .mixer_url()
            .map_or_else(String::new, Url::to_string),
    );
    let policy = signal(config.redirect_wash_policy);
    let twitter = signal(config.washer_config.twitter_policy);

    // A policy picked in the select carries a URL nobody has typed yet; the save takes
    // the one from the field, so any valid URL will do here.
    let picked_mixer_policy = move || RedirectWashPolicy::ViaMixer {
        mixer_url: Url::parse(mixer.get_untracked().trim())
            .ok()
            .or_else(|| {
                saved
                    .get_untracked()
                    .redirect_wash_policy
                    .mixer_url()
                    .cloned()
            })
            .unwrap_or_else(|| Url::parse(DEFAULT_MIXER_URL).expect("static URL")),
    };
    // The mixer URL only matters for the mixer policy.
    let mixer_enabled = move || matches!(policy.get(), RedirectWashPolicy::ViaMixer { .. });
    let save = move || {
        let saved_config = saved.get_untracked();
        let result =
            collect_config(&saved_config, debloat, policy, mixer, twitter).and_then(|config| {
                let path = path.get_untracked();
                urldebloater_desktop_config::save_to(&path, &config)?;
                info!("Saved configuration to {}", path.display());
                Ok(())
            });
        match result {
            Ok(()) => open.set(false),
            Err(err) => {
                error!("Failed to save configuration: {err:#}");
                let message = format!("{err:#}");
                spawn_local(async move {
                    let _ = alert(
                        Alert::new("Could not save the configuration")
                            .message(message)
                            .button("OK"),
                    )
                    .await;
                });
            }
        }
    };

    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Checkbox bind=debloat>"Debloat clipboard"</Checkbox>
            <Row gap=Spacing::Md align=Align::Center>
                <Text width=LABEL_WIDTH>"Redirect wash policy"</Text>
                <Select
                    label="Redirect wash policy"
                    options=POLICY_LABELS
                    selected=move || match policy.get() {
                        RedirectWashPolicy::Ignore => 0,
                        RedirectWashPolicy::Locally => 1,
                        RedirectWashPolicy::ViaMixer { .. } => 2,
                    }
                    @change=move |index| {
                        policy.set(match index {
                            0 => RedirectWashPolicy::Ignore,
                            1 => RedirectWashPolicy::Locally,
                            2 => picked_mixer_policy(),
                            _ => panic!("Invalid index for redirect wash policy")
                        })
                    }
                />
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <Text width=LABEL_WIDTH>"Mixer URL"</Text>
                <TextInput a11y_label="Mixer URL" bind=mixer enabled=mixer_enabled/>
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <Text width=LABEL_WIDTH>"Twitter policy"</Text>
                <Select
                    label="Twitter policy"
                    options=TWITTER_LABELS
                    selected=move || match twitter.get() {
                        TwitterPolicy::Original => 0,
                        TwitterPolicy::VxTwitter => 1,
                        TwitterPolicy::FxTwitter => 2,
                    }
                    @change=move |index| {
                        twitter.set(match index {
                            0 => TwitterPolicy::Original,
                            1 => TwitterPolicy::VxTwitter,
                            2 => TwitterPolicy::FxTwitter,
                            _ => panic!("Invalid index for twitter policy")
                        })
                    }
                />
            </Row>
            <Row gap=Spacing::Sm justify=Justify::End>
                <Button role=ButtonRole::Cancel @click=move || open.set(false)>"Cancel"</Button>
                <Button role=ButtonRole::Default @click=save>"Save"</Button>
            </Row>
        </Column>
    }
}

fn collect_config(
    saved: &Config,
    debloat: Signal<bool>,
    policy: Signal<RedirectWashPolicy>,
    mixer: Signal<String>,
    twitter: Signal<TwitterPolicy>,
) -> Result<Config> {
    let mixer_url = match mixer.get_untracked() {
        text if !text.trim().is_empty() => {
            Url::parse(text.trim()).context("mixer URL is not a valid URL")?
        }
        _ => saved
            .redirect_wash_policy
            .mixer_url()
            .cloned()
            .unwrap_or_else(|| Url::parse(DEFAULT_MIXER_URL).expect("static URL")),
    };

    Ok(Config {
        clipboard_debloater: debloat.get_untracked(),
        redirect_wash_policy: match policy.get_untracked() {
            RedirectWashPolicy::ViaMixer { .. } => RedirectWashPolicy::ViaMixer { mixer_url },
            policy => policy,
        },
        washer_config: WasherConfig {
            twitter_policy: twitter.get_untracked(),
        },
        ..saved.clone()
    })
}
