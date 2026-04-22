use std::path::PathBuf;

use clap::Parser;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo_css_shared::ScssError;
use terrazzo_css_shared::config::Config;
use terrazzo_css_shared::config::ConfigError;
use terrazzo_css_shared::hasher::ClassNameHasher;
use terrazzo_css_shared::rewrite_classes;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help = true)]
struct ScssCli {
    /// The crate root dir, i.e the folder containing the crate's Cargo.toml manifest.
    #[arg(required = true)]
    manifest_dir: PathBuf,

    /// Generate a file with all scss modules concatenated
    #[arg(long)]
    output_file: Option<PathBuf>,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
enum ScssCliError {
    #[error("[{n}] {0}", n = self.name())]
    ConfigError(#[from] ConfigError),

    #[error("[{n}] Failed to read '{0}': {1}", n = self.name())]
    ReadFileError(PathBuf, std::io::Error),

    #[error("[{n}] Failed to parse manifest: {0}", n = self.name())]
    ScssError(#[from] ScssError),

    #[error("[{n}] Failed to read '{0}': {1}", n = self.name())]
    WriteFileError(PathBuf, std::io::Error),

    #[error("[{n}] Failed to resolve the folder of output_file={0}", n = self.name())]
    GetOutputFileFolder(PathBuf),

    #[error("[{n}] Failed to create the folder of output_file={0}: {1}", n = self.name())]
    CreateOutputFileFolder(PathBuf, std::io::Error),
}

fn main() -> Result<(), ScssCliError> {
    run(ScssCli::parse())
}

fn run(cli: ScssCli) -> Result<(), ScssCliError> {
    let config = Config::load(&cli.manifest_dir)?;
    let files = get_hashed_scss(&config)?;

    let mut scss_bundle = String::new();
    let mut first = true;
    for (path, content) in files {
        if first {
            first = false;
        } else {
            scss_bundle.push('\n');
        }
        if cfg!(debug_assertions) {
            scss_bundle.push_str(&format!("/* {} */\n", path.to_string_lossy()));
        }
        scss_bundle.push_str(content.trim());
        scss_bundle.push('\n');
    }
    let output_file = cli.output_file.as_ref().unwrap_or(&config.output_file);
    let () = std::fs::create_dir_all(
        output_file
            .parent()
            .ok_or_else(|| ScssCliError::GetOutputFileFolder(output_file.to_owned()))?,
    )
    .map_err(|error| ScssCliError::CreateOutputFileFolder(output_file.to_owned(), error))?;
    std::fs::write(output_file, scss_bundle)
        .map_err(|error| ScssCliError::WriteFileError(config.output_file, error))
}

fn get_hashed_scss(config: &Config) -> Result<Vec<(PathBuf, String)>, ScssCliError> {
    let mut hashed_scss_files = Vec::new();
    for folder in &config.folders {
        for (entry, meta) in WalkDir::new(folder)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|entry| entry.metadata().ok().map(|meta| (entry, meta)))
        {
            if meta.is_file() {
                let path_str = entry.path().to_string_lossy();
                if config.extensions.iter().any(|ext| path_str.ends_with(ext)) {
                    let file_content = std::fs::read_to_string(entry.path()).map_err(|error| {
                        ScssCliError::ReadFileError(entry.path().to_owned(), error)
                    })?;
                    let hasher = ClassNameHasher::new(&file_content);
                    hashed_scss_files.push((
                        entry.path().to_owned(),
                        rewrite_classes(&file_content, |class| hasher.hash(class))?,
                    ));
                }
            }
        }
    }
    hashed_scss_files.sort_by(|(path1, _), (path2, _)| path1.cmp(path2));
    Ok(hashed_scss_files)
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use tempfile::tempdir;

    #[test]
    fn test() {
        let temp_dir = tempdir().unwrap();
        let temp_dir = temp_dir.path();
        let source_manifest_dir: PathBuf =
            format!("{}/test_data/crate", env!("CARGO_MANIFEST_DIR")).into();

        copy_dir_contents(&source_manifest_dir, temp_dir);

        let cli = super::ScssCli {
            manifest_dir: temp_dir.to_owned(),
            output_file: None,
        };
        super::run(cli).unwrap();
        let output =
            std::fs::read_to_string(temp_dir.join("target/css/terrazzo-terminal.scss")).unwrap();
        assert_eq!(
            r#"
/* $TEMP_DIR/src/client/client.scss */
div>.HnhCUtD9>.HnhCZxyk {
    font-family: "client";
}

/* $TEMP_DIR/src/root.scss */
div>.1JR7UtD9 {
    font-family: "root";
}
"#
            .trim(),
            output
                .replace(temp_dir.to_string_lossy().as_ref(), "$TEMP_DIR")
                .trim()
        );
    }

    fn copy_dir_contents(source: &Path, destination: &Path) {
        for entry in std::fs::read_dir(source).unwrap_or_else(|error| {
            panic!(
                "Failed to read {source:?} from {:?}: {error}",
                std::env::current_dir()
            )
        }) {
            let entry = entry.unwrap();
            let source_path = entry.path();
            let destination_path = destination.join(entry.file_name());

            if entry.file_type().unwrap().is_dir() {
                std::fs::create_dir_all(&destination_path).unwrap();
                copy_dir_contents(&source_path, &destination_path);
            } else {
                std::fs::copy(&source_path, &destination_path).unwrap();
            }
        }
    }
}
