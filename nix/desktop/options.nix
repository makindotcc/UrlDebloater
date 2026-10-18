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

    configGui = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Build the settings window and enable its tray integration. When false, the
        `config-gui-integration` feature is disabled and the settings window is not
        installed.
      '';
    };

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.desktop.override {
        configGui = cfg.configGui;
      };
      defaultText = lib.literalExpression ''
        (self.packages.\${pkgs.stdenv.hostPlatform.system}.desktop).override {
          configGui = config.services.urldebloater-desktop.configGui;
        }
      '';
      description = "UrlDebloater desktop package to use.";
    };

    autostart = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Start UrlDebloater when the graphical session starts. On NixOS the daemon is also
        restarted for logged-in graphical sessions after a system activation.
      '';
    };
  };
}
