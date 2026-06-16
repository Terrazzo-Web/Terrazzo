use std::path::Path;

use siphasher::sip::SipHasher24;

const FILE_HASH_LEN: usize = 4;
const CLASS_HASH_LEN: usize = 4;

pub struct ClassNameHasher {
    file_hash: String,
    debug: bool,
}

impl ClassNameHasher {
    pub fn new(file_path: &Path, file_content: &str, debug: bool) -> Self {
        let mut file_hash = if debug {
            let file_path = file_path
                .parent()
                .and_then(|parent| {
                    parent
                        .ancestors()
                        .find(|directory| directory.join("Cargo.toml").is_file())
                })
                .and_then(|crate_directory| file_path.strip_prefix(crate_directory).ok())
                .unwrap_or(file_path);
            SipHasher24::new().hash(file_path.to_string_lossy().as_bytes())
        } else {
            SipHasher24::new().hash(file_content.as_bytes())
        };
        let file_hash = loop {
            let h = hash_to_string(file_hash, FILE_HASH_LEN);
            if h.starts_with(|c: char| c.is_ascii_alphabetic()) {
                break h;
            }
            file_hash += 1;
        };
        Self { file_hash, debug }
    }

    pub fn hash(&self, class: &str) -> String {
        let class_hash = SipHasher24::new().hash(class.as_bytes());
        let hash = self.file_hash.clone() + &hash_to_string(class_hash, CLASS_HASH_LEN);
        if self.debug {
            format!("{class}-{hash}")
        } else {
            hash
        }
    }
}

fn hash_to_string(mut hash: u64, len: usize) -> String {
    let mut buffer = String::with_capacity(len);
    for _ in 0..len {
        buffer.push(to_char((hash % 62) as u8) as char);
        hash /= 62;
    }
    return buffer;
}

fn to_char(mut modulo: u8) -> u8 {
    if modulo < 26 {
        return b'A' + modulo;
    }
    modulo -= 26;
    if modulo < 26 {
        return b'a' + modulo;
    }
    modulo -= 26;
    return b'0' + modulo;
}

#[cfg(test)]
mod tests {

    #[test]
    fn hash() {
        let hasher = super::ClassNameHasher::new(
            "/style.scss".as_ref(),
            ".hello .world { font-weight: bold; }",
            false,
        );
        assert_eq!("EbZH3bWc", hasher.hash("hello"));
        assert_eq!("EbZHCgTI", hasher.hash("world"));
    }

    #[test]
    fn hash_debug() {
        let hasher = super::ClassNameHasher::new(
            "/style.scss".as_ref(),
            ".hello .world { font-weight: bold; }",
            true,
        );
        assert_eq!("hello-EbZH3bWc", hasher.hash("hello"));
        assert_eq!("world-EbZHCgTI", hasher.hash("world"));
    }
}
