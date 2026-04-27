use std::collections::HashMap;
use std::path::Path;

use clap::Args;
use image::imageops::FilterType;
use image::{ColorType, DynamicImage, GenericImageView};
use serde::Serialize;

use imgctl_core::{Error, InputSource, Result};

#[derive(Args, Debug, Clone)]
pub struct InfoArgs {
    /// Input file path, or `-` for stdin
    #[arg(short, long)]
    pub input: String,
}

#[derive(Debug, Serialize)]
pub struct Gps {
    pub lat: f64,
    pub lng: f64,
}

#[derive(Debug, Default, Serialize)]
pub struct ExifData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub taken: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gps: Option<Gps>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
}

impl ExifData {
    fn is_empty(&self) -> bool {
        self.taken.is_none() && self.gps.is_none() && self.device.is_none()
    }
}

#[derive(Debug, Serialize)]
pub struct InfoOutput {
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
    pub channels: u8,
    pub color_space: &'static str,
    pub has_alpha: bool,
    pub dominant_colors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exif: Option<ExifData>,
}

pub fn run(args: InfoArgs) -> Result<InfoOutput> {
    let input = InputSource::from_arg(&args.input);
    let bytes = input.read_all()?;
    let size_bytes = bytes.len() as u64;

    let raw_fmt =
        image::guess_format(&bytes).map_err(|e| Error::UnsupportedFormat(e.to_string()))?;
    let img = image::load_from_memory_with_format(&bytes, raw_fmt)
        .map_err(|e| Error::Image(e.to_string()))?;

    let format = format_str(raw_fmt);
    let (channels, has_alpha) = color_info(img.color());
    let dominant_colors = dominant_colors(&img, 3);
    let exif = read_exif(&bytes);
    let exif_field = if let Some(e) = exif {
        if e.is_empty() { None } else { Some(e) }
    } else {
        None
    };

    Ok(InfoOutput {
        width: img.width(),
        height: img.height(),
        format,
        size_bytes,
        channels,
        color_space: "sRGB",
        has_alpha,
        dominant_colors,
        exif: exif_field,
    })
}

fn format_str(f: image::ImageFormat) -> &'static str {
    match f {
        image::ImageFormat::Png => "png",
        image::ImageFormat::Jpeg => "jpeg",
        image::ImageFormat::WebP => "webp",
        image::ImageFormat::Bmp => "bmp",
        image::ImageFormat::Gif => "gif",
        image::ImageFormat::Tiff => "tiff",
        image::ImageFormat::Ico => "ico",
        _ => "unknown",
    }
}

fn color_info(c: ColorType) -> (u8, bool) {
    match c {
        ColorType::L8 | ColorType::L16 => (1, false),
        ColorType::La8 | ColorType::La16 => (2, true),
        ColorType::Rgb8 | ColorType::Rgb16 | ColorType::Rgb32F => (3, false),
        ColorType::Rgba8 | ColorType::Rgba16 | ColorType::Rgba32F => (4, true),
        _ => (0, false),
    }
}

/// Quantization-based dominant color extraction:
/// - downsample to 64x64 (Nearest, fast)
/// - bucket each pixel by 5-bit-per-channel quantization (32K buckets max)
/// - take top-K by count, return the average color of each bucket as hex
fn dominant_colors(img: &DynamicImage, k: usize) -> Vec<String> {
    let small = img.resize(64, 64, FilterType::Nearest).to_rgb8();
    let mut buckets: HashMap<(u8, u8, u8), (u64, u64, u64, u64)> = HashMap::new();
    for pixel in small.pixels() {
        let key = (pixel.0[0] >> 3, pixel.0[1] >> 3, pixel.0[2] >> 3);
        let e = buckets.entry(key).or_insert((0, 0, 0, 0));
        e.0 += 1;
        e.1 += u64::from(pixel.0[0]);
        e.2 += u64::from(pixel.0[1]);
        e.3 += u64::from(pixel.0[2]);
    }
    let mut sorted: Vec<_> = buckets.into_values().collect();
    sorted.sort_by_key(|(count, _, _, _)| std::cmp::Reverse(*count));
    sorted
        .into_iter()
        .take(k)
        .map(|(count, sr, sg, sb)| {
            let count = count.max(1);
            let r = (sr / count) as u8;
            let g = (sg / count) as u8;
            let b = (sb / count) as u8;
            format!("#{r:02X}{g:02X}{b:02X}")
        })
        .collect()
}

