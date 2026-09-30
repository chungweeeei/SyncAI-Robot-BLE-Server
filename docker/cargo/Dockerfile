FROM ubuntu:22.04

ARG RUST_VERSION=stable
ARG DEBIAN_FRONTEND=noninteractive


RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        pkg-config \
        libdbus-1-dev \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --no-modify-path --profile minimal \
            --default-toolchain "${RUST_VERSION}" \
            --component clippy,rustfmt \
    && rustc --version && cargo --version

RUN mkdir -p "${CARGO_HOME}/registry" "${CARGO_HOME}/git" \
    && chmod 1777 "${CARGO_HOME}/registry" "${CARGO_HOME}/git"

WORKDIR /work
ENTRYPOINT ["cargo"]
