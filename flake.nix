{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url = "github:hercules-ci/flake-parts";
    devenv.url = "github:cachix/devenv/v1.6.1";
    pre-commit-hooks.url = "github:cachix/git-hooks.nix";
    v_flakes.url = "github:valeratrades/v_flakes?ref=v1.5";
  };

  outputs = inputs@{ self, nixpkgs, rust-overlay, flake-parts, devenv, pre-commit-hooks, v_flakes }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      imports = [
        devenv.flakeModule
      ];

      systems = nixpkgs.lib.systems.flakeExposed;

      perSystem = { config, self', inputs', system, ... }:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ (import rust-overlay) ];
            config.allowUnfree = true;
          };
          #NB: can't load rust-bin from nightly.latest, as there are week guarantees of which components will be available on each day.
          rust = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
            extensions = [ "rust-src" "rust-analyzer" "rust-docs" "rustc-codegen-cranelift-preview" ];
            targets = [ "wasm32-unknown-unknown" ];
          });
          pre-commit-check = pre-commit-hooks.lib.${system}.run (v_flakes.files.preCommit { inherit pkgs; });
          manifest = (pkgs.lib.importTOML ./robot_master_site/Cargo.toml).package;
          pname = manifest.name;
          stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv;
          python = pkgs.python312;

          rs = v_flakes.rs {
            inherit pkgs rust;
            targets."wasm32-unknown-unknown".rustflags = [ ''--cfg=getrandom_backend="wasm_js"'' ];
            build = {
              enable = true;
              workspace = {
                "./robot_master_site" = [ "git_version" "log_directives" ];
              };
            };
          };
          py = v_flakes.py {
            inherit pkgs;
            ruff.exclude.augment = [
              "py_src/partie_guidee/a_test.py"
              "py_src/partie_guidee/b_test.py"
              "py_src/partie_guidee/c_test.py"
              "py_src/partie_guidee/d_test.py"
              "py_src/partie_guidee/e_test.py"
            ];
          };
          github = v_flakes.github {
            inherit pkgs pname rs py;
            enable = true;
            lastSupportedVersion = "nightly-2026-02-01";
            jobs.default = true;
            gitlabSync.mirrorBaseUrl = "https://gitlab.isima.fr/vasakharov";
          };
          readme = v_flakes.readme-fw {
            inherit pkgs pname;
            defaults = true;
            lastSupportedVersion = "nightly-1.92";
            rootDir = ./.;
            badges = [ "msrv" "crates_io" "docs_rs" "loc" "ci" ];
          };
          combined = v_flakes.utils.combine [ rs py github readme ];

          nativeLibs = with pkgs; [
            alsa-lib
            udev
            vulkan-loader
            libxkbcommon
            wayland
            xorg.libX11
            xorg.libXcursor
            xorg.libXi
            xorg.libXrandr
          ];

          rustPlatform = pkgs.makeRustPlatform {
            rustc = rust;
            cargo = rust;
            inherit stdenv;
          };
        in
        {
          _module.args.pkgs = pkgs;

          packages =
            let
              site = rustPlatform.buildRustPackage {
                inherit pname;
                version = "0.1.0";

                buildInputs = [ pkgs.openssl.dev ] ++ nativeLibs;
                nativeBuildInputs = with pkgs; [ pkg-config ];

                cargoLock.lockFile = ./Cargo.lock;
                src = pkgs.lib.cleanSource ./.;
              };

              core = python.pkgs.buildPythonPackage {
                pname = "robot_master";
                version = "0.1.0";
                format = "pyproject";

                src = pkgs.lib.cleanSource ./.;

                cargoDeps = rustPlatform.importCargoLock {
                  lockFile = ./Cargo.lock;
                };

                dependencies = [
                  python.pkgs.typeguard
                  python.pkgs.icecream
                ];

                nativeBuildInputs = [
                  rustPlatform.cargoSetupHook
                  rustPlatform.maturinBuildHook
                  rust
                  pkgs.maturin
                  pkgs.mold
                ];

                maturinBuildFlags = [ "-m" "robot_master/Cargo.toml" "--features" "python" ];

                # .cargo/config.toml has nightly-only -Z flags; use our nightly toolchain.
                RUSTC = "${rust}/bin/rustc";
                CARGO = "${rust}/bin/cargo";
              };
            in
            {
              inherit core site;
              default = pkgs.symlinkJoin {
                name = "robot-master";
                paths = [ site core ];
              };
            };

          devenv.shells.default = {
            languages.python = {
              enable = true;
              package = python;
              uv = {
                enable = true;
                sync.enable = false;
              };
            };

            scripts = {
              run.exec = ''python -m py_src "$@"'';
              uv_sync.exec = "uv sync --prerelease=allow --no-install-project --dev";
              test_a.exec = ''pytest py_src/partie_guidee/a_test.py "$@"'';
              test_b.exec = ''pytest py_src/partie_guidee/b_test.py "$@"'';
              test_c.exec = ''pytest py_src/partie_guidee/c_test.py "$@"'';
              test_d.exec = ''pytest py_src/partie_guidee/d_test.py "$@"'';
              test_e.exec = ''pytest py_src/partie_guidee/e_test.py "$@"'';
            };

            packages = [
              pkgs.mold
              pkgs.openssl
              pkgs.pkg-config
              rust
              pkgs.maturin
              pkgs.simple-http-server
              pkgs.cargo-leptos
            ] ++ nativeLibs ++ pre-commit-check.enabledPackages ++ combined.enabledPackages;

            env = {
              RUST_BACKTRACE = 1;
              RUST_LIB_BACKTRACE = 0;
            };

            enterShell =
              pre-commit-check.shellHook
              + combined.shellHook
              + ''
                cp -f ${(v_flakes.files.treefmt) { inherit pkgs; }} ./.treefmt.toml

                export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath nativeLibs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

                if [ ! -d ".devenv/state/venv" ]; then
                  uv venv .devenv/state/venv
                fi
                source .devenv/state/venv/bin/activate
              '';
          };
        };
    };
}
