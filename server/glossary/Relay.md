# Relay

The connection broker between server and daemons. Daemons hold a persistent
connection to the relay, which multiplexes file tunnels, shell tunnels, SSH
port forwarding, and the HTTP proxy the server (and healer) use to reach
instances without inbound connectivity.
