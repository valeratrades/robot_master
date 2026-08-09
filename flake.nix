{
  inputs = {
    v_flakes.url = "github:valeratrades/v_flakes?ref=v1.6";
  };

  outputs = { self, v_flakes }:
    let
      inherit (v_flakes) flake-parts devenv nixpkgs pre-commit-hooks;
    in
    flake-parts.lib.mkFlake
      # devenv's flakeModule evaluates `inputs.nixpkgs.lib` — it wants the flake, not `default_nixpkgs`.
      { inputs = { inherit self nixpkgs devenv; }; }
      {
        imports = [
          devenv.flakeModule
        ];

        systems = nixpkgs.lib.systems.flakeExposed;

        perSystem = { system, ... }:
          let
            pkgs = import v_flakes.default_nixpkgs {
              inherit system;
              config.allowUnfree = true;
            };
            rust = v_flakes.rs.default_nightly system;
            pre-commit-check = pre-commit-hooks.lib.${system}.run (v_flakes.files.preCommit { inherit pkgs; });
            manifest = (pkgs.lib.importTOML ./robot_master/Cargo.toml).package;
            pname = manifest.name;
            stdenv = pkgs.stdenvAdapters.useMoldLinker pkgs.stdenv;

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
              lastSupportedVersion = "nightly-${v_flakes.rs.nightly_version}";
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
            combined = v_flakes.utils.combine { inherit rust; modules = [ rs py github readme ]; };

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

                  RUSTC_WRAPPER = ""; # the generated .cargo/config.toml asks for sccache, absent from the build sandbox

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
                package = py.python;
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
