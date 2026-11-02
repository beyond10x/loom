# syntax=docker/dockerfile:1.7
FROM rust:1.99-bookworm@sha256:9181b56c2a21ab1d5e73cffe04fc762426d30aa7fac3c91141dd033f49a7fc8b AS builder
WORKDIR /src
COPY . .
RUN --mount=type=cache,id=b10x-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=b10x-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=loom-agent-target,target=/src/target,sharing=locked \
    cargo build --release --locked -p b10x-loom-cli --bin b10x-loom && \
    install -D /src/target/release/b10x-loom /out/b10x-loom

FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3
ARG SOURCE_SHA=unknown
LABEL org.opencontainers.image.source="https://github.com/beyond10x/loom" \
      org.opencontainers.image.description="b10x-loom, the Loom agent runtime" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.revision=$SOURCE_SHA
RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates libssl3 libcurl4 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /out/b10x-loom /usr/local/bin/b10x-loom
RUN if ldd /usr/local/bin/b10x-loom | grep 'not found'; then exit 1; fi && \
    /usr/local/bin/b10x-loom --version
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/b10x-loom"]
