FROM rustlang/rust:nightly AS builder
WORKDIR /urldebloater

COPY Cargo.toml .
COPY Cargo.lock .
# workspace mocks
RUN cargo new desktop
RUN cargo new urlwasher
RUN cargo new mixer

# urlwasher deps dummy cache layer
COPY urlwasher/Cargo.toml urlwasher/Cargo.toml
RUN cargo build --release

# mixer deps dummy cache layer
COPY mixer/Cargo.toml mixer/Cargo.toml
RUN cargo build --release

# build urlwasher
COPY urlwasher/src/ urlwasher/src/
RUN cargo build --release

COPY mixer/src/ mixer/src/
# force rebuild main.rs
RUN touch mixer/src/main.rs
RUN cargo build --release

FROM debian:bullseye AS runtime
RUN apt-get update && apt install -y tini ca-certificates
WORKDIR /mixer
COPY --from=builder /urldebloater/target/release/urldebloater-mixer /usr/local/bin

ENV RUST_LOG=info
ENTRYPOINT ["tini", "--"]
CMD  ["/usr/local/bin/urldebloater-mixer"]
EXPOSE 7777