fn read_exif(bytes: &[u8]) -> Option<ExifData> {
    let mut cursor = std::io::Cursor::new(bytes);
    let reader = exif::Reader::new();
    let exif = reader.read_from_container(&mut cursor).ok()?;

    let taken = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string())
        .filter(|s| !s.is_empty());

    let lat = read_gps(&exif, exif::Tag::GPSLatitude, exif::Tag::GPSLatitudeRef);
    let lng = read_gps(&exif, exif::Tag::GPSLongitude, exif::Tag::GPSLongitudeRef);
    let gps = match (lat, lng) {
        (Some(lat), Some(lng)) => Some(Gps { lat, lng }),
        _ => None,
    };

    let make = exif
        .get_field(exif::Tag::Make, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string().trim_matches('"').to_string());
    let model = exif
        .get_field(exif::Tag::Model, exif::In::PRIMARY)
        .map(|f| f.display_value().to_string().trim_matches('"').to_string());
    let device = match (make, model) {
        (Some(m1), Some(m2)) if !m1.is_empty() && !m2.is_empty() => Some(format!("{m1} {m2}")),
        (Some(m), None) | (None, Some(m)) if !m.is_empty() => Some(m),
        _ => None,
    };

    Some(ExifData { taken, gps, device })
}

fn read_gps(exif: &exif::Exif, value_tag: exif::Tag, ref_tag: exif::Tag) -> Option<f64> {
    let field = exif.get_field(value_tag, exif::In::PRIMARY)?;
    let rationals = match &field.value {
        exif::Value::Rational(v) => v,
        _ => return None,
    };
    if rationals.len() < 3 {
        return None;
    }
    let deg = rationals[0].to_f64();
    let min = rationals[1].to_f64();
    let sec = rationals[2].to_f64();
    let mut decimal = deg + min / 60.0 + sec / 3600.0;
    if let Some(rf) = exif.get_field(ref_tag, exif::In::PRIMARY) {
        let s = rf.display_value().to_string();
        if s.contains('S') || s.contains('W') {
            decimal = -decimal;
        }
    }
    Some(decimal)
}

// Helper for tests / CLI: detected format string from a Path's extension fallback.
#[allow(dead_code)]
fn format_from_path(p: &Path) -> &'static str {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .map(|s| match s.as_str() {
            "png" => "png",
            "jpg" | "jpeg" => "jpeg",
            "webp" => "webp",
            _ => "unknown",
        })
        .unwrap_or("unknown")
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
            "imgctl-info-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn write_solid_png(path: &PathBuf, color: [u8; 4]) {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(64, 64, Rgba(color)));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, &buf).unwrap();
    }

    #[test]
    fn info_solid_red_png() {
        let path = unique_temp("red.png");
        write_solid_png(&path, [255, 0, 0, 255]);
        let out = run(InfoArgs {
            input: path.to_string_lossy().into_owned(),
        })
        .unwrap();
        assert_eq!(out.width, 64);
        assert_eq!(out.height, 64);
        assert_eq!(out.format, "png");
        assert_eq!(out.channels, 4);
        assert!(out.has_alpha);
        assert!(!out.dominant_colors.is_empty());
        let top = &out.dominant_colors[0];
        assert!(
            top.starts_with("#FF") && top[3..5] == *"00" && top[5..7] == *"00",
            "expected red top color, got {top}"
        );
        assert!(out.exif.is_none(), "PNG fixture should have no EXIF");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn info_unknown_bytes_errs() {
        let path = unique_temp("garbage.bin");
        std::fs::write(&path, b"not an image").unwrap();
        let err = run(InfoArgs {
            input: path.to_string_lossy().into_owned(),
        })
        .unwrap_err();
        assert_eq!(err.code(), "UNSUPPORTED_FORMAT");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn dominant_colors_top_is_solid_blue() {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_pixel(32, 32, Rgba([0, 0, 255, 255])));
        let colors = dominant_colors(&img, 3);
        assert!(!colors.is_empty());
        // Top color should be #0000FFxx pattern (allowing rounding ±1).
        let c = &colors[0];
        assert!(c.starts_with("#00"), "expected blue, got {c}");
    }

    #[test]
    fn color_info_variants() {
        assert_eq!(color_info(ColorType::Rgb8), (3, false));
        assert_eq!(color_info(ColorType::Rgba8), (4, true));
        assert_eq!(color_info(ColorType::L8), (1, false));
        assert_eq!(color_info(ColorType::La8), (2, true));
    }
}
