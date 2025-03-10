#![doc = include_str!("../README.md")]

use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;

use nameth::NamedEnumValues as _;
use nameth::NamedType as _;
use nameth::nameth;

/// Options for [build].
pub struct BuildOptions<'t> {
    /// Where the client code is located. Usually this is
    /// ```
    /// std::env::var("CARGO_MANIFEST_DIR")
    /// # ;
    /// ```
    pub client_dir: PathBuf,

    /// Where the server code is located. Usually this is also
    /// ```
    /// std::env::var("CARGO_MANIFEST_DIR")
    /// # ;
    /// ```
    pub server_dir: PathBuf,

    /// A list of extra compile options.
    ///
    /// For example, to compile the client code with the `"client"` and `"max_level_info"` features enabled,
    /// add `["--features", "client,max_level_info"]` to `wasm_pack_options`.
    pub wasm_pack_options: &'t [&'t str],
}

/// A `cargo` build script helper, to compile the client code to WASM and copy assets to the target folder.
///
/// Uses [wasm-pack](https://rustwasm.github.io/docs/wasm-pack/quickstart.html) to build the WASM assembly.
pub fn build(options: BuildOptions) -> Result<(), BuildError> {
    // https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargo-warning
    // for (key, value) in std::env::vars() {
    //     println!("cargo::warning={key} = {value}");
    // }

    let BuildOptions {
        client_dir,
        server_dir,
        wasm_pack_options,
    } = options;

    // .../client/src
    // https://doc.rust-lang.org/cargo/reference/build-scripts.html#rerun-if-changed
    let client_src_dir = client_dir
        .join("src")
        .to_str()
        .ok_or(BuildErrorInner::InvalidClientSrcDir)?
        .to_owned();
    println!("cargo::rerun-if-changed={client_src_dir}");

    // .../client/pkg
    let client_pkg_dir = client_dir.join("pkg");

    // rm -rf .../client/pkg
    rm(&client_pkg_dir, BuildErrorInner::RmClientPkgError)?;

    // cd .../client
    // wasm-pack build --target web
    let mut wasm_pack = std::process::Command::new("wasm-pack");
    wasm_pack
        .args(["build", "--target", "web"])
        .args(wasm_pack_options)
        .args(["--target-dir", "target/wasm"])
        .current_dir(&client_dir);
    for (key, value) in std::env::vars() {
        if !key.starts_with("CARGO_") && key != "DEBUG" && key != "OPT_LEVEL" && key != "PROFILE" {
            wasm_pack.env(key, value);
        }
    }
    // for (key, value) in wasm_pack.get_envs() {
    //     println! { "cargo::warning={key} = {value}", key = key.to_string_lossy(), value = value.unwrap().to_string_lossy() };
    // }
    let () = wasm_pack
        .status()
        .map_err(|_| BuildErrorInner::WasmPackError)?
        .success()
        .then_some(())
        .ok_or(BuildErrorInner::WasmPackError)?;

    // .../server/assets/wasm
    let assets_dir = server_dir.join("assets");
    let assets_wasm_dir = assets_dir.join("wasm");

    // rm -rf .../server/assets/wasm
    rm(&assets_wasm_dir, BuildErrorInner::RmServerAssetsWasmError)?;

    mv(
        &client_pkg_dir,
        &assets_wasm_dir,
        BuildErrorInner::MvWasmError,
    )?;

    let cargo_manifest_dir =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let debug_or_release = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let target_dir = cargo_manifest_dir.join("target").join(debug_or_release);
    let target_asset_dir = target_dir.join("assets");
    rm(&target_asset_dir, BuildErrorInner::RmTargetAssetsError)?;
    mkdir(&target_dir, BuildErrorInner::MkdirTargetAssetsError)?;
    cp(
        &assets_dir,
        &target_asset_dir,
        BuildErrorInner::CpTargetAssetsError,
    )?;

    Ok(())
}

fn cp<E>(from: &Path, to: &Path, error: E) -> Result<(), E> {
    let Ok(status) = std::process::Command::new("cp")
        .args([OsStr::new("-R"), from.as_os_str(), to.as_os_str()])
        .status()
    else {
        return Err(error);
    };
    status.success().then_some(()).ok_or(error)
}

fn mkdir<E>(path: &Path, error: E) -> Result<(), E> {
    let Ok(status) = std::process::Command::new("mkdir")
        .args([OsStr::new("-p"), path.as_os_str()])
        .status()
    else {
        return Err(error);
    };
    status.success().then_some(()).ok_or(error)
}

fn mv<E>(from: &Path, to: &Path, error: E) -> Result<(), E> {
    let Ok(status) = std::process::Command::new("mv")
        .args([from.as_os_str(), to.as_os_str()])
        .status()
    else {
        return Err(error);
    };
    status.success().then_some(()).ok_or(error)
}

fn rm<E>(path: &Path, error: E) -> Result<(), E> {
    let Ok(status) = std::process::Command::new("rm")
        .args([OsStr::new("-rf"), path.as_os_str()])
        .status()
    else {
        return Err(error);
    };
    status.success().then_some(()).ok_or(error)
}

/// Errors returned by [build].
#[nameth]
#[derive(thiserror::Error, Debug)]
#[error("[{t}] {0}", t = Self::type_name())]
pub struct BuildError(#[from] BuildErrorInner);

#[nameth]
#[derive(thiserror::Error, Debug)]
enum BuildErrorInner {
    #[error("[{n}] Client src dir is invalid UTF-8", n = self.name())]
    InvalidClientSrcDir,

    #[error("[{n}] Failed to eraze old client pkg folder", n = self.name())]
    RmClientPkgError,

    #[error("[{n}] Failed build the WASM", n = self.name())]
    WasmPackError,

    #[error("[{n}] Failed to eraze server assets wasm folder", n = self.name())]
    RmServerAssetsWasmError,

    #[error("[{n}] Failed to move the wasm to the server assets folder", n = self.name())]
    MvWasmError,

    #[error("[{n}] Failed to erase the target assets folder", n = self.name())]
    RmTargetAssetsError,

    #[error("[{n}] Failed to make the target assets folder", n = self.name())]
    MkdirTargetAssetsError,

    #[error("[{n}] Failed to copy to the target assets folder", n = self.name())]
    CpTargetAssetsError,
}

/// Invokes [stylance](https://crates.io/crates/stylance-cli) at compile time.
pub fn build_css() {
    let dir: PathBuf = std::env::var("CARGO_MANIFEST_DIR").unwrap().into();
    let status = std::process::Command::new("stylance")
        .current_dir(&dir)
        .arg(".")
        .status();
    assert!(status.unwrap().success());
}
