FROM rust:1 as builder

WORKDIR /app
COPY . .
RUN cargo install --path .


FROM debian:bullseye-slim

COPY --from=builder /usr/local/cargo/bin/silly-goals /usr/local/bin/silly-goals

CMD ["silly-goals"]
