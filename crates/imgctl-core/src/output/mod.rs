use std::io::Write;

use serde::Serialize;

use crate::error::Result;

pub mod json;
pub mod tsv;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Tsv,
    Json,
    Quiet,
}

impl OutputFormat {
    pub fn write<W: Write, T: Serialize>(self, w: &mut W, value: &T) -> Result<()> {
        match self {
            Self::Tsv => tsv::to_writer(w, value),
            Self::Json => json::to_writer(w, value),
            Self::Quiet => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Stub {
        a: u32,
    }

    #[test]
    fn quiet_emits_nothing() {
        let mut buf = Vec::new();
        OutputFormat::Quiet.write(&mut buf, &Stub { a: 1 }).unwrap();
        assert!(buf.is_empty());
    }

    #[test]
    fn json_emits_json_with_newline() {
        let mut buf = Vec::new();
        OutputFormat::Json.write(&mut buf, &Stub { a: 1 }).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "{\"a\":1}\n");
    }

    #[test]
    fn tsv_emits_kv() {
        let mut buf = Vec::new();
        OutputFormat::Tsv.write(&mut buf, &Stub { a: 1 }).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert_eq!(s, "a\t1\n");
    }
}
