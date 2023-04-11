{ name
, version
, lib
, rustPlatform
, llvmPackages
, protobuf
}:

rustPlatform.buildRustPackage {
  pname = name;
  inherit version;

  src = lib.cleanSource ./..;

  cargoLock = {
    lockFile = ../Cargo.lock;
    outputHashes = {
      "binary-merkle-tree-4.0.0-dev" = "sha256-YxCAFrLWTmGjTFzNkyjE+DNs2cl4IjAlB7qz0KPN1vE=";
      "sub-tokens-0.1.0" = "sha256-GvhgZhOIX39zF+TbQWtTCgahDec4lQjH+NqamLFLUxM=";
    };
  };

  nativeBuildInputs = [
    llvmPackages.clang
    llvmPackages.libclang

    protobuf
  ];

  doCheck = false;

  PROTOC = "${protobuf}/bin/protoc";
  PROTOC_INCLUDE = "${protobuf}/include";

  LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib";

  SUBSTRATE_CLI_GIT_COMMIT_HASH = "";

  # NOTE: We don't build the WASM runtimes since this would require a more
  # complicated rust environment setup and this is only needed for developer
  # environments. The resulting binary is useful for end-users of live networks
  # since those just use the WASM blob from the network chainspec.
  # See also: https://docs.rs/substrate-wasm-builder/latest/substrate_wasm_builder/#environment-variables
  SKIP_WASM_BUILD = 1;
}
