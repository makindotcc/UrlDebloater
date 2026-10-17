{ pkgs }:

let
  inherit (pkgs) lib;
  isLinux = pkgs.stdenv.hostPlatform.isLinux;

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
    pkgs.cargo
    pkgs.cmake
    pkgs.pkg-config
    pkgs.rust-analyzer
    pkgs.rustc
    pkgs.rustPlatform.rustLibSrc
  ] ++ lib.optionals isLinux [ pkgs.wild ];

  buildInputs = lib.optionals isLinux linuxGuiLibraries ++ lib.optionals (!isLinux) [ pkgs.apple-sdk_26 ];

  env = {
    RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
  } // lib.optionalAttrs isLinux {
    LD_LIBRARY_PATH = lib.makeLibraryPath linuxGuiLibraries;
    CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS = "-C link-arg=-fuse-ld=wild";
  };
}
