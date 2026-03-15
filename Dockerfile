FROM rust:1.75-slim AS builder
WORKDIR /build
COPY . .
RUN cargo build --release -p cabalist-cli

FROM debian:bookworm-slim
COPY --from=builder /build/target/release/cabalist-cli /usr/local/bin/
ENTRYPOINT ["cabalist-cli"]
