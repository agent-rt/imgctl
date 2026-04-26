use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::{Error, Result};

#[derive(Debug, Clone)]
pub enum InputSource {
    File(PathBuf),
    Stdio,
}

#[derive(Debug, Clone)]
pub enum OutputSink {
    File(PathBuf),
    Stdio,
}

impl InputSource {
    pub fn from_arg(s: &str) -> Self {
        if s == "-" {
            Self::Stdio
        } else {
            Self::File(PathBuf::from(s))
        }
    }

    pub fn read_all(&self) -> Result<Vec<u8>> {
        match self {
            Self::File(p) => std::fs::read(p).map_err(Error::from),
            Self::Stdio => {
                let mut buf = Vec::new();
                std::io::stdin().lock().read_to_end(&mut buf)?;
                Ok(buf)
            }
        }
    }
}

impl OutputSink {
    pub fn from_arg(s: &str) -> Self {
        if s == "-" {
            Self::Stdio
        } else {
            Self::File(PathBuf::from(s))
        }
    }

    pub fn is_stdio(&self) -> bool {
        matches!(self, Self::Stdio)
    }

    pub fn write_all(&self, bytes: &[u8]) -> Result<()> {
        match self {
            Self::File(p) => std::fs::write(p, bytes).map_err(Error::from),
            Self::Stdio => {
                let mut out = std::io::stdout().lock();
                out.write_all(bytes)?;
                Ok(())
            }
        }
    }
}

/// Dual-channel writer: data on the primary stream, metadata on the side channel.
///
/// Routing:
/// - `OutputSink::File` → data writes to the file, metadata to stdout
/// - `OutputSink::Stdio` → data writes to stdout, metadata to stderr (so the
///   binary payload and TSV/JSON metadata never collide on the same stream)
pub struct OutputChannels {
    pub data: Box<dyn Write>,
    pub meta: Box<dyn Write>,
}

impl OutputChannels {
    pub fn for_sink(sink: &OutputSink) -> Result<Self> {
        Ok(match sink {
            OutputSink::File(p) => Self {
                data: Box::new(std::fs::File::create(p)?),
                meta: Box::new(std::io::stdout()),
            },
            OutputSink::Stdio => Self {
                data: Box::new(std::io::stdout()),
                meta: Box::new(std::io::stderr()),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_temp(suffix: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!("imgctl-{}-{}-{suffix}", std::process::id(), nanos))
    }

    #[test]
    fn input_source_from_arg() {
        match InputSource::from_arg("-") {
            InputSource::Stdio => {}
            other => panic!("expected Stdio, got {other:?}"),
        }
        match InputSource::from_arg("foo.png") {
            InputSource::File(p) => assert_eq!(p, PathBuf::from("foo.png")),
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn output_sink_from_arg() {
        assert!(OutputSink::from_arg("-").is_stdio());
        assert!(!OutputSink::from_arg("out.png").is_stdio());
    }

    #[test]
    fn input_source_reads_file() {
        let path = unique_temp("input.bin");
        let payload: &[u8] = b"hello\x00world";
        std::fs::write(&path, payload).unwrap();

        let read = InputSource::File(path.clone()).read_all().unwrap();
        assert_eq!(read, payload);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn output_channels_file_sink_writes_to_file() {
        let path = unique_temp("output.bin");
        let sink = OutputSink::File(path.clone());
        let mut ch = OutputChannels::for_sink(&sink).unwrap();
        ch.data.write_all(b"binary").unwrap();
        ch.data.flush().unwrap();
        drop(ch);

        let read = std::fs::read(&path).unwrap();
        assert_eq!(read, b"binary");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn output_sink_write_all_to_file() {
        let path = unique_temp("write_all.bin");
        let sink = OutputSink::File(path.clone());
        sink.write_all(b"abc").unwrap();
        let read = std::fs::read(&path).unwrap();
        assert_eq!(read, b"abc");
        let _ = std::fs::remove_file(&path);
    }
}
