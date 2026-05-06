{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/549bd84d6279f9852cae6225e372cc67fb91a4c1";
    rust-overlay.url = "github:oxalica/rust-overlay/adf987c76af8d17b8256d23631bcf203f81e1a63";
    flake-parts.url = "github:hercules-ci/flake-parts/0678d8986be1661af6bb555f3489f2fdfc31f6ff";
    devenv.url = "github:cachix/devenv/f19b62ea677ec6046d78243e176fa01d5ef0d55a";
    pre-commit-hooks.url = "github:cachix/git-hooks.nix/3cfd774b0a530725a077e17354fbdb87ea1c4aad";
    v_flakes.url = "github:valeratrades/v_flakes/6062f652effc94be053865d58ff03c697c31ecb6";
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
            libX11
            libXcursor
            libXi
            libXrandr
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

            in
            {
              inherit site;
              default = site;
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
              uv_sync.exec = "uv sync --prerelease=allow --inexact --dev --group train";
            };

            packages = [
              pkgs.mold
              pkgs.openssl
              pkgs.pkg-config
              rust
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

              '';
          };
        };
    };
}
