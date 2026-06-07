use std::ops::Deref;
use std::path::Path;
use std::path::PathBuf;

use crate::text_editor::ui::ROOT_BASE_PATH;
use crate::utils::more_path::MorePathRef as _;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
/* serde */
#[derive(serde::Serialize, serde::Deserialize)]
pub struct FilePath<BASE, FILE = BASE> {
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "b"))]
    pub base: BASE,
    #[cfg_attr(not(feature = "diagnostics"), serde(rename = "f"))]
    pub file: FILE,
}

impl<B: AsRef<Path>, F: AsRef<Path>> FilePath<B, F> {
    pub fn full_path(&self) -> PathBuf {
        let base = self.base.as_ref();
        let file = self.file.as_ref().make_relative();
        if base.is_absolute() {
            base.join(file)
        } else {
            ROOT_BASE_PATH.join(base).join(file)
        }
    }

    pub fn with_base_path<R>(&self, f: impl FnOnce(&Path) -> R) -> R {
        let base = self.base.as_ref();
        if base.is_absolute() {
            f(base)
        } else {
            f(&ROOT_BASE_PATH.join(base))
        }
    }
}

impl<B, F> FilePath<B, F> {
    pub fn as_ref(&self) -> FilePath<&B, &F> where {
        FilePath {
            base: &self.base,
            file: &self.file,
        }
    }
}

impl<B: Deref, F: Deref> FilePath<B, F> {
    pub fn as_deref(&self) -> FilePath<&B::Target, &F::Target> where {
        FilePath {
            base: &self.base,
            file: &self.file,
        }
    }
}

impl<T> FilePath<T> {
    pub fn map<U>(self, f: impl Fn(T) -> U) -> FilePath<U> {
        self.map2(&f, &f)
    }
}

impl<B, F> FilePath<B, F> {
    pub fn map2<BB, FF>(
        self,
        b: impl FnOnce(B) -> BB,
        f: impl FnOnce(F) -> FF,
    ) -> FilePath<BB, FF> {
        FilePath {
            base: b(self.base),
            file: f(self.file),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::FilePath;
    use crate::utils::more_path::MorePath;

    #[test]
    fn full_path() {
        let actual = FilePath {
            base: "/",
            file: "/",
        }
        .full_path();
        assert_eq!(Path::new("/"), actual);
        assert_eq!("/", actual.to_owned_string());

        let actual = FilePath {
            base: "/a",
            file: "/b/",
        }
        .full_path();
        assert_eq!(Path::new("/a/b"), actual);
        assert_eq!("/a/b", actual.to_owned_string());

        let actual = FilePath {
            base: "a",
            file: "/b/",
        }
        .full_path();
        assert_eq!(Path::new("/a/b"), actual);
        assert_eq!("/a/b", actual.to_owned_string());

        let actual = FilePath {
            base: "a",
            file: "b",
        }
        .full_path();
        assert_eq!(Path::new("/a/b"), actual);
        assert_eq!("/a/b", actual.to_owned_string());
    }
}
