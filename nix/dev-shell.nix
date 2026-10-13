{ pkgs }:

pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.cargo
    pkgs.cmake
    pkgs.pkg-config
    pkgs.rust-analyzer
    pkgs.rustc
    pkgs.rustPlatform.rustLibSrc
    pkgs.wild
  ];
  buildInputs = [
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
  ];

  env = {
    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
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
    ];
    RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
  };
}
