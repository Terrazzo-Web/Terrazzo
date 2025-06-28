//! Server-side assets

use std::collections::HashMap;
use std::ffi::OsStr;
use std::future::ready;
use std::path::Path;
use std::path::PathBuf;
use std::sync::RwLock;

use axum::body::Body;
use axum::body::Bytes;
use axum::response::Response;
use http::HeaderValue;
use http::header;
use include_directory::Dir;
use tracing::debug;
use tracing::warn;

static ASSETS: RwLock<Option<HashMap<String, Asset>>> = RwLock::new(None);

/// Server-side asset.
#[must_use]
pub struct AssetBuilder {
    pub asset_name: String,

    mime: Option<HeaderValue>,

    #[cfg(debug_assertions)]
    full_path: PathBuf,

    #[cfg(not(debug_assertions))]
    content: &'static [u8],
}

impl AssetBuilder {
    /// Create a new asset with a static content.
    pub fn new(
        cargo_manifest_dir: &'static str,
        full_path: impl AsRef<Path>,
        content: &'static [u8],
    ) -> Self {
        let full_path = full_path.as_ref().to_owned();
        let asset_name = full_path.file_name().unwrap();
        let asset_name = asset_name.to_str().unwrap().to_owned();

        #[cfg(not(debug_assertions))]
        return Self {
            asset_name,
            mime: None,
            content,
        };

        #[cfg(debug_assertions)]
        {
            let _ = content;
            fn is_src_dir(leg: &OsStr) -> bool {
                leg.to_string_lossy().as_ref() == "src"
            }
            let full_path = if full_path.iter().any(is_src_dir) {
                let relative_path: PathBuf = full_path
                    .iter()
                    .skip_while(|leg| !is_src_dir(leg))
                    .collect();
                let in_src_path = Path::new(cargo_manifest_dir).join(relative_path);
                dbg!(&in_src_path);
                if in_src_path.exists() {
                    in_src_path
                } else {
                    full_path
                }
            } else {
                full_path
            };
            return Self {
                asset_name,
                full_path,
                mime: None,
            };
        }
    }

    /// Tweaks the name of the asset.
    pub fn asset_name(self, asset_name: impl Into<String>) -> Self {
        Self {
            asset_name: asset_name.into(),
            ..self
        }
    }

    /// Tweaks the file extension of the asset file.
    pub fn extension(self, extension: impl AsRef<OsStr>) -> Self {
        Self {
            asset_name: Path::new(&self.asset_name)
                .with_extension(extension)
                .to_str()
                .unwrap()
                .to_owned(),
            ..self
        }
    }

    /// Tweaks the mime type of the asset.
    /// This affects the "Content-Type" header.
    pub fn mime<M>(self, mime: M) -> Self
    where
        HeaderValue: TryFrom<M>,
        <HeaderValue as TryFrom<M>>::Error: Into<http::Error>,
    {
        let mime = Some(mime.try_into().map_err(Into::into).unwrap());
        Self { mime, ..self }
    }

    /// Records the asset in a static table.
    pub fn install(self) {
        #[cfg(debug_assertions)]
        println!("Installing {:?} => {:?}", self.asset_name, self.full_path);
        let mime = if let Some(mime) = self.mime {
            mime
        } else {
            mime_guess::from_path(&self.asset_name)
                .first_raw()
                .map(HeaderValue::from_static)
                .unwrap_or_else(|| {
                    HeaderValue::from_str(mime::APPLICATION_OCTET_STREAM.as_ref()).unwrap()
                })
        };

        #[cfg(not(debug_assertions))]
        add(
            self.asset_name,
            Asset {
                mime,
                content: self.content,
            },
        );

        #[cfg(debug_assertions)]
        add(
            self.asset_name,
            Asset {
                mime,
                full_path: self.full_path,
            },
        );
    }
}

/// Declares a file as a static asset.
///
/// The content of the file is compiled into the server binary using the [include_bytes] macro.
#[macro_export]
macro_rules! declare_asset {
    ($file:expr $(,)?) => {
        $crate::static_assets::AssetBuilder::new(
            env!("CARGO_MANIFEST_DIR"),
            concat!(env!("CARGO_MANIFEST_DIR"), $file),
            include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), $file)),
        )
    };
}

#[cfg(not(feature = "rustdoc"))]
#[macro_export]
macro_rules! declare_scss_asset {
    ($file:expr $(,)?) => {
        $crate::static_assets::AssetBuilder::new(
            env!("CARGO_MANIFEST_DIR"),
            concat!(env!("CARGO_MANIFEST_DIR"), "/", $file),
            $crate::static_assets::__macro_support::include_scss!($file).as_bytes(),
        )
        .mime($crate::mime::TEXT_CSS_UTF_8.as_ref())
        .extension("css")
    };
}

/// Declares a scss file as a static asset.
///
/// The content of the file is compiled from SCSS into CSS and included in the server binary.
#[cfg(feature = "rustdoc")]
#[macro_export]
macro_rules! declare_scss_asset {
    ($file:expr $(,)?) => {
        $crate::static_assets::AssetBuilder::new(
            env!("CARGO_MANIFEST_DIR"),
            $file,
            $file.as_bytes(),
        )
        .mime($crate::mime::TEXT_CSS_UTF_8.as_ref())
        .extension("css")
    };
}

