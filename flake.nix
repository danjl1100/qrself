{
  description = "a simple webserver for self-referential QR codes";

  inputs = {
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.follows = "rust-overlay/flake-utils";
    nixpkgs.follows = "rust-overlay/nixpkgs";
    crane.url = "github:ipetkov/crane";
    # nix-filter.url = "github:numtide/nix-filter";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, rust-overlay, flake-utils, nixpkgs, crane, advisory-db }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        code = pkgs.callPackage ./. {
          inherit crane advisory-db;
          # nix-filter = nix-filter.lib;
        };
      in rec {
        packages = {
          qrself = code.defaultPackage;
          default = packages.qrself;
        };
        inherit (code) checks;
      }
    );
}
