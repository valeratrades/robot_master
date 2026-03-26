{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-parts.url = "github:hercules-ci/flake-parts";
    devenv.url = "github:cachix/devenv/v1.6.1";
    pre-commit-hooks.url = "github:cachix/git-hooks.nix";
    v_flakes.url = "github:valeratrades/v_flakes?ref=v1.4";
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
            build = {
              enable = true;
              workspace = {
                "./robot_master" = [ "git_version" "log_directives" ];
              };
            };
          };
          github = v_flakes.github {
            inherit pkgs pname rs;
            enable = true;
            lastSupportedVersion = "nightly-2026-02-01";
            langs = [ "rs" "py" ];
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
          combined = v_flakes.utils.combine [ rs github readme ];
        in
        {
          _module.args.pkgs = pkgs;

          packages =
            let
              rustc = rust;
              cargo = rust;
              rustPlatform = pkgs.makeRustPlatform {
                inherit rustc cargo stdenv;
              };
            in
            {
              default = rustPlatform.buildRustPackage {
                inherit pname;
                version = "0.1.0";

                buildInputs = with pkgs; [
                  openssl.dev
                ];
                nativeBuildInputs = with pkgs; [ pkg-config ];

                cargoLock.lockFile = ./Cargo.lock;
                src = pkgs.lib.cleanSource ./.;
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
              uv_sync.exec = "uv sync --prerelease=allow --no-install-project --dev";
            };

            packages = with pkgs; [
              mold
              openssl
              pkg-config
              rust
              simple-http-server
              cargo-leptos
              # bevy dependencies
              alsa-lib
              udev
              vulkan-loader
              libxkbcommon
              wayland
              xorg.libX11
              xorg.libXcursor
              xorg.libXi
              xorg.libXrandr
            ] ++ pre-commit-check.enabledPackages ++ combined.enabledPackages;

            env = {
              RUST_BACKTRACE = 1;
              RUST_LIB_BACKTRACE = 0;
            };

            enterShell =
              pre-commit-check.shellHook
              + combined.shellHook
              + ''
                cp -f ${(v_flakes.files.treefmt) { inherit pkgs; }} ./.treefmt.toml

                export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [
                  pkgs.vulkan-loader
                  pkgs.libxkbcommon
                  pkgs.wayland
                  pkgs.udev
                  pkgs.alsa-lib
                  pkgs.xorg.libX11
                  pkgs.xorg.libXcursor
                  pkgs.xorg.libXi
                  pkgs.xorg.libXrandr
                ]}''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

                if [ ! -d ".devenv/state/venv" ]; then
                  uv venv .devenv/state/venv
                fi
                source .devenv/state/venv/bin/activate
              '';
          };
        };
    };
}
