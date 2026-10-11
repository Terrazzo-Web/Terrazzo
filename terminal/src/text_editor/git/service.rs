use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use server_fn::ServerFnError;

use super::FileMetadata;
use crate::api::client_address::ClientAddress;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::grpc_error::IsGrpcError;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::fsio::git::git_repo_root;

pub async fn git_status(
    remote: ClientAddress,
    base: Arc<Path>,
) -> Result<Option<Vec<FileMetadata>>, ServerFnError> {
    Ok(GIT_STATUS_FN.call(remote, base).await?)
}

remote_fn_service::unary::declare_remote_fn!(
    GIT_STATUS_FN,
    "texteditor.git_status",
    Arc<Path>,
    Option<Vec<FileMetadata>>,
    |_server, base| async move {
        git_status_impl(&base)
            .map_err(|error| GitStatusError {
                path: base.clone(),
                source: error,
            })
            .map_err(GrpcError::from)
    }
);

#[derive(Debug, thiserror::Error)]
#[error("Failed to read Git status from {path:?}: {source}")]
struct GitStatusError {
    path: Arc<Path>,
    #[source]
    source: std::io::Error,
}

impl IsGrpcError for GitStatusError {
    fn code(&self) -> tonic::Code {
        tonic::Code::FailedPrecondition
    }
}

/* TODO: use autoclone::graph

I see the following nodes:
- base is the input parameter
- base_from_root is the first node. You need to create a new enum error type where NotGit is one of the enum values, the other ones are the std::io::Errors
- output is the second node and derives from base_from_root
- paths
- files

The result should a typed Result<Vec<FileMetadata>, GitStatusError>

Where GitStatusError is enum GitStatusError { NotGit, GitStatusError(std::io::Error) }

use
#[nameth]
#[derive(Debug, thiserror::Error)]
pub enum GitStatusError {
    #[error("[{n}] {0}", n = self.name())]
    ... error code ...
 */
fn git_status_impl(base: &Path) -> std::io::Result<Option<Vec<FileMetadata>>> {
    let Some(repo_root) = git_repo_root(base) else {
        return Ok(None);
    };
    let base_from_root = base
        .strip_prefix(repo_root.as_ref())
        .unwrap_or_else(|_| Path::new(""));
    let output = Command::new("git")
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignored=no",
            "--",
            ".",
        ])
        .current_dir(base)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }

    // Porcelain v1 with `-z` emits unquoted, NUL-terminated records:
    // - New: `?? PATH\0` when untracked, or `A  PATH\0` when staged.
    // - Changed: ` M PATH\0` in the worktree, `M  PATH\0` in the index, or `MM PATH\0` in both.
    // - Moved: `R  DESTINATION\0SOURCE\0`; copy records use `C` and the same two-path layout.
    // - Deleted: ` D PATH\0` from the worktree or `D  PATH\0` from the index.
    // The first and second status columns (`X` and `Y`) describe the index and worktree.
    let mut records = output.stdout.split(|byte| *byte == 0);
    let mut paths = HashSet::new();
    while let Some(record) = records.next() {
        // A valid record needs two status bytes, one separating space, and at least one path byte.
        // This also skips the empty field after the output's final NUL terminator.
        if record.len() < 4 {
            continue;
        }
        let status = &record[..2];
        let path = PathBuf::from(String::from_utf8_lossy(&record[3..]).into_owned());
        if let Ok(path) = path.strip_prefix(base_from_root) {
            paths.insert(path.to_owned());
        }
        if status.contains(&b'R') || status.contains(&b'C') {
            let _old_path = records.next();
        }
    }

    let mut gids = HashMap::new();
    let mut uids = HashMap::new();
    let mut files = paths
        .into_iter()
        .map(|path| {
            let metadata = std::fs::symlink_metadata(base.join(&path));
            FileMetadata::make(
                path.display().to_string().into(),
                metadata.as_ref(),
                &mut gids,
                &mut uids,
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(Some(files))
}
