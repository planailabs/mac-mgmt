server: cd server && env DEV_ONLY_NO_AUTH=1 dx serve | tee /tmp/mac-mgmt-server.log
relay: cd relay && RUST_LOG=debug cargo watch -- cargo run | tee /tmp/mac-mgmt-relay.log
tailwind: cd server && npm run tailwind
