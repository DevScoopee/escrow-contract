# Pinned build environment for reproducible milestone_escrow.wasm builds
# (issue #773). Used by scripts/build-reproducible.sh; see the README section
# "Verifying the deployed contract".
#
# Everything that can change the output bytes is pinned:
#   * the Rust toolchain, by image digest (rust:1.96.0-slim-bookworm, matching
#     rust-toolchain.toml);
#   * stellar-cli, by release version and SHA-256 of the release tarball
#     (`stellar contract build` embeds its own version into the wasm metadata);
#   * the platform: always linux/amd64, also on arm64 hosts (emulated).
# The apt packages below are runtime libraries of the stellar binary only and
# do not affect the compiled contract.

FROM --platform=linux/amd64 rust:1.96.0-slim-bookworm@sha256:4732ca96fd086cb9be682050c3f0176288eebaac2b80aa2bcefccfaf198e1950

ARG STELLAR_CLI_VERSION=28.0.0
ARG STELLAR_CLI_SHA256=207544486734fccb4df1afc4a7745478f9f1e21688b2f9506f0ef36f60ce3fdc

RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl libdbus-1-3 libudev1 \
 && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL -o /tmp/stellar-cli.tar.gz \
      "https://github.com/stellar/stellar-cli/releases/download/v${STELLAR_CLI_VERSION}/stellar-cli-${STELLAR_CLI_VERSION}-x86_64-unknown-linux-gnu.tar.gz" \
 && echo "${STELLAR_CLI_SHA256}  /tmp/stellar-cli.tar.gz" | sha256sum -c - \
 && tar -xzf /tmp/stellar-cli.tar.gz -C /usr/local/bin stellar \
 && rm /tmp/stellar-cli.tar.gz \
 && stellar --version

RUN rustup target add wasm32v1-none

# Fixed paths inside the container, so any path the compiler embeds is the same
# on every machine.
ENV CARGO_HOME=/tmp/cargo-home
WORKDIR /build
