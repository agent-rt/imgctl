use std::io::Write;

use serde::Serialize;

use crate::error::{Error, Result};

pub fn to_writer<W: Write, T: Serialize>(w: &mut W, value: &T) -> Result<()> {
    serde_json::to_writer(&mut *w, value)
        .map_err(|e| Error::Serialization(e.to_string()))?;
    w.write_all(b"\n")?;
    Ok(())
}
