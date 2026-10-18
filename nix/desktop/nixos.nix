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
  imports = [ (import ./options.nix { inherit self; }) ];

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    environment.etc = lib.mkIf cfg.autostart {
      "xdg/autostart/urldebloater.desktop".source =
        "${autostartItem}/share/applications/urldebloater-autostart.desktop";
    };
  };
}
