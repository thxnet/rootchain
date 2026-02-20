variable "TAG" {
  default = "develop"
}

variable "CONTAINER_REGISTRY" {
  default = "ghcr.io/thxnet"
}

group "default" {
  targets = [
    "rootchain",
  ]
}

group "all" {
  targets = [
    "rootchain",
    "wasm-artifacts",
  ]
}

target "rootchain" {
  dockerfile = "scripts/ci/dockerfiles/polkadot/polkadot_builder.Dockerfile"
  target     = "rootchain"
  tags       = ["${CONTAINER_REGISTRY}/rootchain:${TAG}"]
  platforms  = ["linux/amd64"]
  args = {
    RUSTC_WRAPPER         = "/usr/bin/sccache"
    AWS_ACCESS_KEY_ID     = null
    AWS_SECRET_ACCESS_KEY = null
    SCCACHE_BUCKET        = null
    SCCACHE_ENDPOINT      = null
    SCCACHE_S3_USE_SSL    = null
    SCCACHE_REGION        = null
  }
  label = {
    "description"                 = "Container image for THXNET."
    "io.thxnet.image.type"        = "final"
    "io.thxnet.image.authors"     = "contact@thxlab.io"
    "io.thxnet.image.vendor"      = "thxlab.io"
    "io.thxnet.image.description" = "THXNET.: The Hybrid Next-Gen Blockchain Network"
  }
  contexts = {
    sccache         = "docker-image://ghcr.io/thxnet/ci-containers/sccache:0.14.0"
    substrate-based = "docker-image://ghcr.io/thxnet/ci-containers/substrate-based:build-2023.05.20-41956af"
    ubuntu          = "docker-image://docker.io/library/ubuntu:22.04"
  }
}

target "wasm-artifacts" {
  dockerfile = "scripts/ci/dockerfiles/polkadot/polkadot_builder.Dockerfile"
  target     = "wasm-artifacts"
  platforms  = ["linux/amd64"]
  output     = ["type=local,dest=./artifacts"]
  args = {
    RUSTC_WRAPPER         = "/usr/bin/sccache"
    AWS_ACCESS_KEY_ID     = null
    AWS_SECRET_ACCESS_KEY = null
    SCCACHE_BUCKET        = null
    SCCACHE_ENDPOINT      = null
    SCCACHE_S3_USE_SSL    = null
    SCCACHE_REGION        = null
  }
  contexts = {
    sccache         = "docker-image://ghcr.io/thxnet/ci-containers/sccache:0.14.0"
    substrate-based = "docker-image://ghcr.io/thxnet/ci-containers/substrate-based:build-2023.05.20-41956af"
  }
}
