{
  description = "a simple webserver for self-referential QR codes";

  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.follows = "rust-overlay/flake-utils";
    nixpkgs.follows = "rust-overlay/nixpkgs";
    crane.url = "github:ipetkov/crane";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, rust-overlay, flake-utils, nixpkgs, crane, advisory-db }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        rustChannel = "beta";
        rustVersion = "latest";
        overlays = [
          rust-overlay.overlays.default
          (self: super: let
              rust-bundle = self.rust-bin.${rustChannel}.${rustVersion}.default;
            in {
              # unpack rust-overlay's bundles to inform crane
              rustc = rust-bundle;
              cargo = rust-bundle;
              clippy = rust-bundle;
            })
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        code = pkgs.callPackage ./. {
          inherit system crane advisory-db;
        };
      in rec {
        inherit (code) checks;
        packages.qrself = code.package;
        packages.default = packages.qrself;
        apps.default = flake-utils.lib.mkApp {
          drv = packages.default;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.checks;

          # Extra inputs can be added here
          nativeBuildInputs = [
            pkgs.cargo
            pkgs.rustc
          ];
        };
      }
    );
}
