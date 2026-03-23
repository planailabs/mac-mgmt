#!/bin/bash

set -euxo pipefail

IP="89.167.85.155"
cargo build --release --target x86_64-unknown-linux-musl
scp target/x86_64-unknown-linux-musl/release/mac-mgmt root@$IP:mac-mgmt-new
ssh root@$IP mv mac-mgmt-new mac-mgmt
ssh root@$IP ./mac-mgmt "$@"
