#!/bin/bash

set -euxo pipefail

IP="2a01:4f9:1a:90eb::cd"
cargo build --release --target x86_64-unknown-linux-musl -p mac-mgmt
ssh root@$IP rm -f mac-mgmt-new
scp target/x86_64-unknown-linux-musl/release/mac-mgmt root@$IP:mac-mgmt-new
ssh root@$IP mv mac-mgmt-new mac-mgmt
ssh root@$IP ./mac-mgmt "$@"
