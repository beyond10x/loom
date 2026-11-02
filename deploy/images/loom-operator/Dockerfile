# syntax=docker/dockerfile:1.7
FROM rust:1.99-bookworm@sha256:9181b56c2a21ab1d5e73cffe04fc762426d30aa7fac3c91141dd033f49a7fc8b AS builder
WORKDIR /src
COPY . .
RUN --mount=type=cache,id=b10x-cargo-registry,target=/usr/local/cargo/registry,sharing=locked \
    --mount=type=cache,id=b10x-cargo-git,target=/usr/local/cargo/git,sharing=locked \
    --mount=type=cache,id=loom-operator-target,target=/src/target,sharing=locked \
    cargo build --release --locked -p b10x-loom-operator --bin b10x-loom-operator && \
    install -D /src/target/release/b10x-loom-operator /out/b10x-loom-operator

FROM debian:bookworm-slim@sha256:99c0b25c46d0214d5a2091092e7908907dcaecd6a962cde0ac700acee222b6f3
ARG SOURCE_SHA=unknown
LABEL org.opencontainers.image.source="https://github.com/beyond10x/loom" \
      org.opencontainers.image.description="b10x-loom-operator, the controller that runs LoomAgent resources" \
      org.opencontainers.image.licenses="Apache-2.0" \
      org.opencontainers.image.revision=$SOURCE_SHA
RUN apt-get update && \
    apt-get install --yes --no-install-recommends ca-certificates libssl3 libcurl4 && \
    rm -rf /var/lib/apt/lists/*
COPY --from=builder /out/b10x-loom-operator /usr/local/bin/b10x-loom-operator
RUN if ldd /usr/local/bin/b10x-loom-operator | grep 'not found'; then exit 1; fi && \
    /usr/local/bin/b10x-loom-operator --version
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/b10x-loom-operator"]
