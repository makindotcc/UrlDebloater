//! Configuration window. Runs as a separate process spawned by the tray, so the
//! application itself does not pay for the GUI runtime while the window is closed.

use anyhow::{Context, Result};
use log::{error, info, warn};
use mitsuami::prelude::*;
use std::path::PathBuf;
use strum::{IntoDiscriminant, IntoEnumIterator};
use url::Url;
use urldebloater_desktop_config::{Config, RedirectWashPolicy, RedirectWashPolicyDiscriminants};
use urldebloater_urlwasher::{PUBLIC_MIXER_INSTANCE, TwitterPolicy};

const WINDOW_WIDTH: f32 = 560.0;
const LABEL_WIDTH: f32 = 170.0;

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

    let open = signal(true);
    let config = signal(config);
    info!("Starting UrlDebloater Configuration GUI");
    App::new()
        .id("com.urldebloater.config")
        .name("UrlDebloater Configuration")
        .icon(AppIcon::bytes(
            include_bytes!(concat!(env!("CARGO_WORKSPACE_DIR"), "assets/icon/tray.png")).as_slice(),
        ))
        .open(move || {
            let path = path.clone();
            Window::new("UrlDebloater Configuration")
                .size(WindowSize::FollowHeight(WINDOW_WIDTH))
                .resizable(false)
                .bind(open)
                .content(move || config_form(open, path.clone(), config))
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

#[derive(Clone)]
struct FormDraft {
    clipboard_debloater: bool,
    redirect_wash_policy: RedirectWashPolicyDiscriminants,
    mixer_url: String,
    use_clear_urls: bool,
    twitter_policy: TwitterPolicy,
}

impl FormDraft {
    fn differs_from(&self, saved: &Config) -> bool {
        match collect_config(saved, self) {
            Ok(config) => config != *saved,
            Err(_) => true,
        }
    }
}

impl From<Config> for FormDraft {
    fn from(config: Config) -> Self {
        Self {
            clipboard_debloater: config.clipboard_debloater,
            redirect_wash_policy: config.redirect_wash_policy.discriminant(),
            mixer_url: config
                .redirect_wash_policy
                .mixer_url()
                .map_or_else(|| String::from(PUBLIC_MIXER_INSTANCE), Url::to_string),
            use_clear_urls: config.use_clear_urls,
            twitter_policy: config.twitter_policy,
        }
    }
}

fn config_form(open: Signal<bool>, path: PathBuf, saved: Signal<Config>) -> impl View {
    let config = saved.get_untracked();
    let draft: Signal<FormDraft> = signal(config.into());
    let dirty = computed(move || saved.with(|saved| draft.with(|draft| draft.differs_from(saved))));
    let saving = signal(false);

    let reset_to_defaults = move || draft.set(Config::default().into());
    let save = {
        let path = path.clone();
        move || save_config(open, saved, saving, draft, false, path.clone())
    };
    let ok = move || save_config(open, saved, saving, draft, true, path.clone());
    let can_save = move || !saving.get() && dirty.get();

    view! {
        <Column padding=Spacing::Xl gap=Spacing::Md>
            <Row gap=Spacing::Sm justify=Justify::SpaceBetween align=Align::Center>
                <Row gap=Spacing::Md>
                    <Checkbox
                        checked=move || draft.with(|draft| draft.clipboard_debloater)
                        @change=move |checked| draft.update(|draft| draft.clipboard_debloater = checked)
                    >
                        "Debloat clipboard"
                    </Checkbox>
                    <Checkbox
                        checked=move || draft.with(|draft| draft.use_clear_urls)
                        @change=move |checked| draft.update(|draft| draft.use_clear_urls = checked)
                    >
                        "Use Clear URLs database"
                    </Checkbox>
                </Row>
                <Button
                    icon=reset_icon()
                    icon_only=true
                    tooltip="Reset to defaults"
                    @click=reset_to_defaults
                >
                    "Reset to defaults"
                </Button>
            </Row>
            <Row gap=Spacing::Md align=Align::Center>
                <Text width=LABEL_WIDTH>"Redirect wash policy"</Text>
                <Select
                    grow=1.0
                    label="Redirect wash policy"
                    options=policy_labels()
                    selected=move || {
                        draft.with(|draft| {
                            combo_index(RedirectWashPolicyDiscriminants::iter(), &draft.redirect_wash_policy)
                        })
                    }
                    @change=move |index| draft.update(|draft| {
                        draft.redirect_wash_policy =
                            combo_value(RedirectWashPolicyDiscriminants::iter(), index);
                    })
                />
            </Row>
            {mixer_url_row(draft)}
            <Row gap=Spacing::Md align=Align::Center>
                <Text width=LABEL_WIDTH>"Twitter domain"</Text>
                <Select
                    grow=1.0
                    label="Twitter domain"
                    options=twitter_labels()
                    selected=move || {
                        draft.with(|draft| combo_index(TwitterPolicy::iter(), &draft.twitter_policy))
                    }
                    @change=move |index| draft.update(|draft| {
                        draft.twitter_policy = combo_value(TwitterPolicy::iter(), index);
                    })
                />
            </Row>
            <Row gap=Spacing::Sm justify=Justify::SpaceBetween align=Align::Center>
                <Button role=ButtonRole::Cancel @click=move || open.set(false)>"Cancel"</Button>
                <Row gap=Spacing::Sm>
                    <Button role=ButtonRole::Normal @click=save enabled=can_save>"Save"</Button>
                    <Button role=ButtonRole::Default @click=ok>"Ok"</Button>
                </Row>
            </Row>
        </Column>
    }
}

fn save_config(
    open: Signal<bool>,
    saved: Signal<Config>,
    saving: Signal<bool>,
    draft: Signal<FormDraft>,
    close: bool,
    path: PathBuf,
) {
    if saving.get_untracked() {
        return;
    }
    let saved_config = saved.get_untracked();
    let draft_value = draft.get_untracked();
    saving.set(true);
    spawn_local(async move {
        let result = spawn_blocking(move || -> Result<Config> {
            let config = collect_config(&saved_config, &draft_value)?;
            urldebloater_desktop_config::save_to(&path, &config)?;
            info!("Saved configuration to {}", path.display());
            Ok(config)
        })
        .await;
        saving.set(false);
        match result {
            Ok(config) => {
                saved.set(config);
                if close {
                    open.set(false);
                }
            }
            Err(err) => {
                error!("Failed to save configuration: {err:#}");
                let _ = alert(
                    Alert::new("Could not save the configuration")
                        .message(format!("{err:#}"))
                        .button("OK"),
                )
                .await;
            }
        }
    });
}

fn mixer_url_row(draft: Signal<FormDraft>) -> impl View {
    let default_url: Url = PUBLIC_MIXER_INSTANCE.parse().expect("static URL");
    let enabled = move || {
        draft.with(|draft| {
            matches!(
                draft.redirect_wash_policy,
                RedirectWashPolicyDiscriminants::ViaMixer
            )
        })
    };
    let not_default = move || {
        !draft.with(|draft| Url::parse(draft.mixer_url.trim()).is_ok_and(|url| url == default_url))
    };
    let invalid = move || {
        enabled() && Url::parse(draft.with(|draft| draft.mixer_url.clone()).trim()).is_err()
    };

    view! {
        <Row gap=Spacing::Md align=Align::Center>
            <Text width=LABEL_WIDTH>"Mixer URL"</Text>
            <Column grow=1.0 gap=Spacing::Xs>
                <Row gap=Spacing::Sm>
                    <TextInput
                        grow=1.0
                        a11y_label="Mixer URL"
                        value=move || draft.with(|draft| draft.mixer_url.clone())
                        @input=move |text| draft.update(|draft| draft.mixer_url = text)
                        enabled=enabled
                    />
                    <Button
                        role=ButtonRole::Normal
                        @click=move || draft.update(|draft| draft.mixer_url = PUBLIC_MIXER_INSTANCE.to_string())
                        enabled=move || enabled() && not_default()
                    >
                        "default"
                    </Button>
                </Row>
                <Show when=invalid>
                    <Text text_style=TextStyle::Caption color=Color::Error>"Enter a valid URL, like https://example.com"</Text>
                </Show>
            </Column>
        </Row>
    }
}

fn collect_config(saved: &Config, draft: &FormDraft) -> Result<Config> {
    Ok(Config {
        clipboard_debloater: draft.clipboard_debloater,
        redirect_wash_policy: match draft.redirect_wash_policy {
            RedirectWashPolicyDiscriminants::ViaMixer => RedirectWashPolicy::ViaMixer {
                mixer_url: Url::parse(draft.mixer_url.trim())
                    .context("mixer URL is not a valid URL")?,
            },
            RedirectWashPolicyDiscriminants::Ignore => RedirectWashPolicy::Ignore,
            RedirectWashPolicyDiscriminants::Locally => RedirectWashPolicy::Locally,
        },
        use_clear_urls: draft.use_clear_urls,
        twitter_policy: draft.twitter_policy,
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

fn reset_icon() -> &'static str {
    platform! {
        macos => "arrow.counterclockwise",
        gtk => "edit-undo-symbolic",
        windows => "\u{E7A7}",
    }
}

fn combo_value<T>(mut options: impl Iterator<Item = T>, selection: usize) -> T {
    options
        .nth(selection)
        .expect("the selection is one of the enum's variants")
}

fn combo_index<T: PartialEq>(mut options: impl Iterator<Item = T>, wanted: &T) -> usize {
    options
        .position(|candidate| candidate == *wanted)
        .expect("the value is one of the enum's variants")
}
