//! The gRPC server that runs in the client.

use tonic::transport::Server;
use tonic::transport::server::Router;
use trz_gateway_common::is_global::IsGlobal;

/// A trait to configure the gRPC server that runs in the client.
///
/// By default only the health check keep-alive service is implemented.
pub trait ClientService: IsGlobal {
    fn configure_service(&self, server: Server) -> Router;
}

impl<F: Fn(Server) -> Router + IsGlobal> ClientService for F {
    fn configure_service(&self, server: Server) -> Router {
        self(server)
    }
}
