{
  self,
  pkgs,
  src,
  cargoLock,
  rustPlatform,
}:

let
  serviceName = "urldebloater-mixer";

  package = rustPlatform.buildRustPackage {
    pname = "urldebloater-mixer";
    version = (pkgs.lib.importTOML ../../Cargo.toml).workspace.package.version;
    inherit src cargoLock;

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

  module =
    {
      config,
      lib,
      pkgs,
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
            ExecStart = "${self.packages.${pkgs.stdenv.hostPlatform.system}.mixer}/bin/urldebloater-mixer";
            Restart = "on-failure";
            RestartSec = "5s";
            User = serviceName;
            Group = serviceName;
          };
          environment = {
            LISTEN_ADDRESS = toString config.services.${serviceName}.listenAddress;
          };
        };

        users.users.${serviceName} = {
          isSystemUser = true;
          group = serviceName;
        };
        users.groups.${serviceName} = { };
      };
    };
in
{
  inherit package module;
}
