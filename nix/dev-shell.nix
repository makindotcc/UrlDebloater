{ pkgs }:

pkgs.mkShell {
  nativeBuildInputs = [
    pkgs.cargo
    pkgs.pkg-config
    pkgs.rustc
  ];
  buildInputs = [
    pkgs.openssl
    pkgs.gtk3
  ];
}
