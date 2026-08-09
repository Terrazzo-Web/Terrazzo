use std::io::ErrorKind;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use futures::Stream;
use futures::StreamExt as _;
use tokio::io::AsyncBufReadExt as _;
use tokio::io::BufReader;
use tokio_stream::wrappers::LinesStream;

use super::SearchError;

pub async fn git_files(
    repo_root: Arc<Path>,
    base: Arc<Path>,
) -> Result<impl Stream<Item = Result<PathBuf, SearchError>>, SearchError> {
    let pathspec = base
        .strip_prefix(&repo_root)
        .map_err(SearchError::InvalidRepoRootPrefix)?;
    let pathspec = if pathspec.as_os_str().is_empty() {
        Path::new(".")
    } else {
        pathspec
    }
    .to_owned();
    let process = tokio::process::Command::new("git")
        .current_dir(&repo_root)
        .arg("--literal-pathspecs")
        .args(["ls-files"])
        .args(["--cached", "--others", "--exclude-standard", "--"])
        .arg(&pathspec)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(SearchError::GitLsFilesError)?;
    let stdout = process.stdout.ok_or_else(|| {
        SearchError::GitLsFilesError(std::io::Error::new(
            ErrorKind::BrokenPipe,
            "git ls-files didn't return anything",
        ))
    })?;

    let lines = BufReader::new(stdout).lines();
    let stream = LinesStream::new(lines);
    let stream = stream.map(|line| line.map_err(SearchError::GitLsFilesError));
    let stream = stream.map(move |line| {
        if pathspec == Path::new(".") {
            return Ok(PathBuf::from(line?));
        }
        Ok(PathBuf::from(line?)
            .strip_prefix(&pathspec)
            .map_err(SearchError::InvalidRepoRootPrefix)?
            .to_owned())
    });
    #[cfg(debug_assertions)]
    let stream = {
        let mut i = 0;
        stream.flat_map(move |row| {
            i += 1;
            futures::stream::once(async move {
                if i % 100 == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                row
            })
        })
    };
    #[cfg(debug_assertions)]
    let stream = stream.inspect(|row| tracing::trace!("Row: {row:?}"));
    Ok(stream)
}
