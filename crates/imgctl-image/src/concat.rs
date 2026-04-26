use clap::{Args, ValueEnum};
use image::{DynamicImage, Rgba, RgbaImage};
use image::imageops;
use serde::Serialize;

use imgctl_core::{ColorRgba, Error, InputSource, OutputSink, Result};

use crate::format::ImageFormat;
use crate::{decode, encode};

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Horizontal,
    Vertical,
}

#[derive(ValueEnum, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[clap(rename_all = "lowercase")]
pub enum Align {
    Start,
    #[default]
    Center,
    End,
}

#[derive(Args, Debug, Clone)]
pub struct ConcatArgs {
    /// Input file paths (specify -i multiple times for multiple images, ≥2)
    #[arg(short, long)]
    pub input: Vec<String>,

    #[arg(short, long)]
    pub output: String,

    #[arg(long, value_enum, default_value_t = Direction::Horizontal)]
    pub direction: Direction,

    /// Gap between images in pixels
    #[arg(long, default_value_t = 0)]
    pub gap: u32,

    /// Background color
    #[arg(long, default_value = "#FFFFFF")]
    pub bg: String,

    /// Cross-axis alignment for images of differing extent on the cross axis
    #[arg(long, value_enum, default_value_t = Align::Center)]
    pub align: Align,

    #[arg(long, default_value_t = 85)]
    pub quality: u8,

    #[arg(long, value_enum)]
    pub format: Option<ImageFormat>,
}

#[derive(Debug, Serialize)]
pub struct ConcatOutput {
    pub output: String,
    pub width: u32,
    pub height: u32,
    pub format: &'static str,
    pub size_bytes: u64,
    pub inputs: usize,
}

pub fn run(args: ConcatArgs) -> Result<ConcatOutput> {
    if args.input.len() < 2 {
        return Err(Error::InvalidArgument(
            "concat requires at least 2 inputs (use -i multiple times)".into(),
        ));
    }
    let bg = ColorRgba::parse(&args.bg)?;
    let sink = OutputSink::from_arg(&args.output);

    let mut images: Vec<RgbaImage> = Vec::with_capacity(args.input.len());
    for path in &args.input {
        let src = InputSource::from_arg(path);
        let dec = decode::load(&src)?;
        images.push(dec.image.to_rgba8());
    }

    let canvas = compose(&images, args.direction, args.gap, args.align, bg)?;
    let dyn_img = DynamicImage::ImageRgba8(canvas);

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

    Ok(ConcatOutput {
        output: args.output,
        width: info.width,
        height: info.height,
        format: info.format,
        size_bytes: info.size_bytes,
        inputs: args.input.len(),
    })
}

