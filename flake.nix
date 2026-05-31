{
  description = "UrlDebloater packages and development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      src = pkgs.lib.cleanSourceWith {
        src = ./.;
        filter = path: _: (baseNameOf path) != "target";
      };
      cargoLock = {
        lockFile = ./Cargo.lock;
        outputHashes = {
          "ecolor-0.22.0" = "sha256-ABNf+iCNo6L9GnZGON6nQvsYIcMZ7JbNV9cD8LdWwus=";
        };
      };
      mixerOutput = import ./nix/mixer.nix {
        inherit
          self
          pkgs
          src
          cargoLock
          ;
      };
    in
    {
      packages.${system} = rec {
        mixer = mixerOutput.package;
        desktop = import ./nix/desktop.nix {
          inherit
            pkgs
            src
            cargoLock
            ;
        };
        default = mixer;
      };

      devShells.${system}.default = import ./nix/dev-shell.nix { inherit pkgs; };

      nixosModules.default = mixerOutput.module;
    };
}
