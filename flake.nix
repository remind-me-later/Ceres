# The tools Ceres is built, tested and proved with, for `nix develop` (or
# direnv's `use flake`, see .envrc). flake.lock pins each of them: updating
# one is a commit.
#
#   nix develop                 enter the shell
#   nix flake update nixpkgs    move the nixpkgs tools forward
#
# What is not here:
#
# - Rust itself. rustup is, and installs the toolchains: stable for ceres
#   (`rustup default stable`, and `rustup component add llvm-tools-preview`
#   for coverage), and the nightlies Kani asks for. nixpkgs's rustup patches
#   the toolchains it downloads to run on NixOS.
# - Kani, which `cargo install` builds and `cargo kani setup` completes by
#   downloading CBMC; on NixOS those binaries need programs.nix-ld. The shell
#   checks that the installed version is kaniVersion below.
# - Lean, which elan installs at the version lean/lean-toolchain names.
{
  description = "Ceres: the tools to build, test and prove it";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # Charon and Aeneas at the nightly lean/translate.sh and lean/lakefile.toml
    # name. Aeneas's flake pins the Charon it is built with (8cae8fd1, the
    # commit translate.sh names), so both come from it. Its nixpkgs is left
    # its own: the pair is built as Aeneas tests it.
    aeneas.url = "github:AeneasVerif/aeneas/nightly-2026.11.07-c5eb4a2";
  };

  outputs =
    { nixpkgs, aeneas, ... }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};

      # The version hdl/kani.sh's verdicts are run with. Change it with the
      # verdicts: `cargo install --locked kani-verifier --version <it>` and
      # `cargo kani setup`, then `hdl/kani.sh rerun` what it should.
      kaniVersion = "0.68.0";

      # rumdl and tombi at the versions .github/workflows/lint.yml pins, from
      # their releases (nixpkgs has others, which format differently).
      release =
        {
          pname,
          version,
          url,
          hash,
          bin,
        }:
        pkgs.stdenvNoCC.mkDerivation {
          inherit pname version;
          src = pkgs.fetchurl { inherit url hash; };
          sourceRoot = ".";
          installPhase = ''
            install -Dm755 ${bin} $out/bin/${pname}
          '';
        };
      rumdl = release rec {
        pname = "rumdl";
        version = "0.2.78";
        url = "https://github.com/rvben/rumdl/releases/download/v${version}/rumdl-v${version}-x86_64-unknown-linux-musl.tar.gz";
        hash = "sha256-UH0iQ8o1gglFiMBhDC7lw7zQpQqFZBSdzOQmjN9Q3oI=";
        bin = "rumdl";
      };
      tombi = release rec {
        pname = "tombi";
        version = "1.8.5";
        url = "https://github.com/tombi-toml/tombi/releases/download/v${version}/tombi-cli-${version}-x86_64-unknown-linux-musl.tar.gz";
        hash = "sha256-NmP26Q+yxAJllIZ8ZR4EDGAAxIQGdHIVrW5xxgZokcE=";
        bin = "tombi-cli-${version}-x86_64-unknown-linux-musl/tombi";
      };

      # What winit, wgpu and glow open at run time (ceres-winit, ceres-egui).
      runtimeLibs = with pkgs; [
        libGL
        libxkbcommon
        vulkan-loader
        wayland
        libx11
        libxcursor
        libxi
        libxrandr
      ];
    in
    {
      devShells.${system}.default = pkgs.mkShell {
        packages = with pkgs; [
          # Rust (see above), and what CI runs on it.
          rustup
          cargo-nextest
          cargo-llvm-cov
          cargo-deny

          # The frontends' libraries.
          pkg-config
          gtk4
          libadwaita
          alsa-lib

          # The boot ROMs and the test ROMs (ceres-test-runner's build.rs
          # downloads the collection with curl and unzips it).
          rgbds
          unzip

          # hdl/: dmg-sim on Icarus (13, which smoke.sha256 was taken
          # with), the readable model on nvc, the knowledge base on Soufflé,
          # and what the scripts call. The C compiler (runts.c, the Gambatte
          # oracle) and make come with mkShell.
          iverilog
          nvc
          souffle
          gawk
          bc
          parallel
          imagemagick
          zlib
          vhdl-ls

          # lean/: elan installs Lean; Lake fetches with git and curl.
          elan
          git
          curl
          aeneas.packages.${system}.charon
          aeneas.packages.${system}.aeneas

          # The Markdown and TOML linters.
          rumdl
          tombi
        ];

        LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;

        shellHook = ''
          # cargo install's binaries (Kani), after the shell's own tools.
          export PATH="$PATH:''${CARGO_HOME:-$HOME/.cargo}/bin"

          if ! rustup show active-toolchain >/dev/null 2>&1; then
            echo "ceres: no Rust toolchain: rustup default stable" >&2
          fi
          kani=$(cargo kani --version 2>/dev/null | awk '/^Kani/ { print $4 }')
          if [ -z "$kani" ]; then
            echo "ceres: no Kani: cargo install --locked kani-verifier --version ${kaniVersion} && cargo kani setup" >&2
          elif [ "$kani" != "${kaniVersion}" ]; then
            echo "ceres: Kani is $kani, flake.nix pins ${kaniVersion}" >&2
          fi
          unset kani
        '';
      };
    };
}
