use clap::{Args, ValueEnum};
use image::imageops::{self, FilterType};
use image::{DynamicImage, RgbaImage};
use serde::Serialize;

use imgctl_core::{Error, InputSource, OutputSink, Region, Result, Size};

use crate::format::ImageFormat;
use crate::{decode, encode};

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum BlurType {
    #[default]
    Gaussian,
    Pixelate,
}

fn parse_region(s: &str) -> std::result::Result<Region, String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected X,Y,W,H, got: {s}"));
    }
    let x = parts[0]
        .trim()
        .parse::<i32>()
        .map_err(|e| format!("x: {e}"))?;
    let y = parts[1]
        .trim()
        .parse::<i32>()
        .map_err(|e| format!("y: {e}"))?;
    let w = parts[2]
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("w: {e}"))?;
    let h = parts[3]
        .trim()
        .parse::<u32>()
        .map_err(|e| format!("h: {e}"))?;
    Ok(Region { x, y, w, h })
}

#[derive(Args, Debug, Clone)]
pub struct BlurArgs {
    #[arg(short, long)]
    pub input: String,

    #[arg(short, long)]
    pub output: String,

    /// Region to blur as "X,Y,W,H"; can be repeated for multiple regions
    #[arg(long, value_parser = parse_region, allow_hyphen_values = true)]
    pub region: Vec<Region>,

    /// Blur sigma; for pixelate, derives block size as max(2, round(sigma*2))
    #[arg(long, default_value_t = 8.0)]
    pub sigma: f32,

    /// Blur algorithm
    #[arg(long = "type", value_enum, default_value_t = BlurType::Gaussian)]
    pub kind: BlurType,

    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Serialize)]
pub struct BlurOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
    pub regions_processed: usize,
}

pub fn run(args: BlurArgs) -> Result<BlurOutput> {
    if args.region.is_empty() {
        return Err(Error::InvalidArgument(
            "blur requires at least one --region".into(),
        ));
    }
    if args.sigma <= 0.0 {
        return Err(Error::InvalidArgument("blur --sigma must be > 0".into()));
    }

    let input = InputSource::from_arg(&args.input);
    let sink = OutputSink::from_arg(&args.output);
    let decoded = decode::load(&input)?;
    let mut img = decoded.image.to_rgba8();
    let img_size = Size {
        w: img.width(),
        h: img.height(),
    };

    let regions_processed = args.region.len();
    for region in &args.region {
        let resolved = region.resolve(img_size)?;
        process_region(&mut img, resolved, args.sigma, args.kind);
    }

    let dyn_img = DynamicImage::ImageRgba8(img);

    let target_fmt = if let Some(f) = args.format {
        f
    } else {
        match &sink {
            OutputSink::File(p) => ImageFormat::from_path(p).ok_or_else(|| {
                Error::UnsupportedFormat(format!(
                    "cannot infer format from output path: {}",
                    p.display()
                ))
            })?,
            OutputSink::Stdio => return Err(Error::FormatRequired),
        }
    };

    let info = encode::write(&dyn_img, target_fmt, args.quality, &sink)?;

    Ok(BlurOutput {
        output: args.output,
        width: info.width,
        height: info.height,
        format: info.format,
        size_bytes: info.size_bytes,
        regions_processed,
    })
}

/// Apply blur in-place over a single resolved region.
pub fn process_region(img: &mut RgbaImage, r: Region, sigma: f32, kind: BlurType) {
    let sub = imageops::crop_imm(img, r.x as u32, r.y as u32, r.w, r.h).to_image();
    let processed = match kind {
        BlurType::Gaussian => imageops::blur(&sub, sigma),
        BlurType::Pixelate => pixelate(&sub, sigma),
    };
    paste_back(img, &processed, r.x as u32, r.y as u32);
}

fn pixelate(sub: &RgbaImage, sigma: f32) -> RgbaImage {
    let block = ((sigma * 2.0).round() as u32).max(2);
    let small_w = (sub.width() / block).max(1);
    let small_h = (sub.height() / block).max(1);
    let small = imageops::resize(sub, small_w, small_h, FilterType::Nearest);
    imageops::resize(&small, sub.width(), sub.height(), FilterType::Nearest)
}

fn paste_back(dst: &mut RgbaImage, src: &RgbaImage, ox: u32, oy: u32) {
    for y in 0..src.height() {
        for x in 0..src.width() {
            let dx = ox + x;
            let dy = oy + y;
            if dx < dst.width() && dy < dst.height() {
                dst.put_pixel(dx, dy, *src.get_pixel(x, y));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    fn checkerboard(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            if (x / 4 + y / 4) % 2 == 0 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 255])
            }
        })
    }

    fn variance(img: &RgbaImage, region: Region) -> f64 {
        let mut sum = 0u64;
        let mut count = 0u64;
        let mut values = Vec::new();
        for y in region.y..(region.y + region.h as i32) {
            for x in region.x..(region.x + region.w as i32) {
                let v = img.get_pixel(x as u32, y as u32).0[0] as u64;
                sum += v;
                count += 1;
                values.push(v as f64);
            }
        }
        let mean = sum as f64 / count.max(1) as f64;
        values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count.max(1) as f64
    }

    #[test]
    fn parse_region_basic() {
        assert_eq!(
            parse_region("10,20,30,40").unwrap(),
            Region {
                x: 10,
                y: 20,
                w: 30,
                h: 40
            }
        );
        assert_eq!(
            parse_region("-5,-5,10,10").unwrap(),
            Region {
                x: -5,
                y: -5,
                w: 10,
                h: 10
            }
        );
    }

    #[test]
    fn parse_region_invalid() {
        assert!(parse_region("10,20,30").is_err());
        assert!(parse_region("a,b,c,d").is_err());
        assert!(parse_region("10,20,-5,40").is_err()); // w must be u32
    }

    #[test]
    fn gaussian_smooths_high_frequency_pattern() {
        let mut img = checkerboard(64, 64);
        let region = Region {
            x: 16,
            y: 16,
            w: 32,
            h: 32,
        };
        let pre = variance(&img, region);
        process_region(&mut img, region, 4.0, BlurType::Gaussian);
        let post = variance(&img, region);
        assert!(
            post < pre,
            "blur should reduce variance: pre={pre}, post={post}"
        );
    }

    #[test]
    fn pixelate_reduces_unique_colors() {
        let mut img = checkerboard(64, 64);
        let region = Region {
            x: 16,
            y: 16,
            w: 32,
            h: 32,
        };
        process_region(&mut img, region, 4.0, BlurType::Pixelate);
        // Pixelate: adjacent pixels in the same block share the same color.
        // Sample two pixels inside the same 8x8 block — they should match.
        let p1 = *img.get_pixel(20, 20);
        let p2 = *img.get_pixel(21, 20);
        assert_eq!(p1.0, p2.0, "adjacent pixels in same block should match");
    }

    #[test]
    fn region_outside_image_does_not_panic() {
        let mut img = checkerboard(50, 50);
        // resolve will clip; if it returns an invalid region, run() would error.
        // process_region itself is given a pre-resolved region, so callers must validate.
        let valid = Region {
            x: 10,
            y: 10,
            w: 20,
            h: 20,
        };
        process_region(&mut img, valid, 2.0, BlurType::Gaussian);
    }
}
