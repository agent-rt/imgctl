use std::io::{Cursor, Write};

use image::DynamicImage;
use image::ImageEncoder;
use image::codecs::jpeg::JpegEncoder;

use imgctl_core::{Error, OutputSink, Result};

use crate::format::ImageFormat;

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct EncodedInfo {
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
}

/// Encode `img` to `fmt`, returning the encoded bytes.
///
/// `quality` is clamped to 1..=100 and only affects lossy formats (JPEG today).
pub fn encode_to_bytes(img: &DynamicImage, fmt: ImageFormat, quality: u8) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    let q = quality.clamp(1, 100);
    match fmt {
        ImageFormat::Jpeg => {
            // JPEG does not support alpha; convert to RGB up-front.
            let rgb = img.to_rgb8();
            let enc = JpegEncoder::new_with_quality(&mut buf, q);
            enc.write_image(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|e| Error::Image(e.to_string()))?;
        }
        _ => {
            let mut cursor = Cursor::new(&mut buf);
            img.write_to(&mut cursor, fmt.to_image())
                .map_err(|e| Error::Image(e.to_string()))?;
        }
    }
    Ok(buf)
}

/// Encode and stream into an arbitrary writer (used by the CLI's data channel).
pub fn write_to<W: Write>(
    img: &DynamicImage,
    fmt: ImageFormat,
    quality: u8,
    w: &mut W,
) -> Result<EncodedInfo> {
    let bytes = encode_to_bytes(img, fmt, quality)?;
    w.write_all(&bytes).map_err(Error::from)?;
    Ok(EncodedInfo {
        width: img.width(),
        height: img.height(),
        format: fmt.as_str(),
        size_bytes: bytes.len() as u64,
    })
}

/// Encode to an `OutputSink` (file or stdout).
pub fn write(
    img: &DynamicImage,
    fmt: ImageFormat,
    quality: u8,
    sink: &OutputSink,
) -> Result<EncodedInfo> {
    let bytes = encode_to_bytes(img, fmt, quality)?;
    sink.write_all(&bytes)?;
    Ok(EncodedInfo {
        width: img.width(),
        height: img.height(),
        format: fmt.as_str(),
        size_bytes: bytes.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    use image::{Rgba, RgbaImage};

    fn unique_temp(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "imgctl-encode-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn fixture_image() -> DynamicImage {
        let img = RgbaImage::from_fn(64, 64, |x, y| {
            let v = ((x * 7 + y * 11) % 255) as u8;
            Rgba([v, v.wrapping_mul(2), v.wrapping_mul(3), 255])
        });
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn encode_png_writes_valid_bytes() {
        let img = fixture_image();
        let bytes = encode_to_bytes(&img, ImageFormat::Png, 85).unwrap();
        // PNG magic
        assert_eq!(
            &bytes[0..8],
            &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]
        );
        let back = image::load_from_memory(&bytes).unwrap();
        assert_eq!(back.width(), 64);
        assert_eq!(back.height(), 64);
    }

    #[test]
    fn encode_jpeg_quality_monotonic() {
        let img = fixture_image();
        let low = encode_to_bytes(&img, ImageFormat::Jpeg, 10).unwrap();
        let high = encode_to_bytes(&img, ImageFormat::Jpeg, 95).unwrap();
        assert!(
            low.len() < high.len(),
            "low={} high={}",
            low.len(),
            high.len()
        );
        // JPEG SOI
        assert_eq!(&low[0..2], &[0xFF, 0xD8]);
    }

    #[test]
    fn encode_quality_clamped_to_valid_range() {
        let img = fixture_image();
        // quality=0 should clamp to 1 and not panic.
        let _ = encode_to_bytes(&img, ImageFormat::Jpeg, 0).unwrap();
        let _ = encode_to_bytes(&img, ImageFormat::Jpeg, 200).unwrap();
    }

    #[test]
    fn write_to_writer_returns_info() {
        let img = fixture_image();
        let mut buf = Vec::new();
        let info = write_to(&img, ImageFormat::Png, 85, &mut buf).unwrap();
        assert_eq!(info.width, 64);
        assert_eq!(info.height, 64);
        assert_eq!(info.format, "png");
        assert_eq!(info.size_bytes as usize, buf.len());
    }

    #[test]
    fn write_to_file_sink_matches_disk() {
        let img = fixture_image();
        let path = unique_temp("png");
        let info = write(&img, ImageFormat::Png, 85, &OutputSink::File(path.clone())).unwrap();
        let on_disk = std::fs::metadata(&path).unwrap().len();
        assert_eq!(info.size_bytes, on_disk);
        let _ = std::fs::remove_file(&path);
    }
}
