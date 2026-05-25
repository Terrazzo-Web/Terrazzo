pub fn serialize_line<T: serde::Serialize>(value: &T) -> Result<String, serde_json::Error> {
    serde_json::to_string(value).map(|json| json + "\n")
}
