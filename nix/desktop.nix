{
  pkgs,
  src,
  cargoLock,
}:

pkgs.rustPlatform.buildRustPackage {
  pname = "urldebloater";
  version = "0.1.4";
  inherit src cargoLock;

  cargoBuildFlags = [
    "-p"
    "urldebloater"
  ];
  cargoTestFlags = [
    "-p"
    "urldebloater"
  ];

  nativeBuildInputs = [
    pkgs.makeWrapper
    pkgs.pkg-config
  ];
  buildInputs = [
    pkgs.gtk3
    pkgs.libayatana-appindicator
    pkgs.openssl
  ];

  postInstall = ''
    wrapProgram $out/bin/urldebloater \
      --prefix LD_LIBRARY_PATH : ${
        pkgs.lib.makeLibraryPath [
          pkgs.libayatana-appindicator
          pkgs.gtk3
        ]
      }
  '';
}
