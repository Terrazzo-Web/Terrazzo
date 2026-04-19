use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;

pub fn serialize_line<T: Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value).map(|json| json + "\n")
}

#[allow(dead_code)]
pub struct NdjsonBuffer<T> {
    pending: String,
    _phantom: PhantomData<fn() -> T>,
}

impl<T> Default for NdjsonBuffer<T> {
    fn default() -> Self {
        Self {
            pending: String::new(),
            _phantom: PhantomData,
        }
    }
}

impl<T: DeserializeOwned> NdjsonBuffer<T> {
    #[allow(dead_code)]
    pub fn push_chunk(&mut self, chunk: &str) -> Vec<Result<T, serde_json::Error>> {
        self.pending.push_str(chunk);

        let mut lines = vec![];
        while let Some(newline) = self.pending.find('\n') {
            let line = self.pending[..newline].to_owned();
            self.pending.drain(..=newline);
            if line.is_empty() {
                continue;
            }
            lines.push(serde_json::from_str::<T>(&line));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::NdjsonBuffer;
    use super::serialize_line;

    #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct TestItem {
        id: u64,
        name: String,
    }

    #[test]
    fn serializes_as_ndjson_line() {
        let line = serialize_line(&TestItem {
            id: 7,
            name: "hello".to_owned(),
        })
        .expect("serialize");

        assert_eq!(line, "{\"id\":7,\"name\":\"hello\"}\n");
    }

    #[test]
    fn splits_lines_and_parses_ndjson_chunks() {
        let item1 = serialize_line(&TestItem {
            id: 1,
            name: "first".to_owned(),
        })
        .expect("item1");
        let item2 = serialize_line(&TestItem {
            id: 2,
            name: "second".to_owned(),
        })
        .expect("item2");

        let mut parser = NdjsonBuffer::<TestItem>::default();

        let first = parser.push_chunk(&(item1.clone() + &item2[..8]));
        assert_eq!(first.len(), 1);
        assert_eq!(
            first[0].as_ref().expect("parsed"),
            &TestItem {
                id: 1,
                name: "first".to_owned()
            }
        );

        let second = parser.push_chunk(&item2[8..]);
        assert_eq!(second.len(), 1);
        assert_eq!(
            second.into_iter().next().expect("second").expect("parsed"),
            TestItem {
                id: 2,
                name: "second".to_owned()
            }
        );
    }
}
