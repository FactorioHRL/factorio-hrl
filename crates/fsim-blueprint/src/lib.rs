use std::io::{Read, Write};
use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use serde_json::Value;

pub mod model;
pub mod raw;

pub use raw::Root;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to read file: {0}")]
    Io(#[from] std::io::Error),
    #[error("empty blueprint string")]
    Empty,
    #[error("unsupported format version '{0}' (expected '0')")]
    UnsupportedVersion(char),
    #[error("invalid base64: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("zlib decompression failed: {0}")]
    Zlib(std::io::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

pub fn load(path: impl AsRef<Path>) -> Result<Root> {
    decode(&std::fs::read_to_string(path)?)
}

pub fn decode(s: &str) -> Result<Root> {
    let json = decode_json(s)?;
    Ok(serde_json::from_value(json)?)
}

pub fn decode_json(s: &str) -> Result<Value> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let mut chars = s.chars();
    match chars.next() {
        None => return Err(Error::Empty),
        Some('0') => {}
        Some(c) => return Err(Error::UnsupportedVersion(c)),
    }
    let compressed = STANDARD.decode(chars.as_str())?;
    let mut json = Vec::new();
    ZlibDecoder::new(compressed.as_slice())
        .read_to_end(&mut json)
        .map_err(Error::Zlib)?;
    Ok(serde_json::from_slice(&json)?)
}

pub fn encode(root: &Root) -> Result<String> {
    let json = serde_json::to_vec(root)?;
    let mut z = ZlibEncoder::new(Vec::new(), Compression::best());
    z.write_all(&json).map_err(Error::Zlib)?;
    let compressed = z.finish().map_err(Error::Zlib)?;
    Ok(format!("0{}", STANDARD.encode(compressed)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/sample-blueprints/example_1"
    );

    #[test]
    fn loads_sample() {
        let root = load(SAMPLE).unwrap();
        let count: usize = root.blueprints().map(|b| b.entities.len()).sum();
        assert!(count > 0);
    }

    fn normalize(v: Value) -> Value {
        match v {
            Value::Number(n) => {
                serde_json::Number::from_f64(n.as_f64().unwrap()).map_or(Value::Null, Value::Number)
            }
            Value::Array(a) => Value::Array(a.into_iter().map(normalize).collect()),
            Value::Object(o) => {
                Value::Object(o.into_iter().map(|(k, v)| (k, normalize(v))).collect())
            }
            other => other,
        }
    }

    #[test]
    fn roundtrip_is_semantically_equal() {
        let original = std::fs::read_to_string(SAMPLE).unwrap();
        let before = normalize(decode_json(&original).unwrap());

        let root = decode(&original).unwrap();
        let encoded = encode(&root).unwrap();

        assert_eq!(normalize(decode_json(&encoded).unwrap()), before);
        assert_eq!(decode(&encoded).unwrap(), root);
    }

    #[test]
    fn ignores_whitespace() {
        let original = std::fs::read_to_string(SAMPLE).unwrap();
        let split = original
            .trim()
            .as_bytes()
            .chunks(60)
            .map(|c| std::str::from_utf8(c).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(decode(&split).unwrap(), decode(&original).unwrap());
    }

    #[test]
    fn book_roundtrip() {
        let bp = load(SAMPLE).unwrap();
        let book = Root::BlueprintBook(raw::BlueprintBook {
            blueprints: vec![raw::BookEntry {
                index: 0,
                content: bp,
            }],
            active_index: Some(0),
            label: Some("book".into()),
            extra: serde_json::Map::new(),
        });
        let decoded = decode(&encode(&book).unwrap()).unwrap();
        assert_eq!(decoded, book);
        assert_eq!(decoded.blueprints().count(), 1);
    }

    #[test]
    fn bad_input_returns_errors() {
        assert!(matches!(decode(""), Err(Error::Empty)));
        assert!(matches!(
            decode("1abc"),
            Err(Error::UnsupportedVersion('1'))
        ));
        assert!(matches!(decode("0!!!"), Err(Error::Base64(_))));
        assert!(matches!(decode("0AAAA"), Err(Error::Zlib(_))));
    }
}
