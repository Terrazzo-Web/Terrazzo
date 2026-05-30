use std::sync::Arc;

use scopeguard::defer;
use tracing::Instrument as _;
use tracing::debug;
use tracing::debug_span;

use super::RemoteFnError;
use super::callback::DistributedFn;
use crate::backend::Server;
use crate::backend::client_service::routing::DistributedCallback as _;
use crate::backend::protos::terrazzo::remotefn::RemoteFnRequest;

/// Calls a [RemoteFn] using the [DistributedCallback] framework.
pub fn remote_fn_dispatch(
    server: &Arc<Server>,
    client_address: &[impl AsRef<str>],
    request: RemoteFnRequest,
) -> impl Future<Output = Result<String, RemoteFnError>> {
    async move {
        debug!("Start");
        defer!(debug!("Done"));
        DistributedFn::process(server, client_address, request)
            .await
            .map_err(|error| RemoteFnError::Distributed(Box::new(error)))
    }
    .instrument(debug_span!("DistributedFn"))
}
