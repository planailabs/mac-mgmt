#!/usr/bin/env bash

cargo build --release --target x86_64-unknown-linux-musl
cargo build --release --target aarch64-apple-darwin

target/x86_64-unknown-linux-musl/release/mac-mgmt
