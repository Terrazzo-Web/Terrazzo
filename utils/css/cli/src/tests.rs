use std::path::Path;
use std::path::PathBuf;

use tempfile::tempdir;

#[test]
fn test() {
    let temp_dir = tempdir().unwrap();
    let temp_dir = temp_dir.path();
    let source_manifest_dir: PathBuf = format!("{}/tests/crate", env!("CARGO_MANIFEST_DIR")).into();

    copy_dir_contents(&source_manifest_dir, temp_dir);

    let cli = super::Cli {
        manifest_dir: temp_dir.to_owned(),
    };
    super::run(cli).unwrap();
    let output =
        std::fs::read_to_string(temp_dir.join("target/css/terrazzo-terminal.scss")).unwrap();
    assert_eq!(
        r#"
/* $TEMP_DIR/src/root.scss */
div>.1JR7UtD9 {
    font-family: "root";
}
    
/* $TEMP_DIR/src/client/client.scss */
div>.HnhCUtD9>.HnhCZxyk {
    font-family: "client";
}
"#
        .trim(),
        output
            .replace(&temp_dir.to_string_lossy().as_ref(), "$TEMP_DIR")
            .trim()
    );
}

fn copy_dir_contents(source: &Path, destination: &Path) {
    for entry in std::fs::read_dir(source).unwrap() {
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
