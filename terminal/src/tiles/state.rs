#![cfg(feature = "tiles-state")]

macro_rules! make_state {
($name:ident, $ty:ty) => {
    pub mod $name {
        use server_fn::ServerFnError;
        use terrazzo::server;

        pub mod ty {
            pub type Type = $ty;

            #[allow(unused)]
            pub use super::super::*;
        }

        #[cfg(feature = "server")]
        mod state {
            use std::collections::HashMap;
            use std::hash::BuildHasherDefault;
            use std::hash::DefaultHasher;
            use crate::tiles::id::TileId;

            pub static STATE: std::sync::Mutex<HashMap<Option<TileId>, super::ty::Type, BuildHasherDefault<DefaultHasher>>> =
                std::sync::Mutex::new(HashMap::with_hasher(BuildHasherDefault::new()));
        }

        use crate::api::client_address::ClientAddress;
        use crate::tiles::id::TileId;

        #[cfg_attr(feature = "server", allow(unused))]
        #[server(protocol = ::server_fn::Http<::server_fn::codec::Json, ::server_fn::codec::Json>)]
        #[cfg_attr(feature = "server", nameth::nameth)]
        pub async fn get(
            tile: Option<TileId>,
            remote: ClientAddress,
        ) -> Result<ty::Type, ServerFnError> {
            Ok(remote::GET_REMOTE_FN
                .call(remote, remote::GetRequest { tile })
                .await?)
        }

        #[cfg(feature = "client")]
        use crate::frontend::remotes::Remote;

        #[cfg(feature = "client")]
        pub async fn set(
            tile: Option<TileId>,
            remote: Remote,
            value: ty::Type,
        ) -> Result<(), ServerFnError> {
            use std::pin::Pin;
            use std::sync::OnceLock;
            use std::time::Duration;
            use futures::future::Shared;
            use terrazzo::prelude::diagnostics::warn;
            use terrazzo::widgets::debounce::DoDebounce as _;

            struct ThreadSafe(
                Box<dyn Fn((Option<TileId>, Remote, ty::Type)) -> Shared<Pin<Box<dyn Future<Output = ()> + Send + Sync>>>>,
            );

            unsafe impl Send for ThreadSafe {}
            unsafe impl Sync for ThreadSafe {}

            static DEBOUNCED_SET: OnceLock<ThreadSafe> = OnceLock::new();
            const STORE_STATE_DEBOUNCE_DELAY: Duration = Duration::from_millis(100);

            let debounced_set = DEBOUNCED_SET.get_or_init(|| {
                ThreadSafe(Box::new(
                    STORE_STATE_DEBOUNCE_DELAY.async_debounce(|(tile, remote, value)| async move {
                        set_impl(tile, remote, value)
                            .await
                            .unwrap_or_else(|error| warn!("Failed to save: {error}"))
                    }),
                ))
            });
            let debounced_set = &*debounced_set.0;

            let () = debounced_set((tile, remote, value)).await;
            Ok(())
        }

        #[cfg_attr(feature = "server", allow(unused))]
        #[server(protocol = ::server_fn::Http<::server_fn::codec::Json, ::server_fn::codec::Json>)]
        #[cfg_attr(feature = "server", nameth::nameth)]
        async fn set_impl(
            tile: Option<TileId>,
            remote: ClientAddress,
            value: ty::Type,
        ) -> Result<(), ServerFnError> {
            Ok(remote::SET_REMOTE_FN
                .call(remote, remote::SetRequest { tile, value })
                .await?)
        }

        #[cfg(feature = "server")]
        mod remote {
            use std::future::ready;

            use const_format::formatcp;
            use serde::Deserialize;
            use serde::Serialize;

            use crate::backend::client_service::remote_fn_service;
            use crate::tiles::id::TileId;

            #[derive(Debug, Default, Serialize, Deserialize)]
            #[serde(default)]
            pub struct GetRequest {
                #[cfg_attr(not(feature = "diagnostics"), serde(rename = "t"))]
                pub(super) tile: Option<TileId>,
            }

            #[derive(Debug, Default, Serialize, Deserialize)]
            #[serde(default)]
            pub struct SetRequest {
                #[cfg_attr(not(feature = "diagnostics"), serde(rename = "t"))]
                pub(super) tile: Option<TileId>,

                #[cfg_attr(not(feature = "diagnostics"), serde(rename = "v"))]
                pub(super) value: super::ty::Type,
            }

            remote_fn_service::unary::declare_remote_fn!(
                GET_REMOTE_FN,
                formatcp!("{}-state-{}", super::GET, stringify!($name)),
                GetRequest,
                super::ty::Type,
                |_server, request: GetRequest| {
                    let state = super::state::STATE.lock().expect(stringify!($name));
                    ready(Ok::<super::ty::Type, StateError>(
                        state.get(&request.tile).cloned().unwrap_or_default(),
                    ))
                }
            );

            remote_fn_service::unary::declare_remote_fn!(
                SET_REMOTE_FN,
                formatcp!("{}-state-{}", super::SET_IMPL, stringify!($name)),
                SetRequest,
                (),
                |_server, request: SetRequest| {
                    let mut state = super::state::STATE.lock().expect(stringify!($name));
                    state.insert(request.tile, request.value);
                    ready(Ok::<(), StateError>(()))
                }
            );

            enum StateError {}

            impl From<StateError> for tonic::Status {
                fn from(value: StateError) -> Self {
                    match value {}
                }
            }
        }
    }
};
}

pub(crate) use make_state;
