use std::borrow::Cow;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::path::PathBuf;

pub trait MorePath {
    fn to_owned_string(self) -> String;
}

pub trait MorePathRef {
    fn make_relative(&self) -> &Path;
}

impl MorePath for &Path {
    fn to_owned_string(self) -> String {
        self.as_os_str().to_owned_string()
    }
}

impl MorePathRef for Path {
    fn make_relative(&self) -> &Path {
        self.strip_prefix("/").unwrap_or(self)
    }
}

impl MorePath for PathBuf {
    fn to_owned_string(self) -> String {
        self.into_os_string().to_owned_string()
    }
}

impl MorePathRef for PathBuf {
    fn make_relative(&self) -> &Path {
        self.as_path().make_relative()
    }
}

impl MorePath for &OsStr {
    fn to_owned_string(self) -> String {
        let cow: Cow<'_, str> = self.to_string_lossy();
        return cow.into_owned();
    }
}

impl MorePathRef for OsStr {
    fn make_relative(&self) -> &Path {
        Path::new(self).make_relative()
    }
}

impl MorePath for OsString {
    fn to_owned_string(self) -> String {
        match self.into_string() {
            Ok(string) => string,
            Err(os_string) => os_string.as_os_str().to_owned_string(),
        }
    }
}

impl MorePathRef for OsString {
    fn make_relative(&self) -> &Path {
        self.as_os_str().make_relative()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::MorePathRef;

    #[test]
    fn make_relative_when_relative() {
        assert_eq!(Path::new("a/b/c"), Path::new("a//b/c").make_relative());
    }

    #[test]
    fn make_relative_when_absolute() {
        assert_eq!(Path::new("a/b/c"), Path::new("/a/b/c").make_relative());
        assert_eq!(Path::new("a/b/c"), Path::new("//a/b/c").make_relative());
    }

    #[test]
    fn make_relative_when_root() {
        assert_eq!(Path::new(""), Path::new("/").make_relative());
    }
}
