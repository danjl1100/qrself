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
        overlays = [
          rust-overlay.overlays.default
        ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rustChannel = "beta";
        rustVersion = "latest";
        rustToolchain = pkgs.rust-bin.${rustChannel}.${rustVersion}.default;
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;
        code = pkgs.callPackage ./. {
          inherit system craneLib advisory-db;
        };
      in rec {
        checks = code.checks;
        packages.qrself = code.package;

        packages.default = packages.qrself;

        apps.default = flake-utils.lib.mkApp {
          drv = packages.qrself;
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
