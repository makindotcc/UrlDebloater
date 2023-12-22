# UrlDebloater
Remove tracking params from URLs.

## Desktop (Windows/mac/linux)

### Features
- automatically extract links from clipboard and remove tracking parameters.
- unshorten tiktok per user links (https://vm.tiktok.com/PerUserGeneratedPath) to "anonymous" links (https://tiktok.com/@user/video/852438128934291) \
⚠️ it sends request to tiktok in background to achieve this (can still be correlated with your IP address, see todo) ⚠️

### Todo
- installer
- act as default http url scheme handler, so opened link from non-browser program will be proxied through debloater before opening it in browser of your choice.
- optional [ClearURLs](https://docs.clearurls.xyz/) database support
- gui configuration
- tray icon with helpful shortcuts
- ❓ unshorten tiktok links via proxy (socks/http or rest api of url-debloater (self-hosted or mine public instance)) ❓

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
