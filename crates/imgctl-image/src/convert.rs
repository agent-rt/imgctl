use clap::Args;
use serde::Serialize;

use imgctl_core::{Error, InputSource, OutputSink, Result};

use crate::format::ImageFormat;
use crate::{decode, encode};

#[derive(Args, Debug, Clone)]
pub struct ConvertArgs {
    /// Input file path, or `-` for stdin
    #[arg(short, long)]
    pub input: String,

    /// Output file path, or `-` for stdout
    #[arg(short, long)]
    pub output: String,

    /// Quality for lossy formats (1-100)
    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    /// Output format (required when writing to stdout; otherwise inferred from path)
    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Serialize)]
pub struct ConvertOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
}

pub fn run(args: ConvertArgs) -> Result<ConvertOutput> {
    let input = InputSource::from_arg(&args.input);
    let sink = OutputSink::from_arg(&args.output);

    let decoded = decode::load(&input)?;

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

    let info = encode::write(&decoded.image, target_fmt, args.quality, &sink)?;

    Ok(ConvertOutput {
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
            "imgctl-convert-{}-{nanos}-{suffix}",
            std::process::id()
        ))
    }

    fn write_fixture_png(path: &PathBuf) {
        let img = RgbaImage::from_fn(64, 64, |x, y| {
            let v = ((x * 7 + y * 11) % 255) as u8;
            Rgba([v, v.wrapping_mul(2), v.wrapping_mul(3), 255])
        });
        let dyn_img = DynamicImage::ImageRgba8(img);
        let mut buf = Vec::new();
        dyn_img
            .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        std::fs::write(path, &buf).unwrap();
    }

    fn args(input: &PathBuf, output: &PathBuf, quality: u8) -> ConvertArgs {
        ConvertArgs {
            input: input.to_string_lossy().into_owned(),
            output: output.to_string_lossy().into_owned(),
            quality,
            format: None,
        }
    }

    #[test]
    fn convert_png_to_jpeg_preserves_dimensions() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.jpg");
        write_fixture_png(&input);

        let out = run(args(&input, &output, 85)).unwrap();
        assert_eq!(out.width, 64);
        assert_eq!(out.height, 64);
        assert_eq!(out.format, "jpeg");
        assert!(out.size_bytes > 0);

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn convert_quality_low_smaller_than_high() {
        let input = unique_temp("in.png");
        let low = unique_temp("low.jpg");
        let high = unique_temp("high.jpg");
        write_fixture_png(&input);

        let low_out = run(args(&input, &low, 10)).unwrap();
        let high_out = run(args(&input, &high, 95)).unwrap();
        assert!(
            low_out.size_bytes < high_out.size_bytes,
            "low={} high={}",
            low_out.size_bytes,
            high_out.size_bytes
        );

        for p in [&input, &low, &high] {
            let _ = std::fs::remove_file(p);
        }
    }

    #[test]
    fn convert_unknown_extension_errs() {
        let input = unique_temp("in.png");
        let output = unique_temp("out.unknown");
        write_fixture_png(&input);

        let err = run(args(&input, &output, 85)).unwrap_err();
        assert_eq!(err.code(), "UNSUPPORTED_FORMAT");

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }

    #[test]
    fn convert_stdio_without_format_errs() {
        let input = unique_temp("in.png");
        write_fixture_png(&input);

        let mut a = args(&input, &PathBuf::from("/dev/null"), 85);
        a.output = "-".into();
        let err = run(a).unwrap_err();
        assert_eq!(err.code(), "FORMAT_REQUIRED");

        let _ = std::fs::remove_file(&input);
    }

    #[test]
    fn convert_explicit_format_overrides_path() {
        let input = unique_temp("in.png");
        // .unknown extension but explicit --format png
        let output = unique_temp("out.unknown");
        write_fixture_png(&input);

        let out = run(ConvertArgs {
            input: input.to_string_lossy().into_owned(),
            output: output.to_string_lossy().into_owned(),
            quality: 85,
            format: Some(ImageFormat::Png),
        })
        .unwrap();
        assert_eq!(out.format, "png");

        let _ = std::fs::remove_file(&input);
        let _ = std::fs::remove_file(&output);
    }
}