/// Compose multiple images into a single canvas (no I/O).
pub fn compose(
    images: &[RgbaImage],
    direction: Direction,
    gap: u32,
    align: Align,
    bg: ColorRgba,
) -> Result<RgbaImage> {
    let n = images.len() as u32;
    if n < 2 {
        return Err(Error::InvalidArgument(
            "compose requires at least 2 images".into(),
        ));
    }
    let total_gap = gap.saturating_mul(n.saturating_sub(1));
    let (canvas_w, canvas_h) = match direction {
        Direction::Horizontal => {
            let w: u32 = images.iter().map(|i| i.width()).sum::<u32>() + total_gap;
            let h: u32 = images.iter().map(|i| i.height()).max().unwrap_or(0);
            (w, h)
        }
        Direction::Vertical => {
            let w: u32 = images.iter().map(|i| i.width()).max().unwrap_or(0);
            let h: u32 = images.iter().map(|i| i.height()).sum::<u32>() + total_gap;
            (w, h)
        }
    };
    if canvas_w == 0 || canvas_h == 0 {
        return Err(Error::InvalidArgument(
            "concat canvas would be empty (zero-size input?)".into(),
        ));
    }

    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, Rgba(bg.0));
    let mut cursor: i64 = 0;
    for img in images {
        let (x, y) = match direction {
            Direction::Horizontal => {
                let cross = canvas_h.saturating_sub(img.height());
                let y = match align {
                    Align::Start => 0,
                    Align::Center => cross / 2,
                    Align::End => cross,
                };
                (cursor, y as i64)
            }
            Direction::Vertical => {
                let cross = canvas_w.saturating_sub(img.width());
                let x = match align {
                    Align::Start => 0,
                    Align::Center => cross / 2,
                    Align::End => cross,
                };
                (x as i64, cursor)
            }
        };
        imageops::overlay(&mut canvas, img, x, y);
        cursor += match direction {
            Direction::Horizontal => i64::from(img.width()) + i64::from(gap),
            Direction::Vertical => i64::from(img.height()) + i64::from(gap),
        };
    }
    Ok(canvas)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, color: [u8; 4]) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba(color))
    }

    #[test]
    fn horizontal_no_gap_canvas_size() {
        let imgs = vec![
            solid(100, 100, [255, 0, 0, 255]),
            solid(50, 100, [0, 255, 0, 255]),
            solid(80, 100, [0, 0, 255, 255]),
        ];
        let canvas = compose(&imgs, Direction::Horizontal, 0, Align::Start, ColorRgba::rgba(255, 255, 255, 255)).unwrap();
        assert_eq!(canvas.width(), 230);
        assert_eq!(canvas.height(), 100);
    }

    #[test]
    fn horizontal_with_gap_canvas_size() {
        let imgs = vec![
            solid(100, 100, [255, 0, 0, 255]),
            solid(100, 100, [0, 255, 0, 255]),
            solid(100, 100, [0, 0, 255, 255]),
        ];
        let canvas = compose(&imgs, Direction::Horizontal, 10, Align::Start, ColorRgba::rgba(255, 255, 255, 255)).unwrap();
        assert_eq!(canvas.width(), 320); // 100 + 10 + 100 + 10 + 100
    }

    #[test]
    fn horizontal_center_align_pads_short_image() {
        let imgs = vec![
            solid(50, 100, [255, 0, 0, 255]),
            solid(50, 50, [0, 255, 0, 255]), // shorter
        ];
        let canvas = compose(&imgs, Direction::Horizontal, 0, Align::Center, ColorRgba::rgba(255, 255, 255, 255)).unwrap();
        assert_eq!(canvas.width(), 100);
        assert_eq!(canvas.height(), 100);
        // Second image (green) should be centered vertically: y=25..75
        let p_top = canvas.get_pixel(75, 0).0; // bg
        let p_mid = canvas.get_pixel(75, 50).0; // green center
        assert_eq!(p_top, [255, 255, 255, 255]);
        assert_eq!(p_mid, [0, 255, 0, 255]);
    }

    #[test]
    fn vertical_canvas_size() {
        let imgs = vec![
            solid(100, 50, [255, 0, 0, 255]),
            solid(80, 50, [0, 255, 0, 255]),
        ];
        let canvas = compose(&imgs, Direction::Vertical, 5, Align::Start, ColorRgba::rgba(0, 0, 0, 255)).unwrap();
        assert_eq!(canvas.width(), 100);
        assert_eq!(canvas.height(), 105);
    }

    #[test]
    fn fewer_than_two_inputs_errs() {
        let imgs = vec![solid(10, 10, [255, 0, 0, 255])];
        assert!(compose(&imgs, Direction::Horizontal, 0, Align::Start, ColorRgba::rgba(255, 255, 255, 255)).is_err());
    }

    #[test]
    fn bg_color_visible_in_gap() {
        let imgs = vec![
            solid(50, 50, [255, 0, 0, 255]),
            solid(50, 50, [0, 0, 255, 255]),
        ];
        let canvas = compose(&imgs, Direction::Horizontal, 10, Align::Start, ColorRgba::rgba(0, 255, 0, 255)).unwrap();
        // gap region [50..60] should be green
        let p = canvas.get_pixel(55, 25).0;
        assert_eq!(p, [0, 255, 0, 255]);
    }
}
