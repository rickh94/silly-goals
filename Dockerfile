FROM node:latest as builder1

RUN npm install -g pnpm
WORKDIR /app
COPY package.json pnpm-lock.yaml .
RUN pnpm install
COPY . .
RUN npm run build:prod

FROM rust:1-slim-bookworm as builder2

RUN apt-get update && apt-get install -y libssl-dev openssl pkg-config 

WORKDIR /app
COPY . .
COPY --from=builder1 /app/static/main.css /app/static/main.css
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo install --path .


FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl-dev openssl && apt clean && rm -rf /var/lib/apt/lists/*

COPY --from=builder2 /usr/local/cargo/bin/silly-goals /usr/local/bin/silly-goals

CMD ["silly-goals"]
