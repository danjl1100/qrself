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
        ];
        rustToolchain = pkgs.rust-bin.${rustChannel}.${rustVersion}.default;
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        code = pkgs.callPackage ./. {
          inherit system crane advisory-db rustToolchain;
        };
      in rec {
        checks = code.checks;
        packages.qrself = code.package;
        packages.docker = pkgs.dockerTools.buildImage {
          name = "rust-qrself";
          config = {
            Cmd = let
              init_prefix = if (system == "x86_64-linux") then ["${pkgs.tini}/bin/tini"] else [];
            in init_prefix ++ ["${packages.qrself}/bin/qrself" ];
          };
        };
        packages.dockerScript = pkgs.writeShellScript "docker-test.sh" ''
          set -e
          docker load < $(nix build .#docker --no-link --print-out-paths)
          echo "Now run:  docker run --rm -p 3000:3000 -e BIND_ADDRESS=0.0.0.0:3000 [the hash]"
          echo "Or:   docker image tag [the hash] [tag]"
          echo "Then: docker image push [tag]    (run docker image ls, to recall the tag)"
        '';

        packages.default = packages.qrself;

        apps.default = flake-utils.lib.mkApp {
          drv = packages.qrself;
        };

        devShells.default = pkgs.mkShell {
          inputsFrom = builtins.attrValues self.checks;

          # Extra inputs can be added here
          nativeBuildInputs = [
            pkgs.bacon
            rustToolchain
          ];
        };
      }
    );
}
