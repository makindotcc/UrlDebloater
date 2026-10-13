{
  pkgs,
  src,
  cargoLock,
  rustPlatform,
}:

rustPlatform.buildRustPackage {
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
    pkgs.pkg-config
  ];
  buildInputs = [ ];
}
