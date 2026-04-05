{
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.services.suboxide;

  startCommand =
    [
      (lib.getExe cfg.package)
      "--database"
      (toString cfg.databasePath)
      "--port"
      (toString cfg.port)
      "serve"
    ]
    ++ lib.optionals cfg.autoScan [
      "--auto-scan"
      "--auto-scan-interval"
      (toString cfg.autoScanInterval)
    ]
    ++ cfg.extraArgs;
in {
  options.services.suboxide = {
    enable = lib.mkEnableOption "the Suboxide music streaming server";

    package = lib.mkOption {
      type = lib.types.package;
      default = pkgs.callPackage ./package.nix {};
      defaultText = lib.literalExpression "pkgs.callPackage ./package.nix { }";
      description = "Suboxide package to run for the service.";
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "suboxide";
      description = "User account under which the Suboxide service runs.";
    };

    group = lib.mkOption {
      type = lib.types.str;
      default = "suboxide";
      description = "Group account under which the Suboxide service runs.";
    };

    dataDir = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/suboxide";
      description = "Directory used by Suboxide for persistent runtime data.";
    };

    databasePath = lib.mkOption {
      type = lib.types.path;
      default = "/var/lib/suboxide/suboxide.db";
      example = "/var/lib/suboxide/suboxide.db";
      description = "SQLite database path passed via --database.";
    };

    port = lib.mkOption {
      type = lib.types.port;
      default = 4040;
      description = "TCP port that Suboxide listens on.";
    };

    autoScan = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Enable periodic incremental music library scans.";
    };

    autoScanInterval = lib.mkOption {
      type = lib.types.ints.positive;
      default = 300;
      description = "Auto-scan interval in seconds when autoScan is enabled.";
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = "Open the configured Suboxide port in the firewall.";
    };

    environment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = {};
      example = {
        RUST_LOG = "suboxide=info";
        LASTFM_API_KEY = "<api-key>";
      };
      description = "Environment variables to set for the Suboxide process.";
    };

    environmentFile = lib.mkOption {
      type = lib.types.nullOr lib.types.path;
      default = null;
      example = "/run/secrets/suboxide.env";
      description = "Optional environment file loaded by systemd (for secrets).";
    };

    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [];
      example = ["--help"];
      description = "Additional arguments appended to the suboxide command line.";
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = cfg.autoScanInterval > 0;
        message = "services.suboxide.autoScanInterval must be greater than 0.";
      }
    ];

    users.groups = lib.mkIf (cfg.group == "suboxide") {
      suboxide = {};
    };

    users.users = lib.mkIf (cfg.user == "suboxide") {
      suboxide = {
        isSystemUser = true;
        group = cfg.group;
        home = cfg.dataDir;
        createHome = false;
      };
    };

    systemd.tmpfiles.rules = [
      "d ${toString cfg.dataDir} 0750 ${cfg.user} ${cfg.group} -"
    ];

    networking.firewall.allowedTCPPorts = lib.mkIf cfg.openFirewall [cfg.port];

    systemd.services.suboxide = {
      description = "Suboxide music streaming server";
      after = ["network-online.target"];
      wants = ["network-online.target"];
      wantedBy = ["multi-user.target"];

      environment = cfg.environment;

      serviceConfig =
        {
          Type = "simple";
          User = cfg.user;
          Group = cfg.group;
          WorkingDirectory = cfg.dataDir;
          ExecStart = lib.escapeShellArgs startCommand;
          Restart = "on-failure";
          RestartSec = "5s";

          NoNewPrivileges = true;
          PrivateTmp = true;
          PrivateDevices = true;
          ProtectSystem = "strict";
          ProtectHome = true;
          ProtectControlGroups = true;
          ProtectKernelTunables = true;
          ProtectKernelModules = true;
          ProtectClock = true;
          LockPersonality = true;
          RestrictNamespaces = true;
          RestrictRealtime = true;
          RestrictSUIDSGID = true;
          MemoryDenyWriteExecute = true;
          SystemCallArchitectures = "native";
          UMask = "0077";

          ReadWritePaths = [
            cfg.dataDir
            (builtins.dirOf (toString cfg.databasePath))
          ];
        }
        // lib.optionalAttrs (cfg.environmentFile != null) {
          EnvironmentFile = cfg.environmentFile;
        };
    };
  };
}
