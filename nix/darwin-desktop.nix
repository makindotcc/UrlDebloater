{ self }:

{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.urldebloater-desktop;
in
{
  options.services.urldebloater-desktop = {
    enable = lib.mkEnableOption "the UrlDebloater desktop tray application";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.desktop;
      defaultText = lib.literalExpression "self.packages.\${pkgs.stdenv.hostPlatform.system}.desktop";
      description = "UrlDebloater desktop package to use.";
    };

    startAtLogin = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Start UrlDebloater when a graphical user logs in.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    launchd.agents = lib.mkIf cfg.startAtLogin {
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
