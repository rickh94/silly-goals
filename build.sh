#!/usr/bin/env bash
cargo install sqlx
sqlx migrate run
cargo build --release
