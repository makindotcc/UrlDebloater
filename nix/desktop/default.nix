{
  pkgs,
  src,
  cargoLock,
  configGui ? true,
}:

let
  inherit (pkgs) lib;
  isLinux = pkgs.stdenv.hostPlatform.isLinux;

  version = (fromTOML (builtins.readFile ../../crates/desktop-daemon/Cargo.toml)).package.version;

  settingsItem = pkgs.makeDesktopItem {
    name = "urldebloater-config";
    desktopName = "UrlDebloater Settings";
    genericName = "URL cleaner";
    comment = "Choose how UrlDebloater washes links";
    exec = "urldebloater-desktop-config-gui";
    icon = "urldebloater";
    terminal = false;
    categories = [ "Utility" ];
    startupNotify = false;
  };
in
pkgs.rustPlatform.buildRustPackage {
  pname = "urldebloater-desktop";
  inherit version src cargoLock;

  cargoBuildFlags = [
    "-p"
    "urldebloater-desktop-daemon"
  ]
  ++ lib.optionals configGui [
    "-p"
    "urldebloater-desktop-config-gui"
  ];
  buildNoDefaultFeatures = !configGui;
  checkNoDefaultFeatures = !configGui;
  cargoTestFlags = [
    "-p"
    "urldebloater-desktop-config"
  ];

  nativeBuildInputs = [
    pkgs.pkg-config
  ]
  ++ lib.optional isLinux pkgs.wrapGAppsHook4;

  buildInputs = lib.optionals isLinux [
    pkgs.gtk4
    pkgs.libadwaita
  ];

  installPhase = ''
    runHook preInstall

    mkdir -p $out/bin
    install -m755 target/${pkgs.stdenv.hostPlatform.rust.rustcTarget}/release/urldebloater-desktop-daemon $out/bin/
  ''
  + lib.optionalString configGui ''
    install -m755 target/${pkgs.stdenv.hostPlatform.rust.rustcTarget}/release/urldebloater-desktop-config-gui $out/bin/
  ''
  + lib.optionalString isLinux ''
    install -Dm644 ${../../assets/icon/tray.png} $out/share/icons/hicolor/256x256/apps/urldebloater.png
  ''
  + lib.optionalString (isLinux && configGui) ''
    install -Dm644 ${settingsItem}/share/applications/urldebloater-config.desktop $out/share/applications/urldebloater-config.desktop
  ''
  + ''
    runHook postInstall
  '';

  meta = {
    description = "UrlDebloater desktop tray application and settings window";
    homepage = "https://github.com/makindotcc/UrlDebloater";
    mainProgram = "urldebloater-desktop-daemon";
    platforms = lib.platforms.linux ++ lib.platforms.darwin;
  };
}
