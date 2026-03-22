#!/bin/bash

IP="89.167.85.155"
cargo build --release --target x86_64-unknown-linux-musl
scp target/x86_64-unknown-linux-musl/release/mac-mgmt root@$IP:
ssh root@$IP ./mac-mgmt "$@"
