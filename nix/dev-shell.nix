{ pkgs }:

pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.cargo
    pkgs.pkg-config
    pkgs.rustc
  ];
  buildInputs = [
    pkgs.libayatana-appindicator
    pkgs.openssl
    pkgs.gtk3
  ];

  env = {
    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
      pkgs.libayatana-appindicator
      pkgs.gtk3
    ];
  };
}