#[doc(hidden)]
pub mod __macro_support {
    pub use ::include_directory;
    pub use ::rsass_macros::include_scss;
}

struct Asset {
    mime: HeaderValue,

    #[cfg(debug_assertions)]
    full_path: PathBuf,

    #[cfg(not(debug_assertions))]
    content: &'static [u8],
}

fn add(name: String, value: Asset) {
    let mut assets = ASSETS.write().unwrap();
    if let Some(assets) = &mut *assets {
        let old = assets.insert(name.clone(), value);
        assert!(old.is_none(), "Duplicate asset '{name}'");
        return;
    }

    let mut map = HashMap::new();
    map.insert(name, value);
    *assets = Some(map);
}

/// Axum handler that serves all the registered static assets.
pub fn get(path: &str) -> std::future::Ready<Response<Body>> {
    debug!("Getting {path}");
    let assets = ASSETS.read().expect(path);
    let assets = &*assets;
    let Some(asset) = assets.as_ref().and_then(|assets| assets.get(path)) else {
        warn!("Not found: {path}");
        return ready(Response::builder().status(404).body(Body::empty()).unwrap());
    };

    #[cfg(not(debug_assertions))]
    {
        return ready(
            Response::builder()
                .header(header::CONTENT_TYPE, asset.mime.clone())
                .header(header::CONTENT_LENGTH, asset.content.len().to_string())
                .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                .body(Body::from(Bytes::from_static(asset.content)))
                .expect(path),
        );
    }

    #[cfg(debug_assertions)]
    {
        let content = get_asset_content(path, asset);
        return ready(
            Response::builder()
                .header(header::CONTENT_TYPE, asset.mime.clone())
                .header(header::CONTENT_LENGTH, content.len().to_string())
                .body(Body::from(Bytes::from(content)))
                .expect(path),
        );
    }
}

#[cfg(debug_assertions)]
fn get_asset_content(path: &str, asset: &Asset) -> Vec<u8> {
    assert!(cfg!(feature = "debug"));
    #[cfg(feature = "debug")]
    {
        let asset_extension = || {
            asset
                .full_path
                .extension()
                .unwrap_or_default()
                .to_ascii_lowercase()
        };
        let path_extension = || {
            Path::new(path)
                .extension()
                .unwrap_or_default()
                .to_ascii_lowercase()
        };
        if asset_extension() == "scss" && path_extension() == "css" {
            use rsass::output::Format;
            use rsass::output::Style;
            return rsass::compile_scss_path(
                &asset.full_path,
                Format {
                    style: Style::Expanded,
                    precision: 10,
                },
            )
            .unwrap();
        }
    }
    return std::fs::read(&asset.full_path).unwrap_or_else(|_| {
        panic!(
            "path:{path} full_path:{}",
            asset.full_path.to_string_lossy()
        )
    });
}

/// Macro to load a folder of static assets.
///
/// See [install_dir].
#[macro_export]
macro_rules! declare_assets_dir {
    ($prefix:literal, $dir:tt) => {{
        use $crate::static_assets::__macro_support::include_directory;
        static DIR: include_directory::Dir<'_> = include_directory::include_directory!($dir);
        let root = $crate::static_assets::resolve_root($dir);
        $crate::static_assets::install_dir(env!("CARGO_MANIFEST_DIR"), $prefix, &root, &DIR);
    }};
}

/// Loads all the files in a folder (recursively) into the server binary as static assets.
pub fn install_dir(
    cargo_manifest_dir: &'static str,
    prefix: &str,
    root: &Path,
    dir: &Dir<'static>,
) {
    for entry in dir.entries() {
        if let Some(dir) = entry.as_dir() {
            let _ = root; // only used in debug mode.
            install_dir(cargo_manifest_dir, prefix, root, dir);
        }
        if let Some(file) = entry.as_file() {
            let asset_name = Path::new(prefix).join(entry.path());
            let asset_name = asset_name.as_os_str().to_str().unwrap();

            #[cfg(not(debug_assertions))]
            let full_path = file.path();

            #[cfg(debug_assertions)]
            let full_path = root.join(file.path());

            AssetBuilder::new(cargo_manifest_dir, full_path, file.contents())
                .asset_name(asset_name)
                .install();
        }
    }
}

#[cfg(not(debug_assertions))]
pub fn resolve_root(_: impl AsRef<Path>) -> PathBuf {
    PathBuf::default()
}

#[cfg(debug_assertions)]
pub fn resolve_root(root: impl AsRef<Path>) -> PathBuf {
    let mut result = PathBuf::new();
    for leg in root.as_ref().iter() {
        if leg.as_encoded_bytes().starts_with(b"$") {
            let leg = std::env::var(&leg.to_string_lossy().as_ref()[1..]);
            result.push(leg.unwrap());
        } else {
            result.push(leg);
        }
    }
    return result;
}
