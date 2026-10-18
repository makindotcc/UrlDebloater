{ self }:

{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.services.urldebloater-desktop;

  systemctl = "${config.systemd.package}/bin/systemctl";
  loginctl = "${config.systemd.package}/bin/loginctl";
  runuser = "${pkgs.util-linux}/bin/runuser";
  env = "${pkgs.coreutils}/bin/env";
in
{
  imports = [ (import ./options.nix { inherit self; }) ];

  config = lib.mkIf cfg.enable {
    environment.systemPackages = [ cfg.package ];

    systemd.user.services.urldebloater-desktop = {
      description = "UrlDebloater desktop tray application";
      wantedBy = lib.optionals cfg.autostart [ "graphical-session.target" ];
      partOf = [ "graphical-session.target" ];
      after = [ "graphical-session.target" ];
      serviceConfig = {
        Type = "exec";
        ExecStart = "${cfg.package}/bin/urldebloater-desktop-daemon";
        Restart = "on-failure";
        RestartSec = 5;
      };
    };

    # A rebuild does not re-run graphical-session.target, so start the new
    # service for users who are already logged in.
    system.activationScripts.urldebloater-desktop = lib.mkIf cfg.autostart {
      deps = [ "etc" ];
      text = ''
        if [ -d /run/systemd/system ]; then
          ${loginctl} list-sessions --no-legend | while read -r session uid user _; do
            [ -n "$session" ] || continue
            [ "$(${loginctl} show-session "$session" -p Active --value)" = "yes" ] || continue
            case "$(${loginctl} show-session "$session" -p Type --value)" in
              wayland|x11) ;;
              *) continue ;;
            esac

            runtime_dir="/run/user/$uid"
            [ -S "$runtime_dir/bus" ] || continue

            ${runuser} -u "$user" -- ${env} \
              XDG_RUNTIME_DIR="$runtime_dir" \
              DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_dir/bus" \
              ${systemctl} --user daemon-reload || true
            ${runuser} -u "$user" -- ${env} \
              XDG_RUNTIME_DIR="$runtime_dir" \
              DBUS_SESSION_BUS_ADDRESS="unix:path=$runtime_dir/bus" \
              ${systemctl} --user --no-block restart urldebloater-desktop.service || true
          done
        fi
      '';
    };
  };
}
