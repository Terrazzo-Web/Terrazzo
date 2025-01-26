# Terrazzo Gateway

The Terrazzo Gateway allows clients (aka. remote services) to expose gRPC APIs
through the Gateway.

The remote services need outbound connectivity to the Gateway.
Only the Gateway is publicly available.

## Remote services tunnel connections

Remote services open a WebSocket connections with the Gateway.
These WebSockets are then used to transport gRPC traffic.

The connection is encrypted twice
1. The WebSocket connection is transported over HTTPS
2. The gRPC connection is secured with TLS, using the remote service certificate.

## Remote services authentication

Each remote service authenticates with a TLS certificate.

The TLS certificate is issued by the Gateway in exchange for a code that can be
queried from the Gateway and that changes every 60 seconds.
