use clap::Args;
use serde::Serialize;

use imgctl_core::{Error, InputSource, OutputSink, Region, Result, Size};

use crate::format::ImageFormat;
use crate::{decode, encode};

#[derive(Args, Debug, Clone)]
pub struct CropArgs {
    /// Input file path, or `-` for stdin
    #[arg(short, long)]
    pub input: String,

    /// Output file path, or `-` for stdout
    #[arg(short, long)]
    pub output: String,

    /// Origin X (negative values count from the right edge)
    #[arg(long, allow_hyphen_values = true)]
    pub x: i32,

    /// Origin Y (negative values count from the bottom edge)
    #[arg(long, allow_hyphen_values = true)]
    pub y: i32,

    /// Crop width
    #[arg(long)]
    pub w: u32,

    /// Crop height
    #[arg(long)]
    pub h: u32,

    /// Quality for lossy formats (1-100)
    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    /// Output format (required when writing to stdout; otherwise inferred from path)
    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Serialize)]
pub struct CropOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
}

pub fn run(args: CropArgs) -> Result<CropOutput> {
    let input = InputSource::from_arg(&args.input);
    let sink = OutputSink::from_arg(&args.output);
    let decoded = decode::load(&input)?;
    let img = decoded.image;

    let region = Region {
        x: args.x,
        y: args.y,
        w: args.w,
        h: args.h,
    };
    let resolved = region.resolve(Size {
        w: img.width(),
        h: img.height(),
    })?;

    let cropped = img.crop_imm(resolved.x as u32, resolved.y as u32, resolved.w, resolved.h);

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

    let info = encode::write(&cropped, target_fmt, args.quality, &sink)?;

    Ok(CropOutput {
        output: args.output,
        width: info.width,
        height: info.height,
        format: info.format,
        size_bytes: info.size_bytes,
    })
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
            "imgctl-crop-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn write_png_fixture(path: &PathBuf, w: u32, h: u32) {
        let img = DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, y| {
            let v = ((x * 7 + y * 11) % 255) as u8;
            Rgba([v, v.wrapping_mul(2), v.wrapping_mul(3), 255])
        }));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, &buf).unwrap();
    }

    fn args(input: &PathBuf, output: &PathBuf, x: i32, y: i32, w: u32, h: u32) -> CropArgs {
        CropArgs {
            input: input.to_string_lossy().into_owned(),
            output: output.to_string_lossy().into_owned(),
            x,
            y,
            w,
            h,
            quality: 85,
            format: None,
        }
    }

    #[test]
    fn crop_normal_region() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_png_fixture(&input, 200, 200);

        let out = run(args(&input, &output, 10, 10, 100, 100)).unwrap();
        assert_eq!(out.width, 100);
        assert_eq!(out.height, 100);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn crop_negative_coords_from_bottom_right() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_png_fixture(&input, 200, 200);

        // -50, -50 = origin (150, 150); 30x30 crop ends at (180, 180) — within bounds
        let out = run(args(&input, &output, -50, -50, 30, 30)).unwrap();
        assert_eq!(out.width, 30);
        assert_eq!(out.height, 30);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn crop_overflow_clamps_to_image_bounds() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_png_fixture(&input, 100, 100);

        // origin (50, 50), 200x200 → clamped to 50x50
        let out = run(args(&input, &output, 50, 50, 200, 200)).unwrap();
        assert_eq!(out.width, 50);
        assert_eq!(out.height, 50);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn crop_zero_area_errs() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_png_fixture(&input, 100, 100);

        let err = run(args(&input, &output, 200, 200, 100, 100)).unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }
}
