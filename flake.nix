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
              exec ${pkgs.bash}/bin/bash ${jinx}/jinx "$@"
            '';
          };
        in
        {
          devShells.default = pkgs.mkShell {
            packages =
              [
                rustToolchain
                jinxCommand
                pkgs.e2fsprogs
                pkgs.gdb
                pkgs.git
                pkgs.limine
                pkgs.meson
                pkgs.mtools
                pkgs.ninja
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
          };
        };
    };
}
