#!/usr/bin/env bash
set -euxo pipefail

cargo build --release --target x86_64-unknown-linux-musl
cargo zigbuild --release --target aarch64-apple-darwin
