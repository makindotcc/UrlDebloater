//! Configuration window. Runs as a separate process spawned by the tray, so the
//! application itself does not pay for the GUI runtime while the window is closed.

use anyhow::{Context, Result};
use mitsuami::prelude::*;
use std::path::PathBuf;
use strum::IntoEnumIterator;
use url::Url;
use urldebloater_desktop_config::{Config, RedirectWashPolicy, RedirectWashPolicyDiscriminants};
use urldebloater_urlwasher::{TwitterPolicy, WasherConfig};

const WINDOW_WIDTH: f32 = 560.0;
const WINDOW_HEIGHT: f32 = 320.0;
const LABEL_WIDTH: f32 = 170.0;
/// Where a mixer policy points when the field is empty and the old config has none.
const DEFAULT_MIXER_URL: &str = "https://urldebloater.makin.cc";

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

    // The window is the app: Save and Cancel close it, and the process ends with it.
    // The signals are handles (`Copy`), so the closures a view body builds can stay
    // `Fn` whatever they carry.
    let open = signal(true);
    let path = signal(path);
    let config = signal(config);
    App::new()
        .id("com.urldebloater.Config")
        .name("UrlDebloater Configuration")
        .open(move || {
            Window::new("UrlDebloater Configuration")
                .size(Size::new(WINDOW_WIDTH, WINDOW_HEIGHT))
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
    let policy = signal(RedirectWashPolicyDiscriminants::from(
        &config.redirect_wash_policy,
    ));
    let mixer = signal(
        config
            .redirect_wash_policy
            .mixer_url()
            .map_or_else(String::new, Url::to_string),
    );
    let twitter = signal(config.washer_config.twitter_policy);

    // The mixer URL only matters for the mixer policy.
    let mixer_enabled = move || policy.get() == RedirectWashPolicyDiscriminants::ViaMixer;
    let save = move || {
        let saved_config = saved.get_untracked();
        let result =
            collect_config(&saved_config, debloat, policy, mixer, twitter).and_then(|config| {
                let path = path.get_untracked();
                urldebloater_desktop_config::save_to(&path, &config)?;
                log::info!("Saved configuration to {}", path.display());
                Ok(())
            });
        match result {
            Ok(()) => open.set(false),
            Err(err) => {
                log::error!("Failed to save configuration: {err:#}");
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
                    options=move || policy_labels()
                    selected=move || {
                        combo_index(RedirectWashPolicyDiscriminants::iter(), &policy.get())
                    }
                    @change=move |index| {
                        policy.set(combo_value(RedirectWashPolicyDiscriminants::iter(), index))
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
                    options=move || twitter_labels()
                    selected=move || combo_index(TwitterPolicy::iter(), &twitter.get())
                    @change=move |index| twitter.set(combo_value(TwitterPolicy::iter(), index))
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
    policy: Signal<RedirectWashPolicyDiscriminants>,
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
            RedirectWashPolicyDiscriminants::Ignore => RedirectWashPolicy::Ignore,
            RedirectWashPolicyDiscriminants::Locally => RedirectWashPolicy::Locally,
            RedirectWashPolicyDiscriminants::ViaMixer => RedirectWashPolicy::ViaMixer { mixer_url },
        },
        washer_config: WasherConfig {
            twitter_policy: twitter.get_untracked(),
        },
        ..saved.clone()
    })
}

fn policy_labels() -> Vec<String> {
    RedirectWashPolicyDiscriminants::iter()
        .map(|policy| redirect_wash_label(policy).to_owned())
        .collect()
}

fn twitter_labels() -> Vec<String> {
    TwitterPolicy::iter()
        .map(|policy| twitter_policy_label(&policy).to_owned())
        .collect()
}

fn redirect_wash_label(policy: RedirectWashPolicyDiscriminants) -> &'static str {
    match policy {
        RedirectWashPolicyDiscriminants::Ignore => "Ignore redirects",
        RedirectWashPolicyDiscriminants::Locally => "Resolve redirects locally",
        RedirectWashPolicyDiscriminants::ViaMixer => "Resolve via mixer",
    }
}

fn twitter_policy_label(policy: &TwitterPolicy) -> &'static str {
    match policy {
        TwitterPolicy::Original => "x.com",
        TwitterPolicy::VxTwitter => "VxTwitter.com",
        TwitterPolicy::FxTwitter => "FxTwitter.com",
    }
}

fn combo_value<T: Default>(mut options: impl Iterator<Item = T>, selection: usize) -> T {
    options.nth(selection).unwrap_or_default()
}

fn combo_index<T: PartialEq>(mut options: impl Iterator<Item = T>, wanted: &T) -> usize {
    options
        .position(|candidate| candidate == *wanted)
        .expect("the value is one of the enum's variants")
}
