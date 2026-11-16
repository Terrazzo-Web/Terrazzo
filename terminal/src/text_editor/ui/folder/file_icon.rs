//! File associations are resolved entirely in the browser.
use crate::assets::icons;

pub(super) fn file_icon(name: &str, is_dir: bool) -> icons::Icon {
    if is_dir {
        return icons::filetype_folder_base();
    }
    let name = name.to_ascii_lowercase();
    // Recognize common extensionless names and dotfiles before parsing extensions.
    match name.as_str() {
        "dockerfile" | "containerfile" => return icons::filetype_docker(),
        "makefile" | "gnumakefile" | "justfile" => return icons::filetype_console(),
        "license" | "licence" | "readme" | "changelog" | "authors" => {
            return icons::filetype_document();
        }
        ".gitignore" | ".gitattributes" | ".gitmodules" => return icons::filetype_git(),
        ".bashrc" | ".zshrc" | ".profile" => return icons::filetype_console(),
        ".env" | ".editorconfig" | ".npmrc" => return icons::filetype_settings(),
        _ => {}
    }
    if name.starts_with(".env.") {
        return icons::filetype_settings();
    }
    let extension = std::path::Path::new(&name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    match extension {
        "rs" => icons::filetype_rust(),
        "js" | "mjs" | "cjs" => icons::filetype_javascript(),
        "ts" | "mts" | "cts" => icons::filetype_typescript(),
        "jsx" | "tsx" => icons::filetype_react(),
        "py" | "pyi" | "pyw" | "ipynb" => icons::filetype_python(),
        "java" | "jar" | "class" => icons::filetype_java(),
        "c" | "h" => icons::filetype_c(),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => icons::filetype_cpp(),
        "cs" | "csx" => icons::filetype_csharp(),
        "go" => icons::filetype_go(),
        "rb" | "erb" | "gem" => icons::filetype_ruby(),
        "php" | "phtml" => icons::filetype_php(),
        "swift" => icons::filetype_swift(),
        "kt" | "kts" => icons::filetype_kotlin(),
        "scala" | "sc" => icons::filetype_scala(),
        "lua" => icons::filetype_lua(),
        "sh" | "bash" | "zsh" | "fish" | "bashrc" | "zshrc" => icons::filetype_console(),
        "ps1" | "psm1" | "psd1" => icons::filetype_powershell(),
        "html" | "htm" | "xhtml" => icons::filetype_html(),
        "css" => icons::filetype_css(),
        "sass" | "scss" => icons::filetype_sass(),
        "less" => icons::filetype_less(),
        "json" | "jsonc" | "json5" | "geojson" => icons::filetype_json(),
        "yaml" | "yml" => icons::filetype_yaml(),
        "toml" => icons::filetype_toml(),
        "xml" | "xsd" | "xsl" | "xslt" | "plist" | "csproj" => icons::filetype_xml(),
        "md" | "markdown" | "mdx" => icons::filetype_markdown(),
        "pdf" => icons::filetype_pdf(),
        "doc" | "docx" | "odt" | "rtf" => icons::filetype_word(),
        "csv" | "tsv" | "xls" | "xlsx" | "ods" => icons::filetype_table(),
        "ppt" | "pptx" | "odp" => icons::filetype_powerpoint(),
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "svg" | "ico" | "bmp" | "tif"
        | "tiff" | "heic" | "raw" | "psd" | "ai" | "eps" => icons::filetype_image(),
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" | "opus" | "aiff" => icons::filetype_audio(),
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" | "mpg" | "mpeg" => icons::filetype_video(),
        "zip" | "gz" | "bz2" | "xz" | "zst" | "tar" | "tgz" | "tbz2" | "txz" | "rar" | "7z"
        | "iso" => icons::filetype_zip(),
        "ttf" | "otf" | "woff" | "woff2" | "eot" => icons::filetype_font(),
        "sql" | "db" | "sqlite" | "sqlite3" | "mdb" => icons::filetype_database(),
        "dockerfile" => icons::filetype_docker(),
        "gitignore" | "gitattributes" | "gitmodules" => icons::filetype_git(),
        "lock" | "pem" | "key" | "crt" | "cer" | "p12" | "pfx" => icons::filetype_lock(),
        "ini" | "cfg" | "conf" | "config" | "env" | "properties" | "editorconfig" => {
            icons::filetype_settings()
        }
        "exe" | "dll" | "so" | "dylib" | "bin" | "wasm" | "deb" | "rpm" | "apk" | "appimage" => {
            icons::filetype_exe()
        }
        "txt" | "log" | "rst" | "tex" | "adoc" => icons::filetype_document(),
        _ => icons::filetype_document(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_file_types() {
        for (name, expected) in [
            ("main.RS", icons::filetype_rust()),
            ("index.d.ts", icons::filetype_typescript()),
            ("archive.tar.gz", icons::filetype_zip()),
            (".gitignore", icons::filetype_git()),
            ("Dockerfile", icons::filetype_docker()),
            (".env.local", icons::filetype_settings()),
            ("photo.JPEG", icons::filetype_image()),
            ("unknown.xyz", icons::filetype_document()),
            ("no-extension", icons::filetype_document()),
            (".unknown", icons::filetype_document()),
        ] {
            assert_eq!(file_icon(name, false), expected, "{name}");
        }
        assert_eq!(file_icon("source.rs", true), icons::filetype_folder_base());
        assert_eq!(file_icon("..", true), icons::filetype_folder_base());
    }
}
