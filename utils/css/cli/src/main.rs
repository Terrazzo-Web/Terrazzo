use std::path::PathBuf;

use clap::Parser;
use nameth::NamedEnumValues as _;
use nameth::nameth;
use terrazzo_css_shared::CssError;
use terrazzo_css_shared::config::Config;
use terrazzo_css_shared::config::ConfigError;
use terrazzo_css_shared::hasher::ClassNameHasher;
use terrazzo_css_shared::rewrite_classes;
use walkdir::WalkDir;

#[derive(Parser)]
#[command(author, version, about, long_about = None, arg_required_else_help = true)]
struct Cli {
    /// The crate root dir, i.e the folder containing the crate's Cargo.toml manifest.
    #[arg(required = true)]
    manifest_dir: PathBuf,
}

#[nameth]
#[derive(thiserror::Error, Debug)]
enum CssCliError {
    #[error("[{n}] {0}", n = self.name())]
    ConfigError(#[from] ConfigError),

    #[error("[{n}] Failed to read '{0}': {1}", n = self.name())]
    ReadFileError(PathBuf, std::io::Error),

    #[error("[{n}] Failed to parse manifest: {0}", n = self.name())]
    CssError(#[from] CssError),

    #[error("[{n}] Failed to read '{0}': {1}", n = self.name())]
    WriteFileError(PathBuf, std::io::Error),
}

fn main() -> Result<(), CssCliError> {
    let cli = Cli::parse();
    let config = Config::load(&cli.manifest_dir)?;
    let files = get_hashed_css(&config)?;

    let mut output_file = String::new();
    for (path, content) in files {
        output_file.push_str(&format!("/* {} */\n", path.to_string_lossy()));
        output_file.push_str(content.trim());
        output_file.push('\n');
    }
    std::fs::write(&config.output_file, output_file)
        .map_err(|error| CssCliError::WriteFileError(config.output_file, error))
}

fn get_hashed_css(config: &Config) -> Result<Vec<(PathBuf, String)>, CssCliError> {
    let mut hashed_css_files = Vec::new();
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
                        CssCliError::ReadFileError(entry.path().to_owned(), error)
                    })?;
                    let hasher = ClassNameHasher::new(&file_content);
                    hashed_css_files.push((
                        entry.path().to_owned(),
                        rewrite_classes(&file_content, |class| hasher.hash(class))?,
                    ));
                }
            }
        }
    }
    Ok(hashed_css_files)
}
