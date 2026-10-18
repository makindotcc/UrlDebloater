# UrlDebloater browser extension

Chrome and Firefox Manifest V3 extensions clean every HTTP(S) main-frame navigation with
`urlwasher-wasm`. ClearURLs removes tracking parameters. Redirect resolution applies only to
`vm.tiktok.com` and `on.soundcloud.com`.

## Build

From the repository root, install the prerequisites and run the build:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
sh extensions/browser/build.sh
```

The build also requires `zip`. It writes unpacked packages to `extensions/browser/dist/chrome/`
and `extensions/browser/dist/firefox/`, plus `urldebloater-chrome.zip` and
`urldebloater-firefox.zip` under `extensions/browser/dist/`.

## Load

- Chrome: open `chrome://extensions`, enable Developer mode, choose **Load unpacked**, and select
  `extensions/browser/dist/chrome/`.
- Firefox: open `about:debugging#/runtime/this-firefox`, choose **Load Temporary Add-on**, and
  select `extensions/browser/dist/firefox/manifest.json`.

## Settings

The options page is available in English and Polish and stores settings with `storage.sync`.

- **Clean URLs during navigation** is enabled by default.
- **Off** removes tracking parameters without unrolling short links.
- **Local in browser** is the default. The browser resolves short links directly without sending
  cookies; the shortener sees the browser's IP address.
- **Mixer** sends supported short links to the configured mixer instead. The default mixer is
  `https://urldebloater.makin.cc/`; the mixer sees the links it resolves.

## Chrome request behavior

Chrome does not provide blocking `webRequest` for arbitrary asynchronous washing. When local or
mixer short-link resolution is enabled, a `declarativeNetRequest` rule instead redirects the
main-frame request to a local waiting page before it reaches the network. The washed destination
replaces that page after resolution; if resolution fails, the original URL remains unopened.

For other URLs, the original request may briefly start before `tabs.update` removes tracking
parameters. Firefox uses blocking `webRequest`, so dirty requests do not reach the network.
