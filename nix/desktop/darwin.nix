{ self }:

{
  config,
  lib,
  ...
}:

let
  cfg = config.services.urldebloater-desktop;
in
{
  imports = [ (import ./options.nix { inherit self; }) ];

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    launchd.agents = lib.mkIf cfg.autostart {
      urldebloater = {
        command = "${cfg.package}/bin/urldebloater-desktop-daemon";
        serviceConfig = {
          Label = "com.urldebloater.desktop";
          RunAtLoad = true;
          KeepAlive = {
            SuccessfulExit = false;
          };
          LimitLoadToSessionType = "Aqua";
        };
      };
    };
  };
}
