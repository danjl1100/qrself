{ pkgs, crane, advisory-db }:
let
  craneLib = crane.mkLib pkgs;
  src = craneLib.cleanCargoSource ./.;

  # Build *just* the cargo dependencies, so we can reuse all of that work
  cargoArtifacts = craneLib.buildDepsOnly {
    inherit src;
  };

  # Run clippy (and deny all warnings) on the crate source,
  # reusing the dependency artifacts from above.
  #
  # Note that this is done as a separate derivation so it
  # does not impact building just the crate by itself.
  qrself-clippy = craneLib.cargoClippy {
    inherit cargoArtifacts src;
    cargoClippyExtraArgs = "-- --deny warnings";
  };

  # Build the actual crate itself, reusing the dependency
  # artifacts from above
  qrself = craneLib.buildPackage {
    inherit cargoArtifacts src;
  };

  # qrself-coverage = craneLib.cargoTarpaulin {
  #   inherit cargoArtifacts src;
  # };
in {
  defaultPackage = qrself;
  checks = {
    inherit
      # Build the crate as part of `nix flake check` for convenience
      qrself
      qrself-clippy
      # qrself-coverage
      ;
  };
}
