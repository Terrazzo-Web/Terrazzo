use std::path::Path;
use std::sync::Arc;

use autoclone::graph;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;

use super::FileMetadata;
use crate::api::client_address::ClientAddress;
use crate::backend::client_service::grpc_error::GrpcError;
use crate::backend::client_service::grpc_error::IsGrpcError;
use crate::backend::client_service::remote_fn_service;
use crate::text_editor::fsio::git::git_repo_root;

pub async fn git_diff_ls(
    remote: ClientAddress,
    base: Arc<Path>,
) -> Result<Option<Vec<FileMetadata>>, ServerFnError> {
    Ok(GIT_DIFF_LS_FN.call(remote, base).await?)
}

remote_fn_service::unary::declare_remote_fn!(
    GIT_DIFF_LS_FN,
    "texteditor.git_diff_ls",
    Arc<Path>,
    Option<Vec<FileMetadata>>,
    |_server, base| async move {
        match git_diff_ls::run(&base).await {
            Ok(files) => Ok(Some(files)),
            Err(GitDiffLsError::NotGit) => Ok(None),
            Err(error) => Err(GrpcError::from(error)),
        }
    }
);

#[nameth]
#[derive(Debug, thiserror::Error)]
pub enum GitDiffLsError {
    #[error("[{n}] Not a Git repository", n = self.name())]
    NotGit,

    #[error("[{n}] {0}", n = self.name())]
    IoError(#[from] std::io::Error),

    #[error("[{n}] git status failed: {0}", n = self.name())]
    GitStatusError(String),
}

impl IsGrpcError for GitDiffLsError {
    fn code(&self) -> tonic::Code {
        tonic::Code::FailedPrecondition
    }
}

#[graph]
mod git_diff_ls {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Output;
    use std::sync::Arc;
    use tokio::process::Command;

    use super::FileMetadata;
    use super::GitDiffLsError;
    use super::git_repo_root;

    pub fn run(files: Vec<FileMetadata>) -> Result<Vec<FileMetadata>, GitDiffLsError> {
        Ok(files)
    }

    fn files(base: &Path, paths: HashSet<PathBuf>) -> Vec<FileMetadata> {
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
        files
    }

    fn paths(output: Output, base_from_root: &(Arc<Path>, PathBuf)) -> HashSet<PathBuf> {
        let (_, base_from_root) = base_from_root;
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

        paths
    }

    async fn output(base_from_root: &(Arc<Path>, PathBuf)) -> Result<Output, GitDiffLsError> {
        let (repo_root, relative) = base_from_root;
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
            .current_dir(repo_root.join(relative))
            .output()
            .await?;
        if !output.status.success() {
            return Err(GitDiffLsError::GitStatusError(format!(
                "{}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim_end(),
            )));
        }
        Ok(output)
    }

    fn base_from_root(base: &Path) -> Result<(Arc<Path>, PathBuf), GitDiffLsError> {
        let repo_root = git_repo_root(base).ok_or(GitDiffLsError::NotGit)?;
        let relative = base
            .strip_prefix(repo_root.as_ref())
            .unwrap_or_else(|_| Path::new(""))
            .to_owned();
        Ok((repo_root, relative))
    }
}
