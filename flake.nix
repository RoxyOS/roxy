{
  description = "Roxy OS development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay.url = "github:oxalica/rust-overlay";
    jinx = {
      url = "github:Mintsuki/Jinx/287ceaf9a2c08b43dbc56d38d8b815fd990a2192";
      flake = false;
    };
  };

  outputs =
    inputs@{
      flake-parts,
      jinx,
      nixpkgs,
      rust-overlay,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [ "x86_64-linux" ];

      perSystem =
        { system, ... }:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };

          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          patchedJinx = pkgs.runCommand "jinx-roxy" { } ''
            mkdir -p "$out"
            cp ${jinx}/jinx "$out/jinx"
            chmod u+w "$out/jinx"
            substituteInPlace "$out/jinx" \
              --replace-fail \
                'https://snapshot.debian.org/archive/debian/''${JINX_DEBIAN_SNAPSHOT}/' \
                'https://mirrors.aliyun.com/debian' \
              --replace-fail \
                '--foreign sid' \
                '--foreign bookworm'
          '';

          jinxRuntimeInputs = with pkgs; [
            bash
            coreutils
            findutils
            gawk
            git
            gnumake
            gnugrep
            gnused
            gnutar
            gzip
            procps
            util-linux
            wget
            zstd
          ];

          jinxCommand = pkgs.writeShellApplication {
            name = "jinx";
            runtimeInputs = jinxRuntimeInputs;
            text = ''
              exec ${pkgs.bash}/bin/bash ${patchedJinx}/jinx "$@"
            '';
          };
        in
        {
          devShells.default = pkgs.mkShell {
            packages = [
              rustToolchain
              jinxCommand
              pkgs.e2fsprogs
              pkgs.jq
              pkgs.tokei
              pkgs.gdb
              pkgs.git
              pkgs.meson
              pkgs.wget
              pkgs.ninja
              pkgs.openssl
              pkgs.OVMF.fd
              pkgs.pkg-config
              pkgs.qemu
              pkgs.xorriso
            ]
            ++ (with pkgs.llvmPackages; [
              clang
              lld
              llvm
            ]);

            OVMF_CODE = "${pkgs.OVMF.fd}/FV/OVMF_CODE.fd";
          };
        };
    };
}
