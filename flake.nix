{
  description = "THXNET. Rootchain";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      # nightly-2023-04-10
      url = "github:nix-community/fenix?ref=4869bb2408e6778840c8d00be4b45d8353f24723";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    let
      name = "polkadot";
      version = "0.9.40";
    in
    (flake-utils.lib.eachDefaultSystem
      (system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [
              self.overlays.default
              fenix.overlays.default
            ];
          };

          rustToolchain = fenix.packages.${system}.fromToolchainFile {
            file = ./rust-toolchain.toml;
            sha256 = "sha256-DCQf3SCznJP8yCYJ4Vziqq3KZkacs+PrWkCir6y3tGA=";
          };

          rustPlatform = pkgs.makeRustPlatform {
            cargo = rustToolchain;
            rustc = rustToolchain;
          };

          cargoArgs = [
            "--workspace"
            "--bins"
            "--examples"
            "--tests"
            "--benches"
            "--all-targets"
          ];

          unitTestArgs = [
            "--workspace"
          ];
        in
        rec {
          formatter = pkgs.treefmt;

          devShells.default = pkgs.callPackage ./devshell {
            inherit rustToolchain cargoArgs unitTestArgs;
          };

          packages = rec {
            default = polkadot;
            polkadot = pkgs.callPackage ./devshell/package.nix {
              inherit name version rustPlatform;
            };
            container = pkgs.callPackage ./devshell/container.nix {
              inherit name version polkadot;
            };
          };

          apps.default = flake-utils.lib.mkApp {
            drv = packages.polkadot;
            exePath = "/bin/polkadot";
          };
        })) // {
      overlays.default = final: prev: {
        thxnet-parachain-node = final.callPackage ./devshell/package.nix {
          inherit name version;
        };
      };
    };
}
