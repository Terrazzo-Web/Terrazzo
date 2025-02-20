use tonic::transport::server::Router;
use tonic::transport::Server;
use trz_gateway_common::is_configuration::IsConfiguration;

pub trait ClientService: IsConfiguration {
    fn configure_service(&self, server: Server) -> Router;
}

impl<F: Fn(Server) -> Router + IsConfiguration> ClientService for F {
    fn configure_service(&self, server: Server) -> Router {
        self(server)
    }
}
