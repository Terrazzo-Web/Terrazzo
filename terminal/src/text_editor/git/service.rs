use std::path::Path;
use std::sync::Arc;

use nameth::NamedEnumValues as _;
use nameth::nameth;
use server_fn::ServerFnError;
use terrazzo::graph;

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

/// ```graph
/// run ←── files ←── paths ←── output ←── base_from_root
///                     │                        │
///                     └────────────────────────┘
/// ```
#[graph]
mod git_diff_ls {
    use std::collections::HashMap;
    use std::collections::HashSet;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::Output;
    use std::sync::Arc;
    use tokio::process::Command;
    use tracing::debug;

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
                "--literal-pathspecs",
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=all",
                "--ignored=no",
                "--",
            ])
            .arg(if relative.as_os_str().is_empty() {
                Path::new(".")
            } else {
                relative.as_path()
            })
            .current_dir(repo_root)
            .output()
            .await?;
        if !output.status.success() {
            return Err(GitDiffLsError::GitStatusError(format!(
                "{}: repo_root={repo_root:?} {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim_end(),
            )));
        }
        Ok(output)
    }

    fn base_from_root(base: &Path) -> Result<(Arc<Path>, PathBuf), GitDiffLsError> {
        let repo_root = git_repo_root(base).ok_or(GitDiffLsError::NotGit)?;
        debug!("Found repo root:{repo_root:?} of base:{base:?}");
        let relative = base
            .strip_prefix(repo_root.as_ref())
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidInput, error))?
            .to_owned();
        Ok((repo_root, relative))
    }
}

#[cfg(test)]
mod tests {
    use super::GitDiffLsError;
    use super::git_diff_ls;

    #[tokio::test]
    async fn empty_base_is_not_git() {
        assert!(matches!(
            git_diff_ls::run(std::path::Path::new("")).await,
            Err(GitDiffLsError::NotGit)
        ));
    }

    #[tokio::test]
    async fn lists_changes_relative_to_base() {
        let folder = tempfile::tempdir().unwrap();
        let output = tokio::process::Command::new("git")
            .arg("init")
            .current_dir(folder.path())
            .output()
            .await
            .unwrap();
        assert!(output.status.success());
        let base = folder.path().join("nested[folder]");
        std::fs::create_dir(&base).unwrap();
        std::fs::write(base.join("changed.txt"), "changed").unwrap();
        std::fs::write(folder.path().join("outside.txt"), "outside").unwrap();

        let files = git_diff_ls::run(&base).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name.as_ref(), "changed.txt");
    }
}
