{
  description = "Suboxide API-compatible music streaming server in Rust";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {self, ...} @ inputs:
    inputs.flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import inputs.nixpkgs {
        inherit system;
        overlays = [
          inputs.self.overlays.default
        ];
      };
      naersk' = pkgs.callPackage inputs.naersk {
        cargo = pkgs.rustToolchain;
        rustc = pkgs.rustToolchain;
      };
    in {
      packages.default = naersk'.buildPackage {
        src = ./.;
        buildInputs = with pkgs; [
          openssl
          pkg-config
        ];
      };

      devShells.default = pkgs.mkShell {
        packages = with pkgs; [
          rustToolchain
          openssl
          pkg-config
          cargo-deny
          cargo-edit
          cargo-watch
          rust-analyzer
        ];

        env = {
          # Required by rust-analyzer
          RUST_SRC_PATH = "${pkgs.rustToolchain}/lib/rustlib/src/rust/library";
        };
      };

      checks.default = naersk'.buildPackage {
        src = ./.;
        buildInputs = with pkgs; [
          openssl
          pkg-config
        ];
      };

      checks.clippy = pkgs.stdenv.mkDerivation {
        name = "clippy";
        src = ./.;
        buildInputs = with pkgs; [ rustToolchain ];
        buildPhase = ''
          cargo clippy --all-targets -- -D warnings
        '';
        installPhase = "mkdir -p $out; touch $out/done";
      };

      checks.fmt = pkgs.stdenv.mkDerivation {
        name = "fmt";
        src = ./.;
        buildInputs = with pkgs; [ rustToolchain ];
        buildPhase = ''
          cargo fmt -- --check
        '';
        installPhase = "mkdir -p $out; touch $out/done";
      };
    }) // {
      overlays.default = final: prev: {
        rustToolchain = with inputs.fenix.packages.${prev.stdenv.hostPlatform.system};
          combine (
            with stable; [
              clippy
              rustc
              cargo
              rustfmt
              rust-src
            ]
          );
      };
    };
}

