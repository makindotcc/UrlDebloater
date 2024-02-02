# UrlDebloater

Remove tracking params from URLs.

## Desktop (Windows/mac/linux)

### Features

- automatically extract links from clipboard and remove tracking parameters.
- unshorten tiktok per user links (https://vm.tiktok.com/PerUserGeneratedPath) to "anonymous" links (https://tiktok.com/@user/video/852438128934291) \
  ⚠️ it sends request to tiktok in background to achieve this (can still be correlated with your IP address, see mixing capabilities) ⚠️
- tray icon with helpful shortcuts
- gui configuration

### Todo

- installer
- act as default http url scheme handler, so opened link from non-browser program will be proxied through debloater before opening it in browser of your choice.
- optional [ClearURLs](https://docs.clearurls.xyz/) database support

### Unshortening
To unroll and debloat some URLs like vm.tiktok.com there is need to ask their servers for more information.
UrlDebloater by default sends request from your network to resolve it.\
There is possibility to use [mixer](mixer) (a web server exposing endpoint to unshort URL from network where mixer is hosted). I host public instance of mixer at [https://urldebloater.makin.cc](https://urldebloater.makin.cc/). Using untrusted mixer instance may be privacy concern, but is this really scary for tiktok links?

### Supported websites
- Youtube (clears url query params)
- Twitter (clears url query params)
- TikTok (unshorts vm.tiktok.com links)
- Soundcloud (unshorts on.soundcloud.com links)

### Showcase

https://github.com/makindotcc/UrlDebloater/assets/9150636/12d83dd8-9c60-4ada-94be-11afbf2ba260

## iOS

### Todo

i need to think about it. \
Maybe add shortcut action, so it can be painlessly run from home screen like here:

https://github.com/makindotcc/UrlDebloater/assets/9150636/7e7de474-ffe9-49a2-ac8c-f75f6006fd34

## Android

### Todo

probably possible same features as in desktop app

## Credits

Tray icon provided by [tabler.io](https://tabler.io/icons).
