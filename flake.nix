{
  description = "Flake for urldebloater-mixer service. In future this should also include urldebloater desktop client...";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";

  outputs =
    {
      self,
      nixpkgs,
    }:
    let
      system = "x86_64-linux";
      pkgs = import nixpkgs { inherit system; };
      serviceName = "urldebloater-mixer";
    in
    {
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        name = "urldebloater-mixer";
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
        cargoBuildFlags = [
          "-p"
          "urldebloater-mixer"
        ];
        cargoTestFlags = [
          "-p"
          "urldebloater-mixer"
        ];

        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = [ pkgs.openssl ];
      };

      nixosModules.default =
        {
          config,
          lib,
          pkgs,
          inputs,
          ...
        }:
        {
          options.services.${serviceName} = {
            enable = lib.mkEnableOption "urldebloater mixer service";
            listenAddress = lib.mkOption {
              type = lib.types.str;
              default = "127.0.0.1:7777";
            };
          };

          config = lib.mkIf config.services.${serviceName}.enable {
            systemd.services.${serviceName} = {
              description = "urldebloater mixer service";
              wantedBy = [ "multi-user.target" ];
              after = [ "network.target" ];
              startLimitIntervalSec = 120;
              startLimitBurst = 5;
              serviceConfig = {
                ExecStart = "${self.packages.${pkgs.system}.default}/bin/urldebloater-mixer";
                Restart = "on-failure";
                RestartSec = "5s";
                User = serviceName;
                Group = serviceName;
              };
              environment = {
                LISTEN_ADDRESS = builtins.toString config.services.${serviceName}.listenAddress;
              };
            };

            users.users.${serviceName} = {
              isSystemUser = true;
              group = serviceName;
            };
            users.groups.${serviceName} = { };
          };
        };
    };
}
