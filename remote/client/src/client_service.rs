use tonic::transport::server::Router;
use tonic::transport::Server;
use trz_gateway_common::is_global::IsGlobal;

pub trait ClientService: IsGlobal {
    fn configure_service(&self, server: Server) -> Router;
}

impl<F: Fn(Server) -> Router + IsGlobal> ClientService for F {
    fn configure_service(&self, server: Server) -> Router {
        self(server)
    }
}
