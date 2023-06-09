FROM node:latest as builder1

RUN npm install -g pnpm
WORKDIR /app
COPY package.json pnpm-lock.yaml .
RUN pnpm install
COPY . .
RUN npm run build:prod

FROM rust:1 as builder2

WORKDIR /app
COPY . .
COPY --from=builder1 /app/static/main.css /app/static/main.css
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo install --path .



FROM debian:bullseye-slim

COPY --from=builder2 /usr/local/cargo/bin/silly-goals /usr/local/bin/silly-goals

CMD ["silly-goals"]
