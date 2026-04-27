use clap::{Args, ValueEnum};
use image::DynamicImage;
use serde::Serialize;

use imgctl_core::{ColorRgba, Error, InputSource, OutputSink, Result};

use crate::drawing::arrow::{ArrowStyle as InternalStyle, draw_arrow};
use crate::format::ImageFormat;
use crate::{decode, encode};

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum ArrowStyle {
    #[default]
    Solid,
    Dashed,
}

impl From<ArrowStyle> for InternalStyle {
    fn from(s: ArrowStyle) -> Self {
        match s {
            ArrowStyle::Solid => InternalStyle::Solid,
            ArrowStyle::Dashed => InternalStyle::Dashed,
        }
    }
}

fn parse_point(s: &str) -> std::result::Result<(i32, i32), String> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return Err(format!("expected X,Y, got: {s}"));
    }
    let x = parts[0]
        .trim()
        .parse::<i32>()
        .map_err(|e| format!("x: {e}"))?;
    let y = parts[1]
        .trim()
        .parse::<i32>()
        .map_err(|e| format!("y: {e}"))?;
    Ok((x, y))
}

#[derive(Args, Debug, Clone)]
pub struct ArrowArgs {
    #[arg(short, long)]
    pub input: String,

    #[arg(short, long)]
    pub output: String,

    /// Start point as "X,Y"
    #[arg(long, value_parser = parse_point, allow_hyphen_values = true)]
    pub from: (i32, i32),

    /// End point as "X,Y" (arrowhead tip)
    #[arg(long, value_parser = parse_point, allow_hyphen_values = true)]
    pub to: (i32, i32),

    /// Arrow color
    #[arg(long, default_value = "#FF0000")]
    pub color: String,

    /// Line width in pixels
    #[arg(long, default_value_t = 2)]
    pub width: u32,

    /// Arrowhead size in pixels
    #[arg(long, default_value_t = 12)]
    pub head_size: u32,

    /// Line style
    #[arg(long, value_enum, default_value_t = ArrowStyle::Solid)]
    pub style: ArrowStyle,

    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Serialize)]
pub struct ArrowOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
}

pub fn run(args: ArrowArgs) -> Result<ArrowOutput> {
    let input = InputSource::from_arg(&args.input);
    let sink = OutputSink::from_arg(&args.output);

    let color = ColorRgba::parse(&args.color)?;
    let decoded = decode::load(&input)?;
    let mut img = decoded.image.to_rgba8();

    draw_arrow(
        &mut img,
        args.from,
        args.to,
        color,
        args.width,
        args.head_size,
        args.style.into(),
    );

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

    Ok(ArrowOutput {
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

    #[test]
    fn parse_point_basic() {
        assert_eq!(parse_point("10,20").unwrap(), (10, 20));
        assert_eq!(parse_point("-50,-30").unwrap(), (-50, -30));
        assert_eq!(parse_point(" 1 , 2 ").unwrap(), (1, 2));
    }

    #[test]
    fn parse_point_invalid() {
        assert!(parse_point("10").is_err());
        assert!(parse_point("a,b").is_err());
        assert!(parse_point("10,20,30").is_err());
    }
}
