use std::io::Write;

use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};

/// Serialize `value` as flat key-value TSV.
///
/// - Each leaf field emits `key<TAB>value\n`.
/// - Nested objects use dot-separated paths (`parent.child`).
/// - Arrays use bracketed indices (`parent[0].field`).
/// - `null` values are skipped (no line emitted).
/// - Tab, newline and backslash characters in string values are escaped to
///   `\t`, `\n`, `\\` so each record stays on a single line.
pub fn to_writer<W: Write, T: Serialize>(w: &mut W, value: &T) -> Result<()> {
    let v = serde_json::to_value(value).map_err(|e| Error::Serialization(e.to_string()))?;
    let mut buf = String::new();
    flatten("", &v, &mut buf);
    w.write_all(buf.as_bytes())?;
    Ok(())
}

fn flatten(prefix: &str, value: &Value, out: &mut String) {
    match value {
        Value::Null => {}
        Value::Bool(b) => emit(prefix, if *b { "true" } else { "false" }, out),
        Value::Number(n) => emit(prefix, &n.to_string(), out),
        Value::String(s) => emit(prefix, &escape(s), out),
        Value::Array(arr) => {
            for (i, item) in arr.iter().enumerate() {
                let np = format!("{prefix}[{i}]");
                flatten(&np, item, out);
            }
        }
        Value::Object(map) => {
            for (k, v) in map {
                let np = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&np, v, out);
            }
        }
    }
}

fn emit(key: &str, value: &str, out: &mut String) {
    out.push_str(key);
    out.push('\t');
    out.push_str(value);
    out.push('\n');
}

fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Tile {
        file: String,
        x: i32,
        y: i32,
    }

    #[derive(Serialize)]
    struct Doc {
        success: bool,
        tiles: Vec<Tile>,
    }

    fn render<T: Serialize>(value: &T) -> String {
        let mut buf = Vec::new();
        to_writer(&mut buf, value).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn nested_array_with_dot_and_bracket_paths() {
        let doc = Doc {
            success: true,
            tiles: vec![
                Tile { file: "a.png".into(), x: 0, y: 0 },
                Tile { file: "b.png".into(), x: 100, y: 0 },
            ],
        };
        let got = render(&doc);
        let expected = concat!(
            "success\ttrue\n",
            "tiles[0].file\ta.png\n",
            "tiles[0].x\t0\n",
            "tiles[0].y\t0\n",
            "tiles[1].file\tb.png\n",
            "tiles[1].x\t100\n",
            "tiles[1].y\t0\n",
        );
        assert_eq!(got, expected);
    }

    #[test]
    fn escapes_tab_newline_and_backslash() {
        #[derive(Serialize)]
        struct E {
            msg: String,
        }
        let e = E { msg: "line1\tcol\nline2\\path".into() };
        assert_eq!(render(&e), "msg\tline1\\tcol\\nline2\\\\path\n");
    }

    #[test]
    fn skips_null_fields() {
        #[derive(Serialize)]
        struct N {
            a: i32,
            #[serde(skip_serializing_if = "Option::is_none")]
            b: Option<i32>,
            c: Option<i32>,
        }
        let n = N { a: 1, b: None, c: None };
        // `b` is skipped by serde; `c` becomes Null and is skipped by the TSV emitter.
        assert_eq!(render(&n), "a\t1\n");
    }

    #[test]
    fn floats_format_compactly() {
        #[derive(Serialize)]
        struct F {
            r: f64,
        }
        assert_eq!(render(&F { r: 0.97 }), "r\t0.97\n");
    }

    #[test]
    fn response_error_payload_is_dotted() {
        use crate::error::Error;
        use crate::response::Response;
        let r = Response::<()>::from_error(&Error::NotFound("x".into()), 1);
        let got = render(&r);
        assert!(got.contains("success\tfalse\n"));
        assert!(got.contains("error.code\tNOT_FOUND\n"));
        assert!(got.contains("error.message\tnot found: x\n"));
    }
}
