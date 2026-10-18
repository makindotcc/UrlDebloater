{
  description = "UrlDebloater packages and development shell";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      inherit (nixpkgs) lib;

      systems = [
        "x86_64-linux"
        "aarch64-darwin"
      ];
      forAllSystems = lib.genAttrs systems;

      src = lib.cleanSourceWith {
        src = ./.;
        filter =
          path: _type:
          let
            base = baseNameOf path;
          in
          !(lib.elem base [
            "target"
            ".git"
            ".direnv"
            "result"
            ".gradle"
            ".kotlin"
            ".idea"
            "node_modules"
          ]);
      };
      cargoLock = {
        lockFile = ./Cargo.lock;
      };

      pkgsFor = system: import nixpkgs { inherit system; };

      desktopFor =
        pkgs:
        pkgs.callPackage ./nix/desktop {
          inherit src cargoLock;
        };

      mixerFor =
        pkgs:
        import ./nix/mixer.nix {
          inherit
            self
            pkgs
            src
            cargoLock
            ;
          rustPlatform = pkgs.rustPlatform;
        };

      webFor =
        pkgs:
        let
          wasmBindgenVersion =
            (lib.findFirst (package: package.name == "wasm-bindgen")
              (throw "wasm-bindgen is missing from Cargo.lock")
              (fromTOML (builtins.readFile ./Cargo.lock)).package
            ).version;
        in
        pkgs.callPackage ./nix/web.nix {
          inherit src cargoLock;
          wasmBindgenCli = pkgs."wasm-bindgen-cli_${lib.replaceStrings [ "." ] [ "_" ] wasmBindgenVersion}";
        };
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = pkgsFor system;
          desktop = desktopFor pkgs;
          web = webFor pkgs;
        in
        {
          inherit desktop web;
        }
        // lib.optionalAttrs pkgs.stdenv.hostPlatform.isLinux {
          mixer = (mixerFor pkgs).package;
          default = (mixerFor pkgs).package;
        }
        // lib.optionalAttrs pkgs.stdenv.hostPlatform.isDarwin {
          default = desktop;
        }
      );

      devShells = forAllSystems (system: {
        default = import ./nix/dev-shell.nix {
          pkgs = pkgsFor system;
        };
      });

      nixosModules = {
        default = import ./nix/desktop/nixos.nix { inherit self; };
        desktop = import ./nix/desktop/nixos.nix { inherit self; };
        mixer = (mixerFor (pkgsFor "x86_64-linux")).module;
      };

      darwinModules = {
        default = import ./nix/desktop/darwin.nix { inherit self; };
        desktop = import ./nix/desktop/darwin.nix { inherit self; };
      };
    };
}
