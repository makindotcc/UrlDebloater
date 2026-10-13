{
  description = "UrlDebloater packages and development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      linuxSystem = "x86_64-linux";
      systems = [
        linuxSystem
        "aarch64-darwin"
      ];
      pkgs = import nixpkgs { system = linuxSystem; };
      src = pkgs.lib.cleanSourceWith {
        src = ./.;
        filter = path: _: (baseNameOf path) != "target";
      };
      cargoLock = {
        lockFile = ./Cargo.lock;
      };
      stdenv = pkgs.useWildLinker pkgs.stdenv;
      rustPlatform = pkgs.makeRustPlatform {
        inherit (pkgs) cargo rustc;
        inherit stdenv;
      };
      mixerOutput = import ./nix/mixer.nix {
        inherit
          self
          pkgs
          src
          cargoLock
          rustPlatform
          ;
      };
    in
    {
      packages.${linuxSystem} = rec {
        mixer = mixerOutput.package;
        desktop = import ./nix/desktop.nix {
          inherit
            pkgs
            src
            cargoLock
            rustPlatform
            ;
        };
        default = mixer;
      };

      devShells = pkgs.lib.genAttrs systems (system: {
        default = import ./nix/dev-shell.nix {
          pkgs = import nixpkgs { inherit system; };
        };
      });

      nixosModules.default = mixerOutput.module;
    };
}
