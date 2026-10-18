{ self }:

{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.urldebloater-desktop;

  autostartItem = pkgs.makeDesktopItem {
    name = "urldebloater-autostart";
    desktopName = "UrlDebloater";
    comment = "Remove tracking parameters from links copied to the clipboard";
    exec = "${cfg.package}/bin/urldebloater-desktop-daemon";
    icon = "urldebloater";
    terminal = false;
    noDisplay = true;
  };
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

    autostart = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = "Start UrlDebloater when the graphical session starts.";
    };
  };

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    environment.etc = lib.mkIf cfg.autostart {
      "xdg/autostart/urldebloater.desktop".source =
        "${autostartItem}/share/applications/urldebloater-autostart.desktop";
    };
  };
}
