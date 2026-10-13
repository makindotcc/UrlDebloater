FROM rustlang/rust:nightly-bullseye AS builder
WORKDIR /urldebloater

COPY Cargo.toml .
COPY Cargo.lock .
COPY crates/urlwasher/Cargo.toml crates/urlwasher/Cargo.toml
COPY crates/mixer/Cargo.toml crates/mixer/Cargo.toml

# Build dependencies first, so edits to mixer sources reuse this layer.
RUN mkdir -p crates/urlwasher/src crates/mixer/src \
    && : > crates/urlwasher/src/lib.rs \
    && printf 'fn main() {}\n' > crates/mixer/src/main.rs \
    && cargo build --release --locked -p urldebloater-mixer

COPY crates/urlwasher/src/ crates/urlwasher/src/
COPY crates/mixer/src/ crates/mixer/src/
RUN touch crates/mixer/src/main.rs \
    && cargo build --release --locked -p urldebloater-mixer

FROM debian:bullseye AS runtime
RUN apt-get update && apt install -y tini ca-certificates
WORKDIR /mixer
COPY --from=builder /urldebloater/target/release/urldebloater-mixer /usr/local/bin

ENV RUST_LOG=info
ENTRYPOINT ["tini", "--"]
CMD  ["/usr/local/bin/urldebloater-mixer"]
EXPOSE 7777
