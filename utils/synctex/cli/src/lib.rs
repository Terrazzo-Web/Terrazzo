use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use terrazzo_synctex::Node;
use terrazzo_synctex::Scanner;
use terrazzo_synctex::VisibleBox;

#[derive(Debug, Parser)]
#[command(author, version, about)]
struct Cli {
    /// PDF output file whose SyncTeX sidecar should be opened.
    #[arg(short = 'f', long = "pdf-file")]
    pdf_file: PathBuf,

    /// Directory containing generated SyncTeX/output files.
    #[arg(short = 'b', long = "build-directory")]
    build_directory: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
#[command(rename_all = "snake_case")]
enum Command {
    DisplayQuery(DisplayQuery),
    EditQuery(EditQuery),
    Coordinates,
    Sheet(Sheet),
    DisplayDebug,
}

#[derive(Debug, Args)]
struct DisplayQuery {
    /// Source line.
    #[arg(short = 'l', long)]
    line: i32,

    /// Source column.
    #[arg(short = 'c', long)]
    column: i32,

    /// Page hint, or 0 when no page is known.
    #[arg(short = 'p', long = "page-hint", default_value_t = 0)]
    page_hint: i32,

    /// Source file recorded in the SyncTeX input list.
    #[arg(short = 's', long)]
    source: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct EditQuery {
    /// Output page number.
    #[arg(short = 'p', long)]
    page: i32,

    /// Horizontal page coordinate.
    #[arg(short = 'x', long)]
    h: f32,

    /// Vertical page coordinate.
    #[arg(short = 'y', long)]
    v: f32,
}

#[derive(Debug, Args)]
struct Sheet {
    /// Output page number.
    #[arg(short = 'p', long)]
    page: i32,

    /// Show the sheet content node instead of the sheet node.
    #[arg(short = 'c', long)]
    content: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("{0}")]
    Clap(#[from] clap::Error),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Synctex(#[from] terrazzo_synctex::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn run<I, T, W>(args: I, mut output: W) -> Result<()>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    W: std::io::Write,
{
    let cli = Cli::try_parse_from(args)?;
    let mut scanner = Scanner::open(&cli.pdf_file, cli.build_directory.as_deref())?;

    match cli.command {
        Command::DisplayQuery(query) => {
            let source = resolve_source_file(&scanner, &cli.pdf_file, query.source)?;
            let results =
                scanner.display_query(&source, query.line, query.column, query.page_hint)?;
            write_nodes(&mut output, results, true)?;
        }
        Command::EditQuery(query) => {
            let results = scanner.edit_query(query.page, query.h, query.v)?;
            write_nodes(&mut output, results, true)?;
        }
        Command::Coordinates => {
            writeln!(
                output,
                "x_offset={} y_offset={} magnification={}",
                scanner.x_offset(),
                scanner.y_offset(),
                scanner.magnification()
            )?;
        }
        Command::Sheet(sheet) => {
            let node = if sheet.content {
                scanner.sheet_content(sheet.page)
            } else {
                scanner.sheet(sheet.page)
            };
            if let Some(node) = node {
                write_node(&mut output, node, true)?;
            } else {
                writeln!(output, "no node")?;
            }
        }
        Command::DisplayDebug => scanner.display_debug(),
    }

    Ok(())
}

fn default_source_file(pdf_file: &Path) -> PathBuf {
    pdf_file.with_extension("tex")
}

fn resolve_source_file(
    scanner: &Scanner,
    pdf_file: &Path,
    source: Option<PathBuf>,
) -> Result<PathBuf> {
    let source = source.unwrap_or_else(|| default_source_file(pdf_file));
    if scanner.tag_for_name(&source)? > 0 {
        return Ok(source);
    }

    let Some(file_name) = source.file_name() else {
        return Ok(source);
    };
    let mut input = scanner.input();
    while let Some(node) = input {
        if let Some(name) = node.name() {
            let name = PathBuf::from(name.to_string_lossy().into_owned());
            if name.file_name() == Some(file_name) {
                return Ok(name);
            }
        }
        input = node.sibling();
    }

    Ok(source)
}

fn write_nodes<'scanner, W, I>(
    output: &mut W,
    nodes: I,
    include_source: bool,
) -> std::io::Result<()>
where
    W: std::io::Write,
    I: IntoIterator<Item = Node<'scanner>>,
{
    for node in nodes {
        write_node(output, node, include_source)?;
    }
    Ok(())
}

fn write_node<W>(output: &mut W, node: Node<'_>, include_source: bool) -> std::io::Result<()>
where
    W: std::io::Write,
{
    write!(
        output,
        "page={} tag={} line={} column={} mean_line={}",
        node.page(),
        node.tag(),
        node.line(),
        node.column(),
        node.mean_line()
    )?;
    if include_source && let Some(name) = node.name() {
        write!(output, " source={}", name.to_string_lossy())?;
    }
    writeln!(output, " visible={}", format_visible(node.visible()))?;
    Ok(())
}

fn format_visible(visible: VisibleBox) -> String {
    format!(
        "h:{:.3},v:{:.3},width:{:.3},height:{:.3},depth:{:.3}",
        visible.h, visible.v, visible.width, visible.height, visible.depth
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::run;

    fn fixture(name: &str) -> PathBuf {
        fixture_dir().join(name)
    }

    fn fixture_dir() -> PathBuf {
        static TEST_FIXTURES: &str = "tests/fixtures/PlantUML";
        let result = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(TEST_FIXTURES);
        if result.exists() {
            return result;
        }

        return runfiles::find_runfiles_dir()
            .unwrap()
            .join(std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE"))
            .join(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
            .join(TEST_FIXTURES);
    }

    fn run_cli(args: &[&str]) -> String {
        let pdf = fixture("PlantUML.pdf");
        let mut full_args = vec![
            "terrazzo-synctex".to_owned(),
            "--pdf-file".to_owned(),
            pdf.display().to_string(),
        ];
        full_args.extend(args.iter().map(|arg| (*arg).to_owned()));

        let mut output = Vec::new();
        run(full_args, &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }

    #[test]
    fn coordinates_print_offsets_and_magnification() {
        let output = run_cli(&["coordinates"]);

        assert!(output.contains("x_offset=0"));
        assert!(output.contains("y_offset=0"));
        assert!(output.contains("magnification="));
    }

    #[test]
    fn display_query_prints_visible_boxes() {
        let source = fixture("PlantUML.tex");
        let output = run_cli(&[
            "display_query",
            "--line",
            "22",
            "--column",
            "1",
            "--page-hint",
            "1",
            "--source",
            &source.display().to_string(),
        ]);

        assert!(output.contains("visible=h:"));
        assert!(output.contains("page=1"));
    }

    #[test]
    fn edit_query_prints_source_names_and_visible_boxes() {
        let output = run_cli(&["edit_query", "--page", "1", "--h", "100", "--v", "100"]);

        assert!(output.contains("visible=h:"));
        assert!(output.contains("source="));
    }

    #[test]
    fn sheet_prints_page_node() {
        let output = run_cli(&["sheet", "--page", "1", "--content"]);

        assert!(output.contains("page=1"));
        assert!(output.contains("visible=h:"));
    }
}
