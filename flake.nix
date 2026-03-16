{
  description = "mdbook-tracey — mdbook preprocessor for tracey requirement annotations";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };

    flake-parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    crane.url = "github:ipetkov/crane";

    treefmt-nix = {
      url = "github:numtide/treefmt-nix";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    git-hooks-nix = {
      url = "github:cachix/git-hooks.nix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-compat.follows = "flake-compat";
    };
  };

  outputs =
    inputs@{
      flake-parts,
      nixpkgs,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];

      imports = [
        inputs.treefmt-nix.flakeModule
        inputs.git-hooks-nix.flakeModule
      ];

      # System-agnostic outputs. The overlay builds with the *consumer's*
      # rustPlatform, not our crane/rust-overlay stack — so the package
      # composes into someone else's nixpkgs without dragging our pins
      # along. The crane path stays for local `nix build` / `nix flake
      # check` where the shared cargoArtifacts cache matters.
      flake.overlays.default = final: _prev: {
        mdbook-tracey = final.rustPlatform.buildRustPackage {
          pname = "mdbook-tracey";
          inherit ((builtins.fromTOML (builtins.readFile ./Cargo.toml)).package) version;

          src = final.lib.fileset.toSource {
            root = ./.;
            fileset = final.lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
            ];
          };

          cargoLock.lockFile = ./Cargo.lock;

          # Tests run via crane in our own flake checks; consumers building
          # through the overlay just want the binary.
          doCheck = false;

          buildInputs = final.lib.optionals final.stdenv.isDarwin [
            final.libiconv
          ];

          meta = with final.lib; {
            description = "mdbook preprocessor for tracey requirement annotations";
            homepage = "https://github.com/lovesegfault/mdbook-tracey";
            license = licenses.bsd3;
            mainProgram = "mdbook-tracey";
          };
        };
      };

      perSystem =
        {
          config,
          pkgs,
          system,
          ...
        }:
        let
          cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);
          inherit (cargoToml.package) version;

          rustStable = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          craneLib = (inputs.crane.mkLib pkgs).overrideToolchain rustStable;

          # Source filtering: keep cargo sources + deny.toml + test fixtures
          # (insta snapshot tests read from tests/fixtures/ at runtime).
          unfilteredRoot = ./.;

          commonArgs = {
            src = pkgs.lib.fileset.toSource {
              root = unfilteredRoot;
              fileset = pkgs.lib.fileset.unions [
                (craneLib.fileset.commonCargoSources unfilteredRoot)
                ./deny.toml
                (pkgs.lib.fileset.maybeMissing ./tests/fixtures)
                (pkgs.lib.fileset.maybeMissing ./tests/snapshots)
              ];
            };
            strictDeps = true;

            pname = "mdbook-tracey";
            inherit version;

            nativeBuildInputs = [ ];
            buildInputs = pkgs.lib.optionals pkgs.stdenv.isDarwin [
              pkgs.libiconv
            ];

            RUST_BACKTRACE = "1";
          };

          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          mdbook-tracey = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
              doCheck = false;
            }
          );

          cargoChecks = {
            clippy = craneLib.cargoClippy (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoClippyExtraArgs = "--all-targets -- --deny warnings";
              }
            );

            deny = craneLib.cargoDeny (commonArgs // { inherit cargoArtifacts; });

            nextest = craneLib.cargoNextest (
              commonArgs
              // {
                inherit cargoArtifacts;
                cargoNextestExtraArgs = "--no-tests=warn";
              }
            );

            doc = craneLib.cargoDoc (
              commonArgs
              // {
                inherit cargoArtifacts;
                RUSTDOCFLAGS = "-Dwarnings";
              }
            );

            # End-to-end: build the test-book with the preprocessor on PATH
            # and assert the expected anchor landed in the rendered HTML.
            test-book =
              pkgs.runCommand "mdbook-tracey-test-book"
                {
                  src = pkgs.lib.fileset.toSource {
                    root = ./test-book;
                    fileset = ./test-book;
                  };
                  nativeBuildInputs = [
                    mdbook-tracey
                    pkgs.mdbook
                  ];
                }
                ''
                  cp -r $src $TMPDIR/book
                  chmod -R +w $TMPDIR/book
                  cd $TMPDIR/book
                  mdbook build -d $out
                  grep -q 'id="r-obs.log.batch-64-100ms"' $out/chapter_1.html
                '';
          };
        in
        {
          _module.args.pkgs = import nixpkgs {
            inherit system;
            overlays = [ inputs.rust-overlay.overlays.default ];
          };

          treefmt.config = {
            flakeCheck = false;
            projectRootFile = "flake.nix";

            programs = {
              nixfmt.enable = true;

              rustfmt = {
                enable = true;
                package = rustStable;
              };

              taplo.enable = true;
            };
          };

          pre-commit = {
            check.enable = true;

            settings.hooks = {
              treefmt.enable = true;
              convco.enable = true;
              ripsecrets.enable = true;
              check-added-large-files.enable = true;
              check-merge-conflicts.enable = true;
              end-of-file-fixer.enable = true;
              trim-trailing-whitespace.enable = true;
              deadnix.enable = true;
              nil.enable = true;
              statix.enable = true;
            };
          };

          devShells.default = craneLib.devShell {
            inherit (config) checks;
            packages = with pkgs; [
              cargo-nextest
              cargo-watch
              cargo-edit
              cargo-insta
              mdbook
              config.treefmt.build.wrapper
            ];
            RUST_BACKTRACE = "1";
            RUST_SRC_PATH = "${rustStable}/lib/rustlib/src/rust/library";
            shellHook = config.pre-commit.installationScript;
          };

          packages.default = mdbook-tracey;

          checks = {
            build = mdbook-tracey;

            # Prove the overlay builds against our own nixpkgs. Catches
            # drift between what crane (via rust-overlay's pinned stable)
            # can compile and what nixpkgs' rustPlatform can — e.g. if we
            # start using a language feature nixpkgs' Rust doesn't have yet.
            overlay-build =
              (import nixpkgs {
                inherit system;
                overlays = [ inputs.self.overlays.default ];
              }).mdbook-tracey;
          }
          // cargoChecks;

          formatter = config.treefmt.build.wrapper;
        };
    };
}
