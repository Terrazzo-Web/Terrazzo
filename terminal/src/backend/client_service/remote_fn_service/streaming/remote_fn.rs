use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt;
use scopeguard::defer;
use tracing::debug;
use tracing::debug_span;
use tracing_futures::Instrument as _;

use super::RemoteFnError;
use super::dispatch::remote_fn_dispatch;
use super::response::local::LocalResponseStream;
use crate::api::client_address::ClientAddress;
use crate::backend::Server;
use crate::backend::client_service::remote_fn_service::remote_fn_server;
use crate::backend::protos::terrazzo::remotefn::RemoteFnRequest;

/// A struct that holds a remote server function.
///
/// They must be statically registered using [inventory::submit].
pub struct RemoteFn<I, O> {
    delegate: RegisteredRemoteFn,
    _phantom: PhantomData<(I, O)>,
}

impl<I, O> Clone for RemoteFn<I, O> {
    fn clone(&self) -> Self {
        Self {
            delegate: self.delegate,
            _phantom: PhantomData,
        }
    }
}

#[derive(Clone, Copy)]
pub struct RegisteredRemoteFn {
    pub(super) name: &'static str,
    pub(super) callback: fn(server: &Arc<Server>, &str) -> RemoteFnResult,
}

impl RegisteredRemoteFn {
    pub const fn new(
        name: &'static str,
        callback: fn(server: &Arc<Server>, &str) -> RemoteFnResult,
    ) -> Self {
        Self { name, callback }
    }
}

/// Shorthand for the result of remote functions.
pub type RemoteFnResult = Pin<Box<dyn Stream<Item = Result<String, RemoteFnError>> + Send>>;

impl<I, O> RemoteFn<I, O> {
    pub const fn new(delegate: RegisteredRemoteFn) -> Self {
        Self {
            delegate,
            _phantom: PhantomData,
        }
    }

    /// Calls the remote function.
    ///
    /// The remote function will be called on the client indicated by `address`.
    ///
    /// Takes care of serializing the request and then deserializing the response.
    pub async fn call(
        &self,
        address: ClientAddress,
        request: I,
    ) -> Result<impl Stream<Item = Result<O, RemoteFnError>>, RemoteFnError>
    where
        I: serde::Serialize,
        O: for<'de> serde::Deserialize<'de>,
    {
        async move {
            debug!("Start");
            defer!(debug!("End"));
            let server = remote_fn_server()?;

            let request =
                serde_json::to_string(&request).map_err(RemoteFnError::SerializeRequest)?;

            let response = remote_fn_dispatch(
                &server,
                &address,
                RemoteFnRequest {
                    address: Default::default(),
                    server_fn_name: self.delegate.name.to_string(),
                    json: request,
                },
            )
            .await?;

            let response = LocalResponseStream(response);
            let response = response.map(|item| match item {
                Ok(item) => match serde_json::from_str(&item) {
                    Ok(item) => Ok(item),
                    Err(error) => Err(RemoteFnError::DeserializeResponse(error, item)),
                },
                Err(error) => Err(RemoteFnError::ServerFn(error)),
            });

            return Ok(response);
        }
        .instrument(debug_span!("RemoteFn"))
        .await
    }
}
