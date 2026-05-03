fn main() {
    println!("cargo:rerun-if-changed=vendor/synctex/synctex_parser.c");
    println!("cargo:rerun-if-changed=vendor/synctex/synctex_parser.h");
    println!("cargo:rerun-if-changed=vendor/synctex/synctex_parser_advanced.h");
    println!("cargo:rerun-if-changed=vendor/synctex/synctex_parser_utils.c");
    println!("cargo:rerun-if-changed=vendor/synctex/synctex_parser_utils.h");
    println!("cargo:rerun-if-changed=vendor/synctex/synctex_version.h");

    cc::Build::new()
        .include("vendor/synctex")
        .file("vendor/synctex/synctex_parser.c")
        .file("vendor/synctex/synctex_parser_utils.c")
        .warnings(false)
        .compile("synctex");

    println!("cargo:rustc-link-lib=z");
}
