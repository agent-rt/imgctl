use clap::{Args, ValueEnum};
use image::DynamicImage;
use serde::Serialize;

use imgctl_core::{ColorRgba, Error, InputSource, OutputSink, Result};

use crate::drawing::text::{TextAlign as InternalAlign, load_font, render_text};
use crate::format::ImageFormat;
use crate::{decode, encode};

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl From<TextAlign> for InternalAlign {
    fn from(a: TextAlign) -> Self {
        match a {
            TextAlign::Left => InternalAlign::Left,
            TextAlign::Center => InternalAlign::Center,
            TextAlign::Right => InternalAlign::Right,
        }
    }
}

#[derive(Args, Debug, Clone)]
pub struct TextArgs {
    /// Input file path, or `-` for stdin
    #[arg(short, long)]
    pub input: String,

    /// Output file path, or `-` for stdout
    #[arg(short, long)]
    pub output: String,

    /// Text content to render
    #[arg(long, allow_hyphen_values = true)]
    pub text: String,

    /// X coordinate of the text anchor (see --align)
    #[arg(long, allow_hyphen_values = true)]
    pub x: i32,

    /// Y coordinate (top of the text)
    #[arg(long, allow_hyphen_values = true)]
    pub y: i32,

    /// Font size in pixels
    #[arg(long, default_value_t = 24)]
    pub size: u32,

    /// Text color (e.g. #FF0000 or #FF0000FF)
    #[arg(long, default_value = "#000000")]
    pub color: String,

    /// Optional background color behind the text bounding box
    #[arg(long)]
    pub bg: Option<String>,

    /// Horizontal alignment of `--text` relative to `--x`
    #[arg(long, value_enum, default_value_t = TextAlign::Left)]
    pub align: TextAlign,

    /// Font: filesystem path to a .ttf/.otf, or a system font family name
    /// (e.g. "Hiragino Sans", "PingFang SC"). Defaults to embedded NotoSans (Latin).
    #[arg(long)]
    pub font: Option<String>,

    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Serialize)]
pub struct TextOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
}

pub fn run(args: TextArgs) -> Result<TextOutput> {
    let input = InputSource::from_arg(&args.input);
    let sink = OutputSink::from_arg(&args.output);

    let font = load_font(args.font.as_deref())?;
    let color = ColorRgba::parse(&args.color)?;
    let bg = args.bg.as_deref().map(ColorRgba::parse).transpose()?;

    let decoded = decode::load(&input)?;
    let mut img = decoded.image.to_rgba8();

    render_text(
        &mut img,
        &args.text,
        args.x,
        args.y,
        args.size,
        color,
        args.align.into(),
        &font,
        bg,
    )?;

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

    Ok(TextOutput {
        output: args.output,
        width: info.width,
        height: info.height,
        format: info.format,
        size_bytes: info.size_bytes,
    })
}
