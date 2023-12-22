{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    rust-crate2nix = {
      url = "github:nix-community/crate2nix";
      flake = false;
    };
  };

  outputs =
    inputs @ { flake-parts
    , rust-crate2nix
    , ...
    }: flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
      ];
      perSystem = { pkgs, ... }:
        let
          crateTools = pkgs.callPackage "${rust-crate2nix}/tools.nix" { };
          project = crateTools.appliedCargoNix {
            name = "urldebloater";
            src = ./.;
          };
        in
        rec {
          packages = {
            urldebloater = project.workspaceMembers.urldebloater.build;
            default = packages.urldebloater;
          };
        };
    };
}
