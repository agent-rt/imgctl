use std::path::Path;

use clap::Args;
use serde::Serialize;

use imgctl_core::{Error, InputSource, OutputSink, Result};

#[derive(Args, Debug, Clone)]
pub struct FixArgs {
    /// Input file path
    #[arg(short, long)]
    pub input: String,

    /// Optional output path; if set, the fixed file is written here
    #[arg(short, long)]
    pub output: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FixOutput {
    pub detected_format: &'static str,
    pub extension_format: &'static str,
    pub mismatch: bool,
    pub fixed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
}

pub fn run(args: FixArgs) -> Result<FixOutput> {
    let bytes = InputSource::from_arg(&args.input).read_all()?;

    let detected = detect_format(&bytes);
    let extension = format_from_path(Path::new(&args.input));

    // Repair logic — currently JPEG truncation only.
    let (fixed_bytes, fixed) = match detected {
        "jpeg" => repair_jpeg(&bytes),
        _ => (bytes.clone(), false),
    };

    let written = if let Some(out_path) = args.output {
        OutputSink::from_arg(&out_path).write_all(&fixed_bytes)?;
        Some(out_path)
    } else {
        None
    };

    Ok(FixOutput {
        detected_format: detected,
        extension_format: extension,
        mismatch: detected != extension && extension != "unknown",
        fixed,
        output: written,
    })
}

/// Detect format by magic bytes. Returns one of: png/jpeg/webp/bmp/gif/tiff/ico/heic/avif/unknown.
pub fn detect_format(bytes: &[u8]) -> &'static str {
    if bytes.len() < 4 {
        return "unknown";
    }
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return "png";
    }
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return "jpeg";
    }
    if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return "webp";
    }
    if bytes.starts_with(b"BM") {
        return "bmp";
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return "gif";
    }
    if bytes.starts_with(&[0x49, 0x49, 0x2A, 0x00]) || bytes.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) {
        return "tiff";
    }
    if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
        return "ico";
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if brand == b"heic" || brand == b"heix" || brand == b"mif1" || brand == b"msf1" {
            return "heic";
        }
        if brand == b"avif" || brand == b"avis" {
            return "avif";
        }
    }
    "unknown"
}

fn format_from_path(p: &Path) -> &'static str {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .map(|s| match s.as_str() {
            "png" => "png",
            "jpg" | "jpeg" => "jpeg",
            "webp" => "webp",
            "bmp" => "bmp",
            "gif" => "gif",
            "tif" | "tiff" => "tiff",
            "ico" => "ico",
            "heic" | "heif" => "heic",
            "avif" => "avif",
            _ => "unknown",
        })
        .unwrap_or("unknown")
}

/// Repair a possibly-truncated JPEG by ensuring the byte stream ends with
/// `FF D9` (EOI). If a partial EOI tail is detected, truncate to the last
/// complete EOI marker; otherwise append one.
pub fn repair_jpeg(bytes: &[u8]) -> (Vec<u8>, bool) {
    if bytes.len() < 4 {
        return (bytes.to_vec(), false);
    }
    let n = bytes.len();
    // Already terminated.
    if bytes[n - 2] == 0xFF && bytes[n - 1] == 0xD9 {
        return (bytes.to_vec(), false);
    }
    // Walk backward looking for the last FF D9 occurrence.
    let mut last_eoi: Option<usize> = None;
    let mut i = n - 1;
    while i > 0 {
        if bytes[i - 1] == 0xFF && bytes[i] == 0xD9 {
            last_eoi = Some(i);
            break;
        }
        i -= 1;
    }
    let mut out = match last_eoi {
        Some(eoi_pos) => bytes[..=eoi_pos].to_vec(),
        None => bytes.to_vec(),
    };
    // If no EOI was found at all, append one.
    if last_eoi.is_none() {
        out.push(0xFF);
        out.push(0xD9);
    }
    (out, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::SystemTime;

    use image::{DynamicImage, Rgba, RgbaImage};

    fn unique_temp(suffix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "imgctl-fix-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn png_bytes() -> Vec<u8> {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(16, 16, Rgba([10, 20, 30, 255])));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
        buf
    }

    fn jpeg_bytes() -> Vec<u8> {
        let img = DynamicImage::ImageRgb8(image::RgbImage::from_pixel(16, 16, image::Rgb([100, 50, 200])));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Jpeg).unwrap();
        buf
    }

    #[test]
    fn detect_format_recognizes_common_types() {
        assert_eq!(detect_format(&png_bytes()), "png");
        assert_eq!(detect_format(&jpeg_bytes()), "jpeg");
        assert_eq!(detect_format(b"GIF89a..."), "gif");
        assert_eq!(detect_format(b"BMfoo"), "bmp");
        assert_eq!(detect_format(b"random data here"), "unknown");
    }

    #[test]
    fn format_from_path_basic() {
        assert_eq!(format_from_path(Path::new("foo.png")), "png");
        assert_eq!(format_from_path(Path::new("bar.JPG")), "jpeg");
        assert_eq!(format_from_path(Path::new("baz.unknown")), "unknown");
        assert_eq!(format_from_path(Path::new("noext")), "unknown");
    }

    #[test]
    fn fix_png_with_jpg_extension_reports_mismatch() {
        let path = unique_temp("misnamed.jpg");
        std::fs::write(&path, png_bytes()).unwrap();
        let out = run(FixArgs {
            input: path.to_string_lossy().into_owned(),
            output: None,
        })
        .unwrap();
        assert_eq!(out.detected_format, "png");
        assert_eq!(out.extension_format, "jpeg");
        assert!(out.mismatch);
        assert!(!out.fixed);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn fix_repairs_truncated_jpeg() {
        // Strip the trailing FF D9 to simulate truncation.
        let mut bytes = jpeg_bytes();
        let _ = bytes.pop();
        let _ = bytes.pop();
        // Ensure no EOI at end now.
        assert!(!(bytes[bytes.len() - 2] == 0xFF && bytes[bytes.len() - 1] == 0xD9));

        let (fixed, did_fix) = repair_jpeg(&bytes);
        assert!(did_fix);
        assert_eq!(fixed[fixed.len() - 2], 0xFF);
        assert_eq!(fixed[fixed.len() - 1], 0xD9);
        // Repaired bytes should still decode.
        image::load_from_memory_with_format(&fixed, image::ImageFormat::Jpeg)
            .expect("repaired JPEG should decode");
    }

    #[test]
    fn fix_complete_jpeg_no_change() {
        let bytes = jpeg_bytes();
        let (out, did_fix) = repair_jpeg(&bytes);
        assert!(!did_fix);
        assert_eq!(out, bytes);
    }

    #[test]
    fn fix_writes_output_when_requested() {
        // Truncated JPEG.
        let mut bytes = jpeg_bytes();
        bytes.pop();
        bytes.pop();
        let in_path = unique_temp("trunc.jpg");
        let out_path = unique_temp("repaired.jpg");
        std::fs::write(&in_path, &bytes).unwrap();

        let out = run(FixArgs {
            input: in_path.to_string_lossy().into_owned(),
            output: Some(out_path.to_string_lossy().into_owned()),
        })
        .unwrap();
        assert!(out.fixed);
        assert!(out.output.is_some());
        let written = std::fs::read(&out_path).unwrap();
        assert_eq!(written[written.len() - 2], 0xFF);
        assert_eq!(written[written.len() - 1], 0xD9);

        let _ = std::fs::remove_file(&in_path);
        let _ = std::fs::remove_file(&out_path);
    }
}
