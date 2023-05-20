variable "TAG" {
    default = "develop"
}

variable "REPOSITORY" {
    default = "ghcr.io"
}

variable "DEBUG" {
    default = "0"
}

group "default" {
    targets = [
        "rootchain",
    ]
}

target "base" {
    dockerfile = "scripts/ci/dockerfiles/polkadot/polkadot_builder.Dockerfile"
    args = {
      DEBUG = "${DEBUG}"
    }
    platforms = ["linux/amd64"]
}

target "rootchain" {
    inherits = ["base"]
    target = "rootchain"
    tags = ["${REPOSITORY}/thxnet-relaychain-node:${TAG}"]
}
