#![cfg(feature = "server")]

use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use super::server_fn::AutocompleteItem;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::path_selector::schema::PathSelector;

#[derive(Debug, Serialize, Deserialize)]
pub struct AutoCompletePathRequest {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "k"))]
    pub kind: PathSelector,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "p"))]
    pub prefix: Arc<Path>,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "i"))]
    pub input: String,
}

remote_fn_service::unary::declare_remote_fn!(
    AUTOCOMPLETE_PATH_REMOTE_FN,
    super::server_fn::AUTOCOMPLETE_PATH,
    AutoCompletePathRequest,
    Vec<AutocompleteItem>,
    |server, arg| {
        let settings = server.config().server.with(|server| {
            crate::text_editor::search::service::tantivy::IndexSettings {
                cache_dir: server.tantivy_cache.clone(),
                refresh_interval: server.search_index_refresh,
                stale_after: server.search_index_stale_after,
            }
        });
        async move {
            let result = super::service::autocomplete_path(arg.kind, &arg.prefix, &arg.input);
            let mut result = result.map_err(GrpcError::from)?;
            if arg.kind == PathSelector::FilePath && !arg.input.trim().is_empty() {
                match crate::text_editor::search::service::autocomplete_paths(
                    &arg.prefix,
                    arg.input.trim(),
                    settings,
                    30,
                )
                .await
                {
                    Ok(paths) => {
                        for path in paths {
                            let Ok(metadata) = path.metadata() else {
                                continue;
                            };
                            let Ok(path) = path.strip_prefix(&arg.prefix) else {
                                continue;
                            };
                            let path = path.to_string_lossy().into_owned();
                            if !result.iter().any(|item| item.path == path) {
                                result.push(AutocompleteItem {
                                    path,
                                    is_dir: metadata.is_dir(),
                                });
                            }
                        }
                    }
                    Err(error) => tracing::warn!(%error, "Failed to autocomplete indexed paths"),
                }
            }
            Ok::<_, GrpcError<super::service::AutoCompleteError>>(result)
        }
    }
);
