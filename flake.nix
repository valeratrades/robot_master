{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url = "github:hercules-ci/flake-parts";
    devenv.url = "github:cachix/devenv/v1.6.1";
    pre-commit-hooks.url = "github:cachix/git-hooks.nix";
    v_flakes.url = "github:valeratrades/v_flakes?ref=v1.6";
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
          manifest = (pkgs.lib.importTOML ./robot_master/Cargo.toml).package;
          pname = manifest.name;
          stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv;
          python = pkgs.python312;

          rs = v_flakes.rs {
            inherit pkgs rust;
            targets."wasm32-unknown-unknown".rustflags = [ ''--cfg=getrandom_backend="wasm_js"'' ];
            config = {
              alias = {
                t = "nextest run --workspace";
                ta = "nextest run --workspace --no-fail-fast";
              };
            };
            build = {
              enable = true;
              workspace = {
                "./robot_master_site" = [ "git_version" "log_directives" ]; #Q: do I need it?
                "./robot_master" = [ "git_version" "log_directives" ];
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
            gitignore.extra = "docs/references/**/*.pdf\ndocs/references/**/*.tar.gz";
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
                nativeBuildInputs = with pkgs; [ pkg-config makeWrapper ];

                cargoLock.lockFile = ./Cargo.lock;
                src = pkgs.lib.cleanSource ./.;

                postInstall = ''
                  wrapProgram $out/bin/${pname} \
                    --prefix PATH : ${pkgs.lib.makeBinPath [ pkgs.fzf ]}
                '';
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
              maturin_build.exec = "maturin develop --features python -m robot_master/Cargo.toml";
              uv_sync.exec = "maturin_build && uv sync --prerelease=allow --no-install-project --inexact --dev --group train";
              pytest.exec = "maturin_build && pytest \"$@\"";
            };

            packages = [
              pkgs.mold
              pkgs.openssl
              pkgs.pkg-config
              rust
              pkgs.maturin
              pkgs.simple-http-server
              pkgs.cargo-leptos
              pkgs.fzf
              pkgs.nerd-fonts.symbols-only
              pkgs.noto-fonts
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
                cp -f ${(v_flakes.files.gitattributes) { inherit pkgs; lfs = false; }} ./.gitattributes

                export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath nativeLibs}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

                mkdir -p robot_master_game/assets/fonts
                cp -f ${pkgs.nerd-fonts.symbols-only}/share/fonts/truetype/NerdFonts/Symbols/SymbolsNerdFontMono-Regular.ttf \
                  robot_master_game/assets/fonts/SymbolsNerdFontMono-Regular.ttf
                cp -f ${pkgs.noto-fonts}/share/fonts/noto/NotoSansSymbols2-Regular.otf \
                  robot_master_game/assets/fonts/NotoSansSymbols2-Regular.otf

                if ! python -c "import robot_master" 2>/dev/null; then
                  echo "⚠ robot_master not built — run: maturin build"
                fi
              '';
          };
        };
    };
}
