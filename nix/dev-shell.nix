{ pkgs }:

let
  inherit (pkgs) lib;
  isLinux = pkgs.stdenv.hostPlatform.isLinux;

  rustToolchain = pkgs.rust-bin.stable.latest.default.override {
    extensions = [
      "rust-analyzer"
      "rust-src"
    ];
    targets = [
      "aarch64-linux-android"
      "x86_64-linux-android"
    ];
  };

  androidNdk = pkgs.androidenv.composeAndroidPackages {
    includeNDK = true;
    ndkVersions = [ "27.3.13750724" ];
    includeEmulator = false;
    includeSystemImages = false;
  };
  androidNdkHome = "${androidNdk.ndk-bundle}/libexec/android-sdk/ndk-bundle";

  linuxGuiLibraries = [
    pkgs.atk
    pkgs.cairo
    pkgs.fontconfig
    pkgs.gdk-pixbuf
    pkgs.glib
    pkgs.gtk3
    pkgs.gtk4
    pkgs.harfbuzz
    pkgs.libadwaita
    pkgs.libappindicator-gtk3
    pkgs.libx11
    pkgs.libxcursor
    pkgs.libxext
    pkgs.libxfixes
    pkgs.libxft
    pkgs.libxinerama
    pkgs.libxrender
    pkgs.pango
    pkgs.qt6.qtbase
    pkgs.qt6.qtwayland
    pkgs.wayland
  ];
in
pkgs.mkShell {
  nativeBuildInputs = [
    rustToolchain
    pkgs.cmake
    pkgs.jdk21
    pkgs.ktlint
    pkgs.pkg-config
    pkgs.zola
  ]
  ++ lib.optionals isLinux [
    pkgs.cargo-ndk
    pkgs.wild
  ];

  buildInputs =
    lib.optionals isLinux linuxGuiLibraries ++ lib.optionals (!isLinux) [ pkgs.apple-sdk_26 ];

  env = {
    RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
  }
  // lib.optionalAttrs isLinux {
    ANDROID_NDK_HOME = androidNdkHome;
    ANDROID_NDK_ROOT = androidNdkHome;
    LD_LIBRARY_PATH = lib.makeLibraryPath linuxGuiLibraries;
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C link-arg=-fuse-ld=wild";
  };
}
