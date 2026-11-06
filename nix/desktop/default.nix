{
  pkgs,
  src,
  cargoLock,
  configGui ? true,
}:

let
  inherit (pkgs) lib;
  isLinux = pkgs.stdenv.hostPlatform.isLinux;
  isDarwin = pkgs.stdenv.hostPlatform.isDarwin;

  version = (lib.importTOML ../../Cargo.toml).workspace.package.version;

  infoPlist = lib.generators.toPlist { escape = true; } {
    CFBundleDevelopmentRegion = "en";
    CFBundleDisplayName = "UrlDebloater";
    CFBundleExecutable = "urldebloater-desktop-daemon";
    CFBundleIconFile = "AppIcon";
    CFBundleIconName = "AppIcon";
    CFBundleIdentifier = "com.urldebloater.desktop";
    CFBundleInfoDictionaryVersion = "6.0";
    CFBundleName = "UrlDebloater";
    CFBundlePackageType = "APPL";
    CFBundleShortVersionString = version;
    CFBundleVersion = version;
    LSApplicationCategoryType = "public.app-category.utilities";
    LSMinimumSystemVersion = "11.0";
    LSUIElement = true;
    NSHighResolutionCapable = true;
  };

  appItem = pkgs.makeDesktopItem {
    name = "urldebloater";
    desktopName = "UrlDebloater";
    genericName = "URL cleaner";
    comment = "Remove tracking parameters from links copied to the clipboard";
    exec = "urldebloater-desktop-daemon";
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
  ++ lib.optional isLinux pkgs.wrapGAppsHook4
  ++ lib.optionals isDarwin [
    pkgs.imagemagick
    pkgs.libicns
  ];

  buildInputs = lib.optionals isLinux [
    pkgs.gtk4
    pkgs.libadwaita
    pkgs.wayland
  ];

  preFixup = lib.optionalString isLinux ''
    gappsWrapperArgs+=(--prefix LD_LIBRARY_PATH : "${lib.makeLibraryPath [ pkgs.wayland ]}")
  '';

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
    install -Dm644 ${appItem}/share/applications/urldebloater.desktop $out/share/applications/urldebloater.desktop
  ''
  + lib.optionalString isDarwin ''
    appBundle="$out/Applications/UrlDebloater.app"
    mkdir -p "$appBundle/Contents/"{MacOS,Resources}

    printf '%s' ${lib.escapeShellArg infoPlist} > "$appBundle/Contents/Info.plist"
    printf 'APPL????' > "$appBundle/Contents/PkgInfo"
    iconDir="$(mktemp -d)"
    magick ${../../assets/icon/tray.png} -filter triangle -resize 1024x1024 "$iconDir/AppIcon.png"
    png2icns "$appBundle/Contents/Resources/AppIcon.icns" "$iconDir/AppIcon.png"
    rm -rf "$iconDir"

    mv "$out/bin/urldebloater-desktop-daemon" "$appBundle/Contents/MacOS/"
    ln -s "$appBundle/Contents/MacOS/urldebloater-desktop-daemon" "$out/bin/urldebloater-desktop-daemon"
    ${lib.optionalString configGui ''
      mv "$out/bin/urldebloater-desktop-config-gui" "$appBundle/Contents/MacOS/"
      ln -s "$appBundle/Contents/MacOS/urldebloater-desktop-config-gui" "$out/bin/urldebloater-desktop-config-gui"
    ''}
  ''
  + ''
    runHook postInstall
  '';

  postFixup = lib.optionalString isDarwin ''
    /usr/bin/codesign --force --deep --sign - "$out/Applications/UrlDebloater.app"
  '';

  meta = {
    description = "UrlDebloater desktop tray application and settings window";
    homepage = "https://github.com/makindotcc/UrlDebloater";
    mainProgram = "urldebloater-desktop-daemon";
    platforms = lib.platforms.linux ++ lib.platforms.darwin;
  };
}
