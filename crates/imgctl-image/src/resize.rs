use clap::{Args, ValueEnum};
use image::DynamicImage;
use image::imageops::FilterType;
use serde::Serialize;

use imgctl_core::{Error, InputSource, OutputSink, Result};

use crate::format::ImageFormat;
use crate::{decode, encode};

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "kebab-case")]
pub enum FitMode {
    /// Preserve aspect ratio, fit within target without cropping (default)
    #[default]
    Contain,
    /// Preserve aspect ratio, cover target by cropping the overflow
    Cover,
    /// Stretch to exact target dimensions, ignoring aspect ratio
    Fill,
    /// Only shrink — never upscale
    ScaleDown,
}

#[derive(Args, Debug, Clone)]
pub struct ResizeArgs {
    /// Input file path, or `-` for stdin
    #[arg(short, long)]
    pub input: String,

    /// Output file path, or `-` for stdout
    #[arg(short, long)]
    pub output: String,

    /// Target width (at least one of --width/--height required)
    #[arg(long)]
    pub width: Option<u32>,

    /// Target height (at least one of --width/--height required)
    #[arg(long)]
    pub height: Option<u32>,

    /// Fit mode when both --width and --height are given
    #[arg(long, value_enum, default_value_t = FitMode::Contain)]
    pub fit: FitMode,

    /// Quality for lossy formats (1-100)
    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    /// Output format (required when writing to stdout; otherwise inferred from path)
    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Serialize)]
pub struct ResizeOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
}

pub fn run(args: ResizeArgs) -> Result<ResizeOutput> {
    if args.width.is_none() && args.height.is_none() {
        return Err(Error::InvalidArgument(
            "resize requires at least --width or --height".into(),
        ));
    }

    let input = InputSource::from_arg(&args.input);
    let sink = OutputSink::from_arg(&args.output);
    let decoded = decode::load(&input)?;
    let resized = apply_resize(&decoded.image, args.width, args.height, args.fit);

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

    let info = encode::write(&resized, target_fmt, args.quality, &sink)?;

    Ok(ResizeOutput {
        output: args.output,
        width: info.width,
        height: info.height,
        format: info.format,
        size_bytes: info.size_bytes,
    })
}

fn apply_resize(img: &DynamicImage, w: Option<u32>, h: Option<u32>, fit: FitMode) -> DynamicImage {
    let filter = FilterType::Lanczos3;
    let (sw, sh) = (img.width(), img.height());

    match (w, h) {
        (Some(tw), None) => {
            let th = ((sh as f64 * tw as f64 / sw as f64).round() as u32).max(1);
            img.resize_exact(tw, th, filter)
        }
        (None, Some(th)) => {
            let tw = ((sw as f64 * th as f64 / sh as f64).round() as u32).max(1);
            img.resize_exact(tw, th, filter)
        }
        (Some(tw), Some(th)) => match fit {
            FitMode::Contain => img.resize(tw, th, filter),
            FitMode::Cover => img.resize_to_fill(tw, th, filter),
            FitMode::Fill => img.resize_exact(tw, th, filter),
            FitMode::ScaleDown => {
                if sw <= tw && sh <= th {
                    img.clone()
                } else {
                    img.resize(tw, th, filter)
                }
            }
        },
        (None, None) => unreachable!("validated above"),
    }
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
            "imgctl-resize-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn make_image(w: u32, h: u32) -> DynamicImage {
        DynamicImage::ImageRgba8(RgbaImage::from_fn(w, h, |x, y| {
            let v = ((x * 7 + y * 11) % 255) as u8;
            Rgba([v, v.wrapping_mul(2), v.wrapping_mul(3), 255])
        }))
    }

    fn write_png_fixture(path: &PathBuf, w: u32, h: u32) {
        let img = make_image(w, h);
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, &buf).unwrap();
    }

    #[test]
    fn contain_preserves_aspect() {
        let out = apply_resize(&make_image(400, 200), Some(100), Some(100), FitMode::Contain);
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 50);
    }

    #[test]
    fn cover_fills_target_exactly() {
        let out = apply_resize(&make_image(400, 200), Some(100), Some(100), FitMode::Cover);
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 100);
    }

    #[test]
    fn fill_stretches_to_exact() {
        let out = apply_resize(&make_image(400, 200), Some(150), Some(150), FitMode::Fill);
        assert_eq!(out.width(), 150);
        assert_eq!(out.height(), 150);
    }

    #[test]
    fn scale_down_skips_upscale() {
        let out = apply_resize(&make_image(100, 100), Some(200), Some(200), FitMode::ScaleDown);
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 100);
    }

    #[test]
    fn scale_down_shrinks_larger_source() {
        let out = apply_resize(&make_image(400, 200), Some(200), Some(200), FitMode::ScaleDown);
        // Falls through to Contain: ratio 0.5 → (200, 100)
        assert_eq!(out.width(), 200);
        assert_eq!(out.height(), 100);
    }

    #[test]
    fn width_only_preserves_aspect() {
        let out = apply_resize(&make_image(400, 200), Some(100), None, FitMode::Contain);
        assert_eq!(out.width(), 100);
        assert_eq!(out.height(), 50);
    }

    #[test]
    fn height_only_preserves_aspect() {
        let out = apply_resize(&make_image(400, 200), None, Some(100), FitMode::Contain);
        assert_eq!(out.width(), 200);
        assert_eq!(out.height(), 100);
    }

    fn cli_args(
        input: &PathBuf,
        output: &PathBuf,
        w: Option<u32>,
        h: Option<u32>,
        fit: FitMode,
    ) -> ResizeArgs {
        ResizeArgs {
            input: input.to_string_lossy().into_owned(),
            output: output.to_string_lossy().into_owned(),
            width: w,
            height: h,
            fit,
            quality: 85,
            format: None,
        }
    }

    #[test]
    fn run_resize_writes_expected_dimensions() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_png_fixture(&input, 400, 200);

        let out = run(cli_args(&input, &output, Some(100), Some(100), FitMode::Cover)).unwrap();
        assert_eq!(out.width, 100);
        assert_eq!(out.height, 100);
        assert_eq!(out.format, "png");
        assert!(out.size_bytes > 0);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn run_resize_without_dimensions_errs() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.png");
        write_png_fixture(&input, 100, 100);

        let err = run(cli_args(&input, &output, None, None, FitMode::Contain)).unwrap_err();
        assert_eq!(err.code(), "INVALID_ARGUMENT");

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }
}
