{
  description = "Cabalist - TUI tool for managing Haskell .cabal files";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.stable.latest.default;
      in {
        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "cabalist";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          # Build both binaries
          cargoBuildFlags = [ "--workspace" ];

          meta = with pkgs.lib; {
            description = "TUI tool for managing Haskell .cabal files";
            homepage = "https://github.com/joshburgess/cabalist";
            license = with licenses; [ mit asl20 ];
            maintainers = [ ];
          };
        };

        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            rustToolchain
            rust-analyzer
            cargo-watch
          ];
        };
      }
    );
}
