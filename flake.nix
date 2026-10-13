{
  description = "UrlDebloater packages and development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

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
      packages.${system} = rec {
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

      devShells.${system}.default = import ./nix/dev-shell.nix { inherit pkgs; };

      nixosModules.default = mixerOutput.module;
    };
}
