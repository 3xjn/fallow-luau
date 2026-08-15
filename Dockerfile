# Minimal image: build fallow-luau, run as a one-shot CLI against /workspace.
#   docker build -t fallow-luau:local https://github.com/3xjn/fallow-luau.git#main
#   docker run --rm -v "$PWD:/workspace" --user "$(id -u):$(id -g)" fallow-luau:local health --root . --explain
FROM rust:1-bookworm AS build
WORKDIR /src
COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY src ./src
# Honor rust-toolchain.toml channel when available.
RUN rustup show && cargo build --release --bin fallow-luau

FROM debian:bookworm-slim
RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates git \
  && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/fallow-luau /usr/local/bin/fallow-luau
WORKDIR /workspace
ENTRYPOINT ["fallow-luau"]
CMD ["--help"]
