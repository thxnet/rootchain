# syntax=docker/dockerfile:1.4

FROM sccache AS sccache

# This is the build stage for THXNET. node. Here we create the binary in a temporary image.
FROM substrate-based as builder

COPY --from=sccache /usr/bin/sccache /usr/bin/sccache

WORKDIR /build
COPY . /build

ARG RUSTC_WRAPPER="/usr/bin/sccache"
ARG AWS_ACCESS_KEY_ID
ARG AWS_SECRET_ACCESS_KEY
ARG SCCACHE_BUCKET
ARG SCCACHE_ENDPOINT
ARG SCCACHE_S3_USE_SSL
ARG SCCACHE_REGION

RUN cargo build --locked --release && sccache --show-stats

# This is the 2nd stage: a very small image where we copy the THXENT. binary."
FROM ubuntu as rootchain

COPY --from=builder /build/target/release/polkadot /usr/local/bin

RUN <<EOF
#!/usr/bin/env bash

set -eu

useradd -m -u 1000 -U -s /bin/sh -d /rootchain thxnet

mkdir -p /data /rootchain/.local/share

chown -R thxnet:thxnet /data

ln -s /data /rootchain/.local/share/polkadot

# unclutter and minimize the attack surface
rm -rf /usr/bin /usr/sbin

# check if executable works in this container
/usr/local/bin/polkadot --version

EOF

USER thxnet

VOLUME ["/data"]

ENTRYPOINT ["/usr/local/bin/polkadot"]
