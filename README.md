# UrlDebloater

Remove tracking params from URLs.

### Supported websites
- Youtube & Youtube music (clears url query params)
- Twitter (clears url query params)
- TikTok (unshorts vm.tiktok.com links)
- Soundcloud (unshorts on.soundcloud.com links)
- Spotify (clears url query params on open.spotify.com)

### Features
- automatically extract links from clipboard
- unshorten tiktok per user links (https://vm.tiktok.com/PerUserGeneratedPath) to "anonymous" links (https://tiktok.com/@user/video/852438128934291) \
  ⚠️ it sends request to tiktok in background to achieve this (can still be correlated with your IP address, see mixing capabilities) ⚠️
- tray icon with helpful shortcuts
- gui configuration (native widgets: GTK 4 on Linux, WinUI 3 on Windows, AppKit on macOS)

### Todo
- act as default http url scheme handler, so opened link from non-browser program will be proxied through debloater before opening it in browser of your choice.
- optional [ClearURLs](https://docs.clearurls.xyz/) database support

### Unshortening
To unroll and debloat some URLs like vm.tiktok.com there is need to ask their servers for more information.
UrlDebloater by default sends request from your network to resolve it.\
There is possibility to use [mixer](mixer) (a web server exposing endpoint to unshort URL from network where mixer is hosted, it is AKA "proxy"). I host public instance of mixer at [https://urldebloater.makin.cc](https://urldebloater.makin.cc/). The Android settings screen notes that *Resolve via mixer* sends links from supported services to the configured instance, whose owner can see them. Using untrusted mixer instance may be privacy concern, but is this really scary for tiktok links?

### Showcase

Configuration window:\
![Configuration window](https://github.com/makindotcc/UrlDebloater/assets/9150636/6d482e02-b4f7-4990-b096-24fc62799a8a)

Tray context menu:\
![tray context menu](https://github.com/makindotcc/UrlDebloater/assets/9150636/63090379-fe8f-44c7-bcb0-e2796026366b)

https://github.com/makindotcc/UrlDebloater/assets/9150636/12d83dd8-9c60-4ada-94be-11afbf2ba260

# Configuration

The tray menu's *Settings* entry opens the configuration window — a separate
process (`urldebloater-desktop-config-gui`, expected next to the application binary), so
nothing is allocated for it while it is closed. Saving reloads the configuration
in the running application; no restart is needed.

| system  | config file                                                    |
| ------- | -------------------------------------------------------------- |
| Linux   | `$XDG_CONFIG_HOME/urldebloater/config.json` (default `~/.config`) |
| Windows | `%APPDATA%\urldebloater\config.json`                            |
| macOS   | `~/Library/Application Support/urldebloater/config.json`        |

A `config.json` in the working directory (where older versions kept it) is
migrated on first start.

Policy choices are iterable with `strum::IntoEnumIterator`:
`urldebloater_urlwasher::TwitterPolicy::iter()` and
`urldebloater_desktop_config::RedirectWashPolicyDiscriminants::iter()`.
The settings window stores the redirect variant separately from the mixer URL.

# Repository layout

Crates live in `crates/`:

| crate | role |
| ----- | ---- |
| `urldebloater-urlwasher` | URL washing (query params, redirects, shorteners) |
| `urldebloater-mixer` | optional server for resolving redirects from another network |
| `urldebloater-desktop-daemon` | desktop background process: clipboard, tray, spawning the settings window |
| `urldebloater-desktop-config` | desktop settings model, config path and atomic persistence |
| `urldebloater-desktop-config-gui` | the settings window (native widgets via winio) |
| `urldebloater-android` | Android `cdylib`, JNI initialization and UniFFI binding generator |
| `urldebloater-urlwasher-wasm` | browser build of `TextWasher` with a mixer-backed redirect resolver |

The iOS crate (`urldebloater-ios`) is not there yet; browser extensions are planned under
`extensions/`.

# Nix

The flake packages both desktop binaries for Linux and macOS. On Linux the existing `mixer`
remains the default package; on macOS `desktop` is the default.

```sh
nix build .#desktop
nix run .#desktop
```

The package accepts `configGui = false` through `.override`, for example
`inputs.urldebloater.packages.${system}.desktop.override { configGui = false; }`.

## NixOS

Import the flake's `nixosModules.desktop` and enable the service:

```nix
{
  imports = [ inputs.urldebloater.nixosModules.desktop ];

  services.urldebloater-desktop.enable = true;
  # services.urldebloater-desktop.autostart = false; # optional
  # services.urldebloater-desktop.configGui = false;  # daemon only, no settings window
}
```

The module adds the package to `environment.systemPackages` and, unless `autostart = false`,
starts the daemon through an XDG autostart entry. `configGui = false` builds the daemon without
the `config-gui-integration` feature and does not install the settings window. Updates are
managed by Nix; the application does not self-update.

## nix-darwin

Import the flake's `darwinModules.desktop` and enable the service:

```nix
{
  imports = [ inputs.urldebloater.darwinModules.desktop ];

  services.urldebloater-desktop.enable = true;
  # services.urldebloater-desktop.autostart = false; # optional
  # services.urldebloater-desktop.configGui = false;  # daemon only, no settings window
}
```

The module installs the package into `environment.systemPackages` and, unless
`autostart = false`, manages a launchd agent in `/Library/LaunchAgents`. The agent starts in the
Aqua session for each graphical user. `configGui = false` builds the daemon without the
`config-gui-integration` feature and does not install the settings window. Updates are managed
by Nix; the application does not self-update.

# Web landing page

The landing page is a [Zola](https://www.getzola.org/) site in `web/`. It is plain HTML, CSS,
and JavaScript with no server application. `urldebloater-urlwasher` is compiled to WebAssembly
and runs `TextWasher` in the browser. ClearURLs rules are embedded in the WASM module.
Redirects for the supported short-link domains are resolved by sending only those URLs to the
mixer; the surrounding text and other URLs stay in the browser.

The Nix dev shell includes Zola. Build the WASM package and the site from the repository root:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
wasm-pack build crates/urlwasher-wasm --target web --release \
  --out-dir ../../web/static/wasm \
  --out-name urldebloater_urlwasher_wasm --no-typescript
zola --root web build
```

Zola writes the site to `web/public`. `MIXER_URL` is inserted into the page at build time;
when omitted, it defaults to `http://127.0.0.1:7777/`. Override it for production, for example
`MIXER_URL=https://urldebloater.makin.cc/ zola --root web build`. Deploy `web/public` to any
static host.

For local development:

```sh
zola --root web serve
```

The dev server listens on <http://127.0.0.1:1111> and reloads the page when files change.

Set the production URL when building for Caddy or another domain:

```sh
MIXER_URL=https://urldebloater.makin.cc/ zola --root web build --base-url https://example.com/
```

The default `base_url` in `web/zola.toml` is the GitHub Pages project URL. The `mixer` query
parameter overrides the configured mixer for development, for example
`http://127.0.0.1:1111/?mixer=http%3A%2F%2F127.0.0.1%3A7777%2F`. The mixer allows cross-origin
GET requests so the browser can call it directly.

Do not open the generated HTML directly via `file://`; browsers block ES modules and WASM
loaded from local files.

Screenshots live in `web/static/screenshots/`. The logo is
`web/static/assets/icon/logo.svg`, copied from `assets/icon/logo.svg`.

# Mobile platforms plans

## iOS

### Todo

i need to think about it. \
Maybe add shortcut action, so it can be painlessly run from home screen like here:

https://github.com/makindotcc/UrlDebloater/assets/9150636/7e7de474-ffe9-49a2-ac8c-f75f6006fd34

## Android

The settings screen uses Material 3 Expressive. Resetting settings to their defaults requires confirmation.
The home screen washes arbitrary text in place through the same Rust implementation as desktop.
Its **Clean clipboard** action cleans the current clipboard text directly, without manual pasting
or copying. The same action is reachable from a Quick Settings tile, from a shortcut pinned to the
home screen, and from the launcher long-press menu. A second entry, also available as a tile, a
pinned shortcut and a long-press item, cleans the clipboard and opens its link, asking which one to
open when the text holds several. Other apps can send text through *Share*, and selected text can
be cleaned in place through the text selection menu; both of those clean the incoming text without
touching the clipboard first.

### Native build

Install an Android NDK, `cargo-ndk`, and the Rust standard libraries for the packaged ABIs:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
cargo install cargo-ndk
```

Set `ANDROID_NDK_HOME` if the NDK is not in Android Studio's default location, then build from
`android/`:

```sh
./gradlew assembleDebug
```

Gradle generates the UniFFI Kotlin bindings and packages `liburldebloater.so` for `arm64-v8a`
and `x86_64` at Android API 24. The generated sources and libraries stay under `app/build/generated/`.
The `uniffi` feature on `urldebloater-urlwasher` exports the shared policy types and mobile API;
it also enables `cleanurls-cache`.

ClearURLs data is refreshed at most once every 24 hours and cached as `clearurls-cache.json` in
the app's private files directory. A valid cache is used offline; the bundled catalog is the final
fallback. Native calls use `Dispatchers.IO` because Android's certificate verifier may download
CRLs through the JVM.

### Todo

probably possible same features as in desktop app

# Credits

Tray and app icons provided by [tabler.io](https://tabler.io/icons).
