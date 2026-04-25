const FILE_HASH_LEN: usize = 4;
const CLASS_HASH_LEN: usize = 4;

pub struct ClassNameHasher {
    file_hash: String,
}

impl ClassNameHasher {
    pub fn new(file_content: &str) -> Self {
        let mut file_hash = siphasher::sip::SipHasher24::new().hash(file_content.as_bytes());
        let file_hash = loop {
            let h = hash_to_string(file_hash, FILE_HASH_LEN);
            if h.starts_with(|c| (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')) {
                break h;
            }
            file_hash += 1;
        };
        Self { file_hash }
    }

    pub fn hash(&self, class: &str) -> String {
        let class_hash = siphasher::sip::SipHasher24::new().hash(class.as_bytes());
        self.file_hash.clone() + &hash_to_string(class_hash, CLASS_HASH_LEN)
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
        let hasher = super::ClassNameHasher::new(".hello .world { font-weight: bold; }");
        assert_eq!("EbZH3bWc", hasher.hash("hello"));
        assert_eq!("EbZHCgTI", hasher.hash("world"));
    }
}
